//! The stepped playback cursor: which engraved symbol is sounding.
//!
//! MuseScore's playback cursor is not a line gliding across the bar: it is
//! a translucent bar, as tall as the system, sitting on the rhythmic event
//! that is sounding (a slash, a note, a rest, a tied continuation) and
//! jumping to the next one at that one's time. [`PlaybackTrack`] is that,
//! for a laid-out chart: every drawn rhythmic event *as it is played*, in
//! playing order, each with its time and its box.
//!
//! Where an event is comes from the layout's own beat positions (one per
//! chord-rest segment the engraver placed: its bar, its tick in the bar,
//! its length, its x); when it sounds comes from [`bar_times`], which walks
//! the chart in playing order (a repeat as many times as it says, endings
//! on their own passes) at the chart's tempo and meters. So a bar a repeat
//! plays twice is in the track twice, at the same place, and the cursor
//! goes back to it. Nothing here interpolates x across a bar.
//!
//! What the cursor does where:
//!
//! - **A held symbol** (a whole-bar slash, a bar's rest) is one event: the
//!   cursor stays on it for its length.
//! - **A repeat-bar sign** (a simile bar) is one event the length of the
//!   bar, on the sign: the slashes it stands for are not drawn, so they are
//!   not stepped through.
//! - **A multi-bar rest**: the chart engraver draws none (every bar of rest
//!   is its own bar with its own rest), so the cursor sits on each bar's
//!   rest for that bar, then steps to the next.
//! - **A bar the layout did not draw** (a folded section) has no events:
//!   the cursor is absent until the next drawn bar.
//! - **Before bar 1 and after the last bar** there is no event: no cursor.
//!   The count-in's beats (negative ticks, in the header) are not events.

use keyflow_lang::text::bar_times::{BarClock, BarTime, bar_times};
use keyflow_proto::Chart;

use super::types::{BeatPosition, ChartLayoutResult};
use crate::engraver::scene::SceneNode;

/// How far the cursor reaches past the staff's top and bottom lines, in
/// staff spaces (MuseScore's cursor overshoots the staff by about this).
pub const OVERSHOOT_SPACES: f64 = 0.75;
/// The narrowest the cursor is drawn, in staff spaces: a stemmed slash's
/// head is about one and a half wide, and a thinner bar reads as a line.
pub const MIN_WIDTH_SPACES: f64 = 2.0;
/// The room left each side of the symbol, in staff spaces.
const PAD_SPACES: f64 = 0.3;

/// One rhythmic event as it is played.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackEvent {
    /// The bar, as the engraver numbers it (0-based over every drawn bar).
    pub bar: usize,
    /// Which time through a repeat this is (1 outside repeats).
    pub pass: u32,
    /// Where in the bar it starts, in quarter notes from the barline.
    pub beat: f64,
    /// Its tick in the bar and its length (480 a quarter).
    pub tick: i32,
    pub duration_ticks: i32,
    /// When it sounds, in seconds on the clock the track was built with.
    pub start: f64,
    pub end: f64,
    /// The page (1-based) and the system it is drawn on.
    pub page: u32,
    pub system: usize,
    /// The symbol's own box, in chart points: `x0, y0, x1, y1`, the staff's
    /// top line to its bottom line.
    pub symbol: (f64, f64, f64, f64),
    /// The cursor's box over it: the symbol's, at least
    /// [`MIN_WIDTH_SPACES`] wide, with [`OVERSHOOT_SPACES`] above and
    /// below the staff.
    pub cursor: (f64, f64, f64, f64),
    /// The bar's own box, barline to barline, on the staff.
    pub bar_box: (f64, f64, f64, f64),
    /// Whether the event is a repeat-bar sign standing for the whole bar.
    pub simile: bool,
}

/// A laid-out chart's rhythmic events in playing order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlaybackTrack {
    events: Vec<PlaybackEvent>,
}

