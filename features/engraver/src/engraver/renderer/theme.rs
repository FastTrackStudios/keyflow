//! `ChartTheme`: everything about a chart's presentation that is not the
//! chart itself — its colours, its geometry, its type and its artwork —
//! as one value a host can swap at runtime.
//!
//! The split follows what each part costs to change:
//!
//! - **Colours** ([`RenderConfig`]) are applied at PAINT time. The layout
//!   bakes its ink into every `PaintCommand` (about 150 literals across
//!   `layout/`, nearly all `Color::BLACK`), and [`InkMap`] recolours them
//!   on the way out, so a colour change is a repaint of the layout already
//!   cached — never a re-layout, and never a wait on the paint thread.
//! - **Geometry and type** ([`ChartGeometry`], [`ChartTypography`]) feed
//!   the layout itself, so changing them re-lays the chart out. They hash
//!   into [`ChartTheme::layout_key`], which a host compares to decide
//!   whether a background layout is needed (see `ChartView::layout_job`).
//! - **Artwork** ([`PageArtwork`]) is vector paths drawn on each page,
//!   under or over the music, at paint time.
//!
//! [`ChartTheme::default`] is keyflow-ui's own look, and paints exactly as
//! the renderer did before themes existed: its [`InkMap`] is `None`.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use kurbo::{Affine, BezPath, Rect};
use peniko::Color;

use super::RenderConfig;
use crate::api::pipeline::Paper;
use crate::engraver::layout::PageMargins;
pub use crate::engraver::layout::chart::constants::REPEAT_COLOR;
use crate::engraver::style::MStyle;

/// The red the layout draws push and accent markers and mid-chart key
/// and meter changes in (`chord_renderer.rs`, `layout/chart/mod.rs`).
/// With [`REPEAT_COLOR`], one of the two accents a theme usually remaps.
pub const MARKER_COLOR: Color = Color::from_rgb8(0xCC, 0x00, 0x00);

/// What a colour is used for, which decides what a theme turns it into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InkRole {
    /// Glyphs, text and filled shapes.
    Fill,
    /// Strokes: staff lines, barlines, stems, boxes.
    Line,
}

/// A theme's recolouring of the layout's baked ink.
///
/// Neutral colours (black through grey to white) are re-expressed between
/// the theme's ink and its paper: black becomes the ink (or the line
/// colour, for strokes), white becomes the paper, and a grey keeps its
/// place between the two. That covers the text knock-outs drawn in white
/// and the greys used for secondary marks without naming each one.
/// Chromatic colours (rehearsal-mark palettes, highlights) pass through
/// unless [`RenderConfig::accents`] maps them.
#[derive(Debug, Clone, PartialEq)]
pub struct InkMap {
    paper: [f32; 4],
    ink: [f32; 4],
    line: [f32; 4],
    accents: Vec<([u8; 3], Color)>,
}

impl InkMap {
    /// The map `config` describes, or `None` when it is the identity (the
    /// default black on white with no accents) — so the default look pays
    /// nothing and stays bit-for-bit what it was.
    #[must_use]
    pub fn from_render_config(config: &RenderConfig) -> Option<Self> {
        let identity = config.background_color == Color::WHITE
            && config.foreground_color == Color::BLACK
            && config.staff_color == Color::BLACK
            && config.accents.is_empty();
        if identity {
            return None;
        }
        Some(Self {
            paper: config.background_color.components,
            ink: config.foreground_color.components,
            line: config.staff_color.components,
            accents: config
                .accents
                .iter()
                .map(|(from, to)| {
                    let c = from.to_rgba8();
                    ([c.r, c.g, c.b], *to)
                })
                .collect(),
        })
    }

    /// `color` as this theme paints it in `role`.
    #[must_use]
    pub fn map(&self, color: Color, role: InkRole) -> Color {
        let [r, g, b, a] = color.components;
        if (r - g).abs() < 1e-4 && (g - b).abs() < 1e-4 {
            let base = match role {
                InkRole::Fill => self.ink,
                InkRole::Line => self.line,
            };
            let t = r.clamp(0.0, 1.0);
            let mix = |i: usize| base[i] * (1.0 - t) + self.paper[i] * t;
            return Color::new([mix(0), mix(1), mix(2), a * mix(3)]);
        }
        let rgb = color.to_rgba8();
        match self
            .accents
            .iter()
            .find(|(from, _)| *from == [rgb.r, rgb.g, rgb.b])
        {
            Some((_, to)) => to.multiply_alpha(a),
            None => color,
        }
    }
}

/// The page a chart is laid out on, and how the music sits on it.
#[derive(Debug, Clone, Copy)]
pub struct ChartGeometry {
    /// Paper size.
    pub paper: Paper,
    /// Page margins in points; `None` keeps the preset's.
    pub margins: Option<PageMargins>,
    /// Space between systems in points; `None` keeps the preset's.
    pub system_spacing: Option<f64>,
}

