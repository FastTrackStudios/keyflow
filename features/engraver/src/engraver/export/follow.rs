//! An engraved chart that follows a playing recording.
//!
//! A page that plays a chart's recording (an Architect share page or site)
//! follows any block marked `data-follow-playhead`: of the `[data-at]`
//! elements inside it (seconds, in document order), it marks the last one
//! that has started `aria-current="time"`, scrolls it into view inside the
//! block, and seeks to its time on a click. `data-media` on the block names
//! the recording the times belong to. The page brings the script; the
//! chart brings only attributes.
//!
//! [`add_follow`] puts them on an exported SVG: `data-follow-playhead` (and
//! `data-media`) on the root `<svg>`, and one transparent
//! `<rect class="kf-bar" data-at="…">` over each bar *as it is played*, in
//! playing order, after everything drawn. A bar a repeat plays twice gets two
//! rects at the same place, one per pass, so the times stay in ascending
//! document order — the follower finds the current one by binary search over
//! that order, and a single element per bar with the first pass's time would
//! leave pass two lit on the wrong bar. A click on a repeated bar seeks to its
//! last pass (the topmost rect).
//!
//! The drawing underneath is untouched: without a [`Follow`] the export is
//! byte-for-byte what it was.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use keyflow_lang::text::bar_times::{BarClock, BarTime, bar_times};
use keyflow_proto::Chart;

use crate::engraver::layout::chart::ChartLayoutResult;

/// The bar times to put on an engraving, and the recording they belong to.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Follow {
    /// Every bar in playing order ([`bar_times`]).
    pub bars: Vec<BarTime>,
    /// The recording (`data-media`), as a path or URL relative to the page.
    pub media: Option<String>,
}

impl Follow {
    /// `chart`'s bars on `clock`, for the recording `media`.
    #[must_use]
    pub fn new(chart: &Chart, clock: BarClock<'_>, media: Option<String>) -> Self {
        Self {
            bars: bar_times(chart, clock),
            media,
        }
    }

    /// What the chart itself asks for: `\downbeat = <seconds>` and/or
    /// `\media = "<file>"` in its header. `None` when it names neither, so
    /// an ordinary chart engraves exactly as before.
    #[must_use]
    pub fn from_header(chart: &Chart) -> Option<Self> {
        let settings = &chart.settings;
        if settings.downbeat.is_none() && settings.media.is_none() {
            return None;
        }
        Some(Self::new(
            chart,
            BarClock::at(settings.downbeat.unwrap_or(0.0)),
            settings.media.clone(),
        ))
    }
}

/// Where a drawn bar is: its page and its box in scene coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarBox {
    /// 1-based page (always 1 for a continuous layout).
    pub page: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Every drawn bar's box, by the engraver's bar index, from the layout's
/// beat positions (barline to barline across, the staff with room for the
/// chord symbols above it and a little below).
#[must_use]
pub fn bar_boxes(result: &ChartLayoutResult) -> BTreeMap<usize, BarBox> {
    let mut boxes = BTreeMap::new();
    for beat in &result.beat_positions {
        // Count-in beats carry negative ticks and their own small boxes in
        // the header; they are not bars.
        if beat.absolute_tick < 0 {
            continue;
        }
        boxes.entry(beat.measure).or_insert_with(|| {
            let above = beat.staff_height * 1.2;
            let below = beat.staff_height * 0.4;
            BarBox {
                page: beat.page,
                x: beat.measure_x0,
                y: beat.staff_y - above,
                width: (beat.measure_x1 - beat.measure_x0).max(0.0),
                height: beat.staff_height + above + below,
            }
        });
    }
    boxes
}

/// `svg` (an export of `result`) with the follow attributes added: the
/// root marked, one `kf-bar` rect per played bar. `page` keeps only the
/// bars drawn on that 1-based page (for a per-page export); `None` keeps
/// every bar.
///
/// Bars the layout did not draw (a folded section) have no rect; the next
/// drawn bar is still found at its own time.
#[must_use]
pub fn add_follow(
    svg: &str,
    result: &ChartLayoutResult,
    follow: &Follow,
    page: Option<u32>,
) -> String {
    let Some(root) = svg.find("<svg ") else {
        return svg.to_owned();
    };
    let Some(close) = svg.rfind("</svg>") else {
        return svg.to_owned();
    };

    let boxes = bar_boxes(result);
    let mut rects = String::new();
    for bar in &follow.bars {
        let Some(b) = boxes.get(&bar.bar) else {
            continue;
        };
        if page.is_some_and(|p| p != b.page) {
            continue;
        }
        let _ = writeln!(
            rects,
            r#"  <rect class="kf-bar" data-at="{:.3}" data-bar="{}" data-pass="{}" x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="2"/>"#,
            bar.start,
            bar.bar + 1,
            bar.pass,
            b.x,
            b.y,
            b.width,
            b.height,
        );
    }

    let mut attrs = String::from(" data-follow-playhead=\"\"");
    if let Some(media) = &follow.media {
        let _ = write!(attrs, " data-media=\"{}\"", escape_attr(media));
    }

    let at = root + "<svg".len();
    let mut out = String::with_capacity(svg.len() + rects.len() + 256);
    out.push_str(&svg[..at]);
    out.push_str(&attrs);
    out.push_str(&svg[at..close]);
    out.push_str("<g class=\"kf-follow\">\n");
    out.push_str(STYLE);
    out.push_str(&rects);
    out.push_str("</g>\n");
    out.push_str(&svg[close..]);
    out
}