impl PlaybackTrack {
    /// `chart`'s events as `layout` drew them, on `clock`.
    #[must_use]
    pub fn new(layout: &ChartLayoutResult, chart: &Chart, clock: BarClock<'_>) -> Self {
        Self::from_bars(layout, &bar_times(chart, clock))
    }

    /// The events of `bars` (a chart's bars in playing order, each with its
    /// time: [`bar_times`]) as `layout` drew them.
    #[must_use]
    pub fn from_bars(layout: &ChartLayoutResult, bars: &[BarTime]) -> Self {
        // Each drawn bar's beats, in the bar's order. The count-in's beats
        // carry negative ticks and share measure indices 0 and 1 with the
        // chart's first bars: they are not events.
        let mut by_bar: std::collections::BTreeMap<usize, Vec<&BeatPosition>> =
            std::collections::BTreeMap::new();
        for beat in &layout.beat_positions {
            if beat.absolute_tick >= 0 {
                by_bar.entry(beat.measure).or_default().push(beat);
            }
        }
        let similes = simile_bars(layout);

        let mut events = Vec::new();
        for bar in bars {
            let Some(beats) = by_bar.get(&bar.bar) else {
                continue;
            };
            let Some(first) = beats.first() else {
                continue;
            };
            let (n, d) = first.time_signature;
            let bar_ticks = (i32::from(n.max(1)) * (1920 / i32::from(d.max(1)))).max(1);
            let span = bar.end - bar.start;
            let at = |tick: i32| bar.start + span * f64::from(tick) / f64::from(bar_ticks);
            let simile = similes.iter().any(|&(x, y)| {
                x >= first.measure_x0
                    && x <= first.measure_x1
                    && (y - (first.staff_y + first.staff_height / 2.0)).abs() <= first.staff_height
            });
            if simile {
                // The sign, centred in the bar, for the whole bar.
                let space = first.glyph_size;
                let mid = (first.measure_x0 + first.measure_x1) / 2.0;
                let half = SIMILE_WIDTH_SPACES * space / 2.0;
                events.push(event(
                    first,
                    bar.pass,
                    (0, bar_ticks),
                    (bar.start, bar.end),
                    (mid - half, mid + half),
                    true,
                ));
                continue;
            }
            for beat in beats {
                let width = symbol_width_spaces(beat.duration_ticks) * beat.glyph_size;
                events.push(event(
                    beat,
                    bar.pass,
                    (beat.tick, beat.duration_ticks),
                    (at(beat.tick), at(beat.tick + beat.duration_ticks)),
                    (beat.x, beat.x + width),
                    false,
                ));
            }
        }
        // Playing order is time order; a measured clock that runs backwards
        // somewhere must not break the search below.
        events.sort_by(|a, b| a.start.total_cmp(&b.start));
        Self { events }
    }

    /// Every event, in playing order.
    #[must_use]
    pub fn events(&self) -> &[PlaybackEvent] {
        &self.events
    }

    /// The index of the event sounding at `time`: the last one that has
    /// started, while it lasts. `None` before the first event, after the
    /// last one ends, and in a gap (a bar that was not drawn).
    #[must_use]
    pub fn index_at(&self, time: f64) -> Option<usize> {
        let after = self.events.partition_point(|e| e.start <= time);
        let index = after.checked_sub(1)?;
        (time < self.events[index].end).then_some(index)
    }

    /// The event sounding at `time` ([`Self::index_at`]).
    #[must_use]
    pub fn at(&self, time: f64) -> Option<&PlaybackEvent> {
        self.index_at(time).map(|i| &self.events[i])
    }
}

/// A repeat-bar sign's width, in staff spaces.
const SIMILE_WIDTH_SPACES: f64 = 2.6;