impl Default for ChartGeometry {
    fn default() -> Self {
        Self {
            paper: Paper::Letter,
            margins: None,
            system_spacing: None,
        }
    }
}

/// The chart's type: chord-symbol size, text faces and the engraving
/// style table.
#[derive(Debug, Clone)]
pub struct ChartTypography {
    /// Multiplier on the chord-symbol size (1.0 keeps the preset's).
    pub chord_scale: f64,
    /// `(family the scene asks for, family to draw it with)` — e.g.
    /// `("title-bold", "FreeSans")` to set titles in a sans. The target
    /// must be one of the font bundle's registered families.
    pub font_aliases: Vec<(String, String)>,
    /// The engraving style table; `None` keeps keyflow-ui's default. Shared,
    /// so swapping it does not leak one per theme edit.
    pub style: Option<Arc<MStyle>>,
}

impl Default for ChartTypography {
    fn default() -> Self {
        Self {
            chord_scale: 1.0,
            font_aliases: Vec::new(),
            style: None,
        }
    }
}

/// Where a piece of page artwork sits relative to the music.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtworkLayer {
    /// Over the paper, under the music (a frame, a watermark, a band).
    UnderMusic,
    /// Over the music (a corner mark, a stamp).
    OverMusic,
}

/// A vector path drawn on every page.
#[derive(Debug, Clone)]
pub struct PageArtwork {
    /// The path, in `view_box` units.
    pub path: BezPath,
    /// The box `path` is drawn in, stretched over the whole page — the
    /// SVG `viewBox` with `preserveAspectRatio="none"`, so one piece of
    /// artwork fits Letter and A4 alike.
    pub view_box: Rect,
    /// Fill colour, if filled.
    pub fill: Option<Color>,
    /// Stroke colour and width in points, if stroked.
    pub stroke: Option<(Color, f64)>,
    /// Under or over the music.
    pub layer: ArtworkLayer,
}

impl PageArtwork {
    /// Artwork from SVG path data (`d`) in `view_box` units.
    ///
    /// # Errors
    ///
    /// `d` is not valid SVG path data.
    pub fn from_svg(d: &str, view_box: Rect, layer: ArtworkLayer) -> Result<Self, String> {
        let path = BezPath::from_svg(d).map_err(|e| e.to_string())?;
        Ok(Self {
            path,
            view_box,
            fill: None,
            stroke: None,
            layer,
        })
    }

    /// Fill it with `color`.
    #[must_use]
    pub fn with_fill(mut self, color: Color) -> Self {
        self.fill = Some(color);
        self
    }

    /// Stroke it with `color` at `width` points.
    #[must_use]
    pub fn with_stroke(mut self, color: Color, width: f64) -> Self {
        self.stroke = Some((color, width));
        self
    }

    /// The path placed on a page whose box is `page`, in chart points.
    #[must_use]
    pub fn placed(&self, page: Rect) -> BezPath {
        let vb = self.view_box;
        let sx = if vb.width() > 0.0 {
            page.width() / vb.width()
        } else {
            1.0
        };
        let sy = if vb.height() > 0.0 {
            page.height() / vb.height()
        } else {
            1.0
        };
        let fit = Affine::translate((page.x0, page.y0))
            * Affine::scale_non_uniform(sx, sy)
            * Affine::translate((-vb.x0, -vb.y0));
        fit * &self.path
    }
}

/// A chart's whole presentation. See the module doc.
#[derive(Debug, Clone, Default)]
pub struct ChartTheme {
    /// Colours (paper, ink, staff lines, cursor, accents) and the staff
    /// size.
    pub render: RenderConfig,
    /// Paper, margins and spacing.
    pub geometry: ChartGeometry,
    /// Chord size, text faces and the style table.
    pub typography: ChartTypography,
    /// Vector artwork drawn on each page.
    pub artwork: Vec<PageArtwork>,
}

impl ChartTheme {
    /// A hash of everything in the theme the LAYOUT depends on (geometry,
    /// type, staff size) — not the colours or the artwork, which only
    /// repaint. Two themes with the same key lay a chart out identically.
    #[must_use]
    pub fn layout_key(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let g = &self.geometry;
        matches!(g.paper, Paper::A4).hash(&mut h);
        if let Some(m) = g.margins {
            [m.top, m.right, m.bottom, m.left]
                .map(f64::to_bits)
                .hash(&mut h);
        }
        g.system_spacing.map(f64::to_bits).hash(&mut h);
        self.render.staff_space.map(f64::to_bits).hash(&mut h);
        let t = &self.typography;
        t.chord_scale.to_bits().hash(&mut h);
        t.style
            .as_ref()
            .map(|s| Arc::as_ptr(s) as usize)
            .hash(&mut h);
        h.finish()
    }

