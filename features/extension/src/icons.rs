//! The toolbar's and the palette's icons: SMuFL glyphs of the chart's own
//! music font (`ChartFontBundle`), each drawn as an SVG, so a button shows
//! the same quarter note the chart engraves. Made once, from the font in
//! the binary, and handed to the app as the package's files
//! (`Extension::files`): a toolbar item's `image` names one. The same
//! files are in the package's folder (`icons/`), where a process copy's
//! host and an installed package read them; a test keeps them equal.

use std::sync::OnceLock;

use keyflow::engraver::fonts::ChartFontBundle;
use kurbo::{Affine, Shape};

use crate::tools::{Face, TOOLS};

/// The ink an icon is drawn in: the app's chrome is dark, as MuseScore's.
const INK: &str = "#e6e6e6";

/// The package path of glyph `glyph`'s icon.
#[must_use]
pub fn path_of(glyph: char) -> String {
    format!("icons/{:04X}.svg", u32::from(glyph))
}

/// Glyph `glyph` of the chart's music font as an SVG, fitted to its own
/// box: none when the font has no such glyph.
#[must_use]
pub fn svg(glyph: char) -> Option<String> {
    let bundle = ChartFontBundle::shared().ok()?;
    let path = bundle.smufl_font().get_glyph_path(glyph, 100.0)?;
    // Font outlines are y-up; SVG is y-down.
    let path = Affine::FLIP_Y * path;
    let b = path.bounding_box();
    if b.width() <= 0.0 || b.height() <= 0.0 {
        return None;
    }
    // A square box, the same for every icon at least: three staff spaces
    // (a staff space is a quarter of the em, 25 at size 100). Notes and
    // accidentals fill it, as in MuseScore's toolbar; a dot or a staccato
    // stays small rather than blown up to the box.
    const MIN_SIDE: f64 = 75.0;
    let side = b.width().max(b.height()).max(MIN_SIDE) * 1.15;
    let (x, y) = (b.center().x - side / 2.0, b.center().y - side / 2.0);
    Some(format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{x:.2} {y:.2} {side:.2} {side:.2}"><path fill="{INK}" d="{}"/></svg>"#,
        path.to_svg()
    ))
}

/// Every icon the tools name, as `(path, svg)`: the package's files.
#[must_use]
pub fn files() -> &'static [(&'static str, &'static str)] {
    static FILES: OnceLock<Vec<(&'static str, &'static str)>> = OnceLock::new();
    FILES.get_or_init(|| {
        TOOLS
            .iter()
            .filter_map(|t| match t.face {
                Face::Glyph(g) => svg(g).map(|svg| (path_of(g), svg)),
                Face::Icon(_) => None,
            })
            // Made once for the process's life, as an embedded package's
            // files are: `Extension::files` hands out `'static` text.
            .map(|(p, s)| -> (&'static str, &'static str) {
                (Box::leak(p.into_boxed_str()), Box::leak(s.into_boxed_str()))
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The icons are the package's files on disk too (`icons/`), where a
    /// process copy's host and an installed package read them. They are
    /// these, made from the font: `KEYFLOW_WRITE_ICONS=1` writes them again.
    #[test]
    fn the_packages_icon_files_are_the_fonts_glyphs() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let write = std::env::var_os("KEYFLOW_WRITE_ICONS").is_some();
        for (path, svg) in files() {
            let file = dir.join(path);
            if write {
                std::fs::create_dir_all(file.parent().unwrap()).unwrap();
                std::fs::write(&file, svg).unwrap();
            }
            let on_disk = std::fs::read_to_string(&file)
                .unwrap_or_else(|e| panic!("{path}: {e} (KEYFLOW_WRITE_ICONS=1 writes it)"));
            assert_eq!(on_disk, *svg, "{path} differs from the font's glyph");
        }
    }

    #[test]
    fn every_glyph_tool_has_an_icon_from_the_music_font() {
        for t in TOOLS {
            if let Face::Glyph(g) = t.face {
                let svg = svg(g).unwrap_or_else(|| panic!("{}: no glyph {g:?}", t.action));
                assert!(
                    svg.starts_with("<svg") && svg.contains(" d=\"M"),
                    "{}",
                    t.action
                );
            }
        }
        let glyphs = TOOLS
            .iter()
            .filter(|t| matches!(t.face, Face::Glyph(_)))
            .count();
        assert_eq!(files().len(), glyphs);
    }
}