/// A rhythm symbol's width by its length, in staff spaces: the slash
/// noteheads' (a whole slash is a wide diamond, a half a narrower one, a
/// quarter and shorter the plain slash).
fn symbol_width_spaces(ticks: i32) -> f64 {
    match ticks {
        t if t >= 1920 => 2.4,
        t if t >= 960 => 2.0,
        _ => 1.5,
    }
}

fn event(
    beat: &BeatPosition,
    pass: u32,
    (tick, duration_ticks): (i32, i32),
    (start, end): (f64, f64),
    (x0, x1): (f64, f64),
    simile: bool,
) -> PlaybackEvent {
    let space = beat.glyph_size;
    let (top, bottom) = (beat.staff_y, beat.staff_y + beat.staff_height);
    // The cursor: the symbol and a little room, never narrower than the
    // minimum, centred on the symbol.
    let wanted = (x1 - x0 + 2.0 * PAD_SPACES * space).max(MIN_WIDTH_SPACES * space);
    let mid = (x0 + x1) / 2.0;
    let over = OVERSHOOT_SPACES * space;
    PlaybackEvent {
        bar: beat.measure,
        pass,
        beat: f64::from(tick) / 480.0,
        tick,
        duration_ticks,
        start,
        end,
        page: beat.page,
        system: beat.system,
        symbol: (x0, top, x1, bottom),
        cursor: (
            mid - wanted / 2.0,
            top - over,
            mid + wanted / 2.0,
            bottom + over,
        ),
        bar_box: (beat.measure_x0, top, beat.measure_x1, bottom),
        simile,
    }
}