    /// A theme from named colour tokens — the contract a host's theme
    /// system (Frame's token table) fills. `lookup` answers a token name
    /// with its colour, or `None` to keep the default:
    ///
    /// | token          | sets                                      |
    /// |----------------|-------------------------------------------|
    /// | `chart.paper`  | [`RenderConfig::background_color`]        |
    /// | `chart.ink`    | [`RenderConfig::foreground_color`]        |
    /// | `chart.staff`  | [`RenderConfig::staff_color`]             |
    /// | `chart.cursor` | [`RenderConfig::cursor_color`]            |
    /// | `chart.select` | [`RenderConfig::selection_color`]         |
    /// | `chart.repeat` | accent for [`REPEAT_COLOR`] (repeats, `x4`)|
    /// | `chart.marker` | accent for [`MARKER_COLOR`] (push, changes)|
    #[must_use]
    pub fn from_tokens(lookup: impl Fn(&str) -> Option<Color>) -> Self {
        let mut theme = Self::default();
        let r = &mut theme.render;
        if let Some(c) = lookup("chart.paper") {
            r.background_color = c;
        }
        if let Some(c) = lookup("chart.ink") {
            r.foreground_color = c;
            r.staff_color = c;
        }
        if let Some(c) = lookup("chart.staff") {
            r.staff_color = c;
        }
        r.cursor_color = lookup("chart.cursor");
        if let Some(c) = lookup("chart.select") {
            r.selection_color = c;
        }
        for (token, from) in [
            ("chart.repeat", REPEAT_COLOR),
            ("chart.marker", MARKER_COLOR),
        ] {
            if let Some(to) = lookup(token) {
                r.accents.push((from, to));
            }
        }
        theme
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_render_config_is_the_identity() {
        assert!(InkMap::from_render_config(&RenderConfig::default()).is_none());
    }

    #[test]
    fn neutrals_move_between_ink_and_paper() {
        let config = RenderConfig {
            background_color: Color::from_rgb8(20, 20, 30),
            foreground_color: Color::from_rgb8(240, 230, 200),
            staff_color: Color::from_rgb8(100, 110, 120),
            ..RenderConfig::default()
        };
        let map = InkMap::from_render_config(&config).expect("not identity");
        assert_eq!(
            map.map(Color::BLACK, InkRole::Fill).to_rgba8(),
            config.foreground_color.to_rgba8()
        );
        assert_eq!(
            map.map(Color::BLACK, InkRole::Line).to_rgba8(),
            config.staff_color.to_rgba8()
        );
        assert_eq!(
            map.map(Color::WHITE, InkRole::Fill).to_rgba8(),
            config.background_color.to_rgba8()
        );
        // Translucent black keeps its alpha.
        let wash = map
            .map(Color::from_rgba8(0, 0, 0, 128), InkRole::Fill)
            .to_rgba8();
        assert_eq!((wash.r, wash.a), (240, 128));
        // Chromatic colours pass through unless mapped.
        let red = Color::from_rgb8(0xCC, 0, 0);
        assert_eq!(map.map(red, InkRole::Fill), red);
    }

    #[test]
    fn accents_remap_chromatic_colours() {
        let red = MARKER_COLOR;
        let teal = Color::from_rgb8(0, 160, 160);
        let config = RenderConfig {
            accents: vec![(red, teal)],
            ..RenderConfig::default()
        };
        let map = InkMap::from_render_config(&config).expect("not identity");
        assert_eq!(map.map(red, InkRole::Fill), teal);
    }

    #[test]
    fn layout_key_ignores_colours_and_tracks_geometry() {
        let base = ChartTheme::default();
        let mut recoloured = base.clone();
        recoloured.render.background_color = Color::BLACK;
        assert_eq!(base.layout_key(), recoloured.layout_key());
        let mut a4 = base.clone();
        a4.geometry.paper = Paper::A4;
        assert_ne!(base.layout_key(), a4.layout_key());
        let mut bigger = base.clone();
        bigger.typography.chord_scale = 1.25;
        assert_ne!(base.layout_key(), bigger.layout_key());
    }

    #[test]
    fn artwork_stretches_its_view_box_over_the_page() {
        let art = PageArtwork::from_svg(
            "M0 0 L100 100",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            ArtworkLayer::UnderMusic,
        )
        .expect("svg");
        let placed = art.placed(Rect::new(10.0, 20.0, 622.0, 812.0));
        let bounds = kurbo::Shape::bounding_box(&placed);
        assert_eq!(bounds, Rect::new(10.0, 20.0, 622.0, 812.0));
    }

    #[test]
    fn tokens_fill_the_render_config() {
        let theme = ChartTheme::from_tokens(|name| match name {
            "chart.paper" => Some(Color::BLACK),
            "chart.ink" => Some(Color::WHITE),
            "chart.repeat" => Some(Color::from_rgb8(0, 200, 200)),
            _ => None,
        });
        assert_eq!(theme.render.background_color, Color::BLACK);
        assert_eq!(theme.render.staff_color, Color::WHITE);
        assert!(theme.render.cursor_color.is_none());
        assert_eq!(
            theme.render.accents,
            vec![(REPEAT_COLOR, Color::from_rgb8(0, 200, 200))]
        );
    }
}