/// [`add_follow`] on each page of a per-page export of `result` (in page
/// order, as `export_svg_pages*` return them), each page carrying only its
/// own bars.
#[must_use]
pub fn add_follow_pages(
    pages: &[String],
    result: &ChartLayoutResult,
    follow: &Follow,
) -> Vec<String> {
    pages
        .iter()
        .zip(1u32..)
        .map(|(svg, page)| add_follow(svg, result, follow, Some(page)))
        .collect()
}

/// The bars are invisible until current; a click anywhere on one lands on
/// it (`pointer-events: all` on a fill nobody sees).
const STYLE: &str = "  <style>.kf-bar{fill:#f5a623;fill-opacity:0;pointer-events:all;cursor:pointer}.kf-bar[aria-current=time]{fill-opacity:0.18}</style>\n";

fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::pipeline::ChartPipeline;
    use crate::engraver::layout::chart::LayoutMode;

    fn engrave(src: &str) -> (Chart, ChartLayoutResult, String) {
        let chart = keyflow_lang::text::chart::parse_chart(src).expect("parses");
        let pipeline = ChartPipeline::shared().expect("pipeline");
        let layout = pipeline.layout(&chart, &LayoutMode::ContinuousScroll { width: 800.0 });
        let svg = pipeline.export_svg_snippet(&layout, 6.0);
        (chart, layout, svg)
    }

    fn data_at(svg: &str) -> Vec<(String, String)> {
        svg.match_indices("data-at=\"")
            .map(|(i, _)| {
                let rest = &svg[i + 9..];
                let at = rest[..rest.find('"').unwrap()].to_owned();
                let b = rest.find("data-bar=\"").unwrap() + 10;
                let bar = rest[b..b + rest[b..].find('"').unwrap()].to_owned();
                (bar, at)
            })
            .collect()
    }

    #[test]
    fn a_chart_without_timing_engraves_as_before() {
        let (chart, _, _) = engrave("Song - Artist\n120bpm 4/4 #C\n\nVS 4\nC F G C\n");
        assert!(Follow::from_header(&chart).is_none());
    }

    #[test]
    fn every_bar_gets_its_time_and_the_root_follows() {
        let (chart, layout, svg) = engrave(
            "Song - Artist\n120bpm 4/4 #C\n\\downbeat = 1.5\n\\media = \"a & b.mp3\"\n\nVS 4\nC F G C\n",
        );
        let follow = Follow::from_header(&chart).expect("the header asks to follow");
        let out = add_follow(&svg, &layout, &follow, None);
        assert!(
            out.contains("<svg data-follow-playhead=\"\" data-media=\"a &amp; b.mp3\" xmlns="),
            "{}",
            &out[..300.min(out.len())]
        );
        assert_eq!(
            data_at(&out),
            vec![
                ("1".into(), "1.500".into()),
                ("2".into(), "3.500".into()),
                ("3".into(), "5.500".into()),
                ("4".into(), "7.500".into()),
            ]
        );
        // The drawing itself is untouched: only the root's attributes and
        // the overlay group are new.
        let stripped = out
            .replacen(
                " data-follow-playhead=\"\" data-media=\"a &amp; b.mp3\"",
                "",
                1,
            )
            .replace(
                &out[out.find("<g class=\"kf-follow\">").unwrap()..out.rfind("</svg>").unwrap()],
                "",
            );
        assert_eq!(stripped, svg);
    }

    #[test]
    fn a_repeated_bar_has_a_rect_per_pass_in_time_order() {
        let (chart, layout, svg) =
            engrave("Song - Artist\n120bpm 4/4 #C\n\\downbeat = 0\n\nVS 4\n|: C | G :| F C\n");
        let follow = Follow::from_header(&chart).unwrap();
        let out = add_follow(&svg, &layout, &follow, None);
        assert_eq!(
            data_at(&out),
            vec![
                ("1".into(), "0.000".into()),
                ("2".into(), "2.000".into()),
                ("1".into(), "4.000".into()),
                ("2".into(), "6.000".into()),
                ("3".into(), "8.000".into()),
                ("4".into(), "10.000".into()),
            ]
        );
        // The two passes of bar 1 cover the same box.
        let boxes = bar_boxes(&layout);
        assert_eq!(boxes.len(), 4);
    }
}