/// Where each repeat-bar sign is drawn: its node's origin, in chart points.
fn simile_bars(layout: &ChartLayoutResult) -> Vec<(f64, f64)> {
    fn walk(node: &SceneNode, parent: kurbo::Affine, out: &mut Vec<(f64, f64)>) {
        if !node.visible {
            return;
        }
        let transform = parent * node.transform;
        if node.has_metadata("simile") {
            // The sign's glyph is drawn at its node's own origin plus its
            // centring offset: anywhere inside its bar finds the bar.
            let x = node
                .commands
                .iter()
                .find_map(|c| match c {
                    crate::engraver::scene::paint::PaintCommand::Glyph { position, .. } => {
                        Some(*position)
                    }
                    _ => None,
                })
                .unwrap_or_default();
            let at = transform * x;
            out.push((at.x, at.y));
            return;
        }
        for child in &node.children {
            walk(child, transform, out);
        }
    }
    let mut out = Vec::new();
    walk(&layout.scene, kurbo::Affine::IDENTITY, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::pipeline::{ChartPipeline, Preset, PresetOptions};

    fn track(src: &str) -> (ChartLayoutResult, PlaybackTrack) {
        track_with(src, false)
    }

    /// `similes`: `%` bars drawn as repeat-bar signs (the folded modes).
    fn track_with(src: &str, similes: bool) -> (ChartLayoutResult, PlaybackTrack) {
        let chart = keyflow_lang::text::chart::parse_chart(src).expect("parses");
        let pipeline = ChartPipeline::shared().expect("pipeline");
        let (mode, mut config) =
            ChartPipeline::resolve_preset(Preset::Page, PresetOptions::for_screen(612.0, 1.0));
        config.draw_similes = similes;
        let layout = pipeline.layout_with_config(&chart, &mode, &config);
        let track = PlaybackTrack::new(&layout, &chart, BarClock::at(0.0));
        (layout, track)
    }

    /// `(bar, pass, beat, length in quarters, start)` of every event.
    fn steps(track: &PlaybackTrack) -> Vec<(usize, u32, f64, f64, f64)> {
        track
            .events()
            .iter()
            .map(|e| {
                (
                    e.bar,
                    e.pass,
                    e.beat,
                    f64::from(e.duration_ticks) / 480.0,
                    (e.start * 1000.0).round() / 1000.0,
                )
            })
            .collect()
    }

    /// Mixed durations, a tie, bars of rest, a repeat-bar sign, a repeat
    /// with two endings and a bar of 3/4: 120 bpm, so a quarter is 0.5 s.
    const MIXED: &str = "Cursor - Test\n#C 120bpm 4/4\n\nVS 9\nC\nG_1\nC_2 G_4 ~ G_8 F_8\nr1\nr1\nAm_1\n%\n!T3/4 F_2 G_4\nC_1\n\nCH 4\n|: C_1 | [1] G_2. r4 :| [2] F_1 | C_1\n";

    #[test]
    fn the_cursor_steps_through_the_symbols_in_time() {
        let (_, track) = track(MIXED);
        let got = steps(&track);
        let want = [
            // Bar 1: four beat slashes.
            (0, 1, 0.0, 1.0, 0.0),
            (0, 1, 1.0, 1.0, 0.5),
            (0, 1, 2.0, 1.0, 1.0),
            (0, 1, 3.0, 1.0, 1.5),
            // Bar 2: a whole-bar slash, held.
            (1, 1, 0.0, 4.0, 2.0),
            // Bar 3: a half, a quarter tied to an eighth, an eighth.
            (2, 1, 0.0, 2.0, 4.0),
            (2, 1, 2.0, 1.0, 5.0),
            (2, 1, 3.0, 0.5, 5.5),
            (2, 1, 3.5, 0.5, 5.75),
            // Bars 4 and 5: two bars of rest, each its own.
            (3, 1, 0.0, 4.0, 6.0),
            (4, 1, 0.0, 4.0, 8.0),
            // Bar 6, and bar 7 the same again (`%`).
            (5, 1, 0.0, 4.0, 10.0),
            (6, 1, 0.0, 4.0, 12.0),
            // Bar 8 in 3/4, drawn as its three beat slashes: a beat shorter.
            (7, 1, 0.0, 1.0, 14.0),
            (7, 1, 1.0, 1.0, 14.5),
            (7, 1, 2.0, 1.0, 15.0),
            // Bar 9 is back in 4/4 at 15.5 s.
            (8, 1, 0.0, 4.0, 15.5),
            // The chorus: bar 10, the first ending, bar 10 again, the
            // second ending, the last bar.
            (9, 1, 0.0, 4.0, 17.5),
            (10, 1, 0.0, 3.0, 19.5),
            (10, 1, 3.0, 1.0, 21.0),
            (9, 2, 0.0, 4.0, 21.5),
            (11, 2, 0.0, 4.0, 23.5),
            (12, 1, 0.0, 4.0, 25.5),
        ];
        for (g, w) in got.iter().zip(want) {
            assert_eq!(*g, w);
        }
        assert_eq!(got.len(), want.len(), "{got:#?}");
        assert!(track.events().iter().all(|e| !e.simile), "written out");
    }

    #[test]
    fn a_repeat_bar_sign_is_one_event_for_its_bar() {
        // Bar 2 repeats a bar of four hits: drawn as the sign, it is not
        // stepped through.
        let src = "Sign - Test\n#C 120bpm 4/4\n\nVS 3\nC_4 G_4 C_4 G_4\n%\nF_1\n";
        let (_, written) = track_with(src, false);
        assert_eq!(written.events().iter().filter(|e| e.bar == 1).count(), 4);
        let (_, track) = track_with(src, true);
        let on_sign: Vec<_> = track.events().iter().filter(|e| e.bar == 1).collect();
        assert_eq!(on_sign.len(), 1, "{:#?}", steps(&track));
        let sign = on_sign[0];
        assert!(sign.simile);
        assert_eq!((sign.start, sign.end), (2.0, 4.0));
        for t in [2.0, 2.6, 3.1, 3.9] {
            assert_eq!(track.at(t), Some(sign));
        }
        assert_eq!(track.at(4.0).map(|e| e.bar), Some(2));
    }

    #[test]
    fn it_sits_on_the_symbol_and_never_glides() {
        let (layout, track) = track(MIXED);
        // Anywhere inside a held symbol is the same event, the same box.
        let held = track.at(2.0).expect("bar 2").clone();
        for t in [2.0, 2.4, 3.0, 3.99] {
            assert_eq!(track.at(t), Some(&held), "at {t}");
        }
        assert_eq!(track.at(4.0).map(|e| e.bar), Some(2), "then it jumps");

        for e in track.events() {
            let (sx0, sy0, sx1, sy1) = e.symbol;
            let (cx0, cy0, cx1, cy1) = e.cursor;
            let space = (sy1 - sy0) / 4.0;
            // The staff's height and the overshoot, top and bottom.
            assert!((sy0 - cy0 - OVERSHOOT_SPACES * space).abs() < 1e-6);
            assert!((cy1 - sy1 - OVERSHOOT_SPACES * space).abs() < 1e-6);
            // Over the symbol, at least the minimum wide.
            assert!(cx0 <= sx0 && cx1 >= sx1, "{e:?}");
            assert!(cx1 - cx0 >= MIN_WIDTH_SPACES * space - 1e-6);
            // On the beat the layout placed (not for the sign, which is
            // centred in its bar).
            if !e.simile {
                let beat = layout
                    .beat_positions
                    .iter()
                    .find(|b| b.absolute_tick >= 0 && b.measure == e.bar && b.tick == e.tick)
                    .expect("its beat");
                assert!((beat.x - sx0).abs() < 1e-6);
                assert_eq!(beat.page, e.page);
            } else {
                let (bx0, _, bx1, _) = e.bar_box;
                assert!(((sx0 + sx1) / 2.0 - (bx0 + bx1) / 2.0).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn a_repeat_goes_back_to_the_same_place() {
        let (_, track) = track(MIXED);
        let first = track.at(17.6).expect("bar 10, pass 1");
        let again = track.at(21.6).expect("bar 10, pass 2");
        assert_eq!((first.bar, first.pass), (9, 1));
        assert_eq!((again.bar, again.pass), (9, 2));
        assert_eq!(first.cursor, again.cursor, "the same symbol");
        // The first ending is skipped on pass 2.
        assert_eq!(track.at(23.6).map(|e| e.bar), Some(11));
    }

    #[test]
    fn there_is_no_cursor_outside_the_chart() {
        let (_, track) = track(MIXED);
        assert!(track.at(-0.01).is_none(), "before bar 1");
        assert_eq!(track.index_at(0.0), Some(0));
        let end = track.events().last().unwrap().end;
        assert!(track.at(end).is_none(), "after the last bar");
        assert!(track.at(end - 0.001).is_some());
    }

    #[test]
    fn a_count_in_is_not_stepped_through() {
        let (_, track) = track("Count - Test\n#C 120bpm 4/4\n\nCount 2\n\nVS 2\nC G\n");
        assert_eq!(
            track.events().first().map(|e| (e.bar, e.start)),
            Some((0, 0.0))
        );
        assert_eq!(track.events().len(), 8, "two bars of four slashes");
    }

    #[test]
    fn it_follows_the_layout_over_system_and_page_breaks() {
        // Enough bars for several systems and a second page.
        let mut src = String::from("Long - Test\n#C 120bpm 4/4\n\n");
        for _ in 0..12 {
            src.push_str("VS 8\nC F G C Am F G C\n\n");
        }
        let (_, track) = track(&src);
        let events = track.events();
        assert!(events.iter().any(|e| e.page == 2), "a second page");
        // Systems and pages only ever go forward with time here (no
        // repeats), and a new system starts back at the left.
        for pair in events.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            assert!(b.start >= a.start);
            assert!((b.page, b.system) >= (a.page, a.system));
            if b.system != a.system && b.page == a.page {
                assert!(b.cursor.0 < a.cursor.0, "back to the left edge");
            }
        }
    }
}
