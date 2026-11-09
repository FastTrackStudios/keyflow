//! The lyric lane: a section's timed `[lyrics]` words, engraved as one line
//! of text under each system, every word under the bar and beat it is sung
//! on.
//!
//! The words come from a timed lyric track (`@<bar>:<beat> words`, see
//! `LyricLine::parse_timed`). A beat is a place, not a promise of room: a
//! word goes at its beat when it fits, and otherwise right after the word
//! before it, so a busy bar reads as a run of words instead of a pile.
//! Nothing here reaches into the measure loop — the caller hands over a
//! system's geometry once its other below-staff text is placed, and the
//! whole row is autoplaced below that text as one unit, so the words share
//! a baseline.

use kurbo::Point;
use peniko::Color;

use crate::chart::track::TrackType;
use crate::chart::types::ChartSection;
use crate::engraver::layout::text_metrics::TextFontMetrics;
use crate::engraver::scene::id::{ElementType, SemanticId};
use crate::engraver::scene::node::SceneNode;
use crate::engraver::scene::paint::PaintCommand;
use crate::engraver::style::{MStyle, Sid};

use super::notation_renderer::{self, MeasureSkyline};

/// Where the row starts before autoplace moves it clear of cues.
const LYRIC_BASELINE_BELOW_STAFF_SP: f64 = 3.0;

fn lyric_color() -> Color {
    Color::from_rgba8(0x22, 0x22, 0x22, 0xFF)
}

/// A system's geometry, as the measure loop left it.
pub struct LyricSystem<'a> {
    pub section: &'a ChartSection,
    /// Section-local indices of the system's bars, left to right.
    pub measure_indices: &'a [usize],
    /// Left edge of the first bar's content.
    pub first_measure_x: f64,
    /// Width of each bar, parallel to `measure_indices`.
    pub widths: &'a [f64],
    pub staff_y: f64,
    pub staff_height: f64,
    pub spatium: f64,
    /// The theme: `Sid::LyricsFontFace` / `LyricsFontSize` / `LyricsFontStyle`.
    pub style: &'a MStyle,
}

/// The lyric face the theme asks for.
struct LyricFont {
    family: String,
    size: f64,
    italic: bool,
}

impl LyricFont {
    fn from_style(style: &MStyle, spatium: f64) -> Self {
        let family = match style.string(Sid::LyricsFontFace) {
            "" => "FreeSans".to_string(),
            f => f.to_string(),
        };
        let size_sp = match style.real(Sid::LyricsFontSize) as f64 {
            s if s > 0.0 => s,
            _ => 2.0,
        };
        Self {
            family,
            size: spatium * size_sp,
            italic: style.font_style(Sid::LyricsFontStyle).italic,
        }
    }

    fn text(&self, text: &str, at: Point, color: Color) -> PaintCommand {
        if self.italic {
            PaintCommand::text_italic(text, &self.family, self.size, at, color)
        } else {
            PaintCommand::text(text, &self.family, self.size, at, color)
        }
    }

    /// Metrics for this face: FreeSans (upright or oblique) is measured
    /// from its own outlines; any other family falls back to `chart`, the
    /// chart's text font.
    fn metrics(&self, chart: &TextFontMetrics) -> TextFontMetrics {
        use std::sync::{Arc, OnceLock};
        static OBLIQUE: OnceLock<TextFontMetrics> = OnceLock::new();
        static UPRIGHT: OnceLock<TextFontMetrics> = OnceLock::new();
        if self.family != "FreeSans" {
            return chart.clone();
        }
        if self.italic {
            OBLIQUE
                .get_or_init(|| {
                    TextFontMetrics::new(Arc::new(
                        crate::engraver::fonts::bundle::FREESANS_OBLIQUE_FONT_BYTES.to_vec(),
                    ))
                })
                .clone()
        } else {
            UPRIGHT
                .get_or_init(|| {
                    TextFontMetrics::new(Arc::new(
                        crate::engraver::fonts::bundle::FREESANS_FONT_BYTES.to_vec(),
                    ))
                })
                .clone()
        }
    }
}

/// One syllable to draw, with where its beat is.
#[derive(Debug, Clone, PartialEq, Default)]
struct Placed {
    text: String,
    target_x: f64,
    /// Finishes the hyphenated word before it (the `ceive` of `re-ceive`).
    glued: bool,
    /// The word goes on in the next syllable.
    hyphen_after: bool,
    /// Held: a melisma line runs from its end to the next syllable.
    extender_after: bool,
}

/// A system's slice of one lyric line.
#[derive(Debug, Default)]
struct Row {
    words: Vec<Placed>,
    /// A syllable held over from the previous system: the line opens with
    /// an extender.
    leading_extender: bool,
}

/// Is this syllable an extender (`_`, `____`) rather than a word?
fn is_extender(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c == '_')
}

/// Does any timed lyric word fall in these bars? The caller uses this to
/// hold the band under the staff open.
pub fn system_has_lyrics(section: &ChartSection, measure_indices: &[usize]) -> bool {
    timed_lines(section).any(|line| {
        line.syllables
            .iter()
            .any(|s| measure_indices.contains(&s.measure_index))
    })
}

fn timed_lines(
    section: &ChartSection,
) -> impl Iterator<Item = &crate::chart::lyrics::LyricLine> + '_ {
    section
        .tracks
        .iter()
        .filter(|t| t.track_type == TrackType::Lyrics)
        .filter_map(|t| t.lyrics.as_ref())
        .filter(|l| l.timed)
}

/// Engrave the system's lyric rows (one per timed lyric track). Each row is
/// a line of its own across the whole system, below every cue row: a row
/// that only stepped around the cues it touched would interleave words with
/// the cues beside them, and read as neither.
pub fn place_system_lyrics(
    skyline: &mut MeasureSkyline,
    system: &LyricSystem<'_>,
    text_metrics: &TextFontMetrics,
    id_counter: &mut u64,
) -> Vec<SceneNode> {
    let font = LyricFont::from_style(system.style, system.spatium);
    let text_metrics = &font.metrics(text_metrics);
    let font_size = font.size;
    let first_baseline =
        system.staff_y + system.staff_height + system.spatium * LYRIC_BASELINE_BELOW_STAFF_SP;
    let left_edge = system.first_measure_x;
    let right_edge = system.first_measure_x + system.widths.iter().sum::<f64>();
    let metrics = Metrics {
        space: text_metrics
            .horizontal_advance(" ", font_size)
            .max(system.spatium * 0.6),
        hyphen: text_metrics.horizontal_advance("-", font_size),
        pad: system.spatium * 0.35,
    };

    let mut nodes = Vec::new();
    for line in timed_lines(system.section) {
        let row = place_row(line, system);
        if row.words.is_empty() && !row.leading_extender {
            continue;
        }
        let widths: Vec<f64> = row
            .words
            .iter()
            .map(|p| text_metrics.horizontal_advance(&p.text, font_size))
            .collect();
        let Some(baseline) = reserve_row(
            skyline,
            system,
            first_baseline,
            font_size,
            left_edge,
            right_edge,
        ) else {
            continue;
        };
        let xs = flow(&row.words, &widths, &metrics, right_edge);
        let marks = connectors(&row, &xs, &widths, &metrics, left_edge, right_edge);

        let color = lyric_color();
        let mut paints: Vec<PaintCommand> = row
            .words
            .iter()
            .zip(&xs)
            .map(|(p, &x)| font.text(&p.text, Point::new(x, baseline), color))
            .collect();
        for mark in marks {
            paints.push(match mark {
                Mark::Hyphen(x) => font.text("-", Point::new(x, baseline), color),
                Mark::Extender(x0, x1) => {
                    let y = baseline + system.spatium * 0.15;
                    PaintCommand::line(
                        Point::new(x0, y),
                        Point::new(x1, y),
                        color,
                        system.spatium * 0.12,
                    )
                }
            });
        }
        let mut node = SceneNode::leaf(SemanticId::new(ElementType::Lyrics, *id_counter), paints);
        node.set_element_type("lyric_lane");
        *id_counter += 1;
        nodes.push(node);
    }
    nodes
}

/// Claim a full-width band for one lyric row below everything already under
/// the staff, and return its baseline. The band — not the words — goes into
/// the skyline, so a second row lands below the first.
fn reserve_row(
    skyline: &mut MeasureSkyline,
    system: &LyricSystem<'_>,
    baseline: f64,
    font_size: f64,
    left_edge: f64,
    right_edge: f64,
) -> Option<f64> {
    let band = kurbo::Rect::new(
        left_edge,
        baseline - font_size * 0.8,
        right_edge,
        baseline + font_size * 0.25,
    );
    let probe = SceneNode::leaf(
        SemanticId::new(ElementType::Lyrics, 0),
        vec![PaintCommand::filled_rect(band, Color::TRANSPARENT)],
    );
    let placed = notation_renderer::autoplace_node(skyline, probe, false, system.spatium)?;
    let moved = notation_renderer::scene_ink_bounds(&placed, kurbo::Affine::IDENTITY)?;
    Some(baseline + (moved.y0 - band.y0))
}

/// The syllables of `line` that fall in this system, at their beats.
fn place_row(line: &crate::chart::lyrics::LyricLine, system: &LyricSystem<'_>) -> Row {
    let measures = system.section.measures();
    let mut row = Row::default();
    let mut prev_hyphen = false;
    for syl in &line.syllables {
        let glued = prev_hyphen;
        let in_system = system
            .measure_indices
            .iter()
            .position(|&m| m == syl.measure_index);
        if is_extender(&syl.text) {
            if in_system.is_some() {
                match row.words.last_mut() {
                    Some(last) => last.extender_after = true,
                    None => row.leading_extender = true,
                }
            }
            continue;
        }
        prev_hyphen = syl.hyphen_after;
        let Some(local) = in_system else {
            continue;
        };
        let measure_x = system.first_measure_x + system.widths[..local].iter().sum::<f64>();
        let width = system.widths[local];
        let beats = measures
            .get(syl.measure_index)
            .map(|m| m.time_signature.0)
            .filter(|&b| b > 0)
            .unwrap_or(4) as f64;
        row.words.push(Placed {
            text: syl.text.clone(),
            target_x: beat_x(measure_x, width, beats, syl.beat as f64, system.spatium),
            glued,
            hyphen_after: syl.hyphen_after,
            extender_after: false,
        });
    }
    row
}

/// The x of a 0-based beat on an even grid across the bar — the same grid
/// the bar's slashes sit on, near enough for "this word is on beat 3".
fn beat_x(measure_x: f64, width: f64, beats: f64, beat: f64, spatium: f64) -> f64 {
    let usable = (width - spatium).max(0.0);
    let beat = beat.clamp(0.0, (beats - 0.25).max(0.0));
    measure_x + spatium * 0.5 + beat / beats * usable
}

/// Widths the flow needs besides the words'.
struct Metrics {
    /// Between two words.
    space: f64,
    /// A hyphen's advance.
    hyphen: f64,
    /// Clear space either side of a hyphen or an extender.
    pad: f64,
}

impl Metrics {
    /// The least gap before word `p`: room for a hyphen inside a word,
    /// a space between words.
    fn gap_before(&self, p: &Placed) -> f64 {
        if p.glued {
            self.hyphen + 2.0 * self.pad
        } else {
            self.space
        }
    }
}

/// Left edges for each word: at its beat when it fits, else right after
/// the word before it; then pulled back from the system's right edge so the
/// last words stay on the page. Words never overlap and never reorder.
fn flow(placed: &[Placed], widths: &[f64], m: &Metrics, right_edge: f64) -> Vec<f64> {
    let mut xs = Vec::with_capacity(placed.len());
    let mut prev_end = f64::NEG_INFINITY;
    for (p, w) in placed.iter().zip(widths) {
        let x = p.target_x.max(prev_end + m.gap_before(p));
        xs.push(x);
        prev_end = x + w;
    }
    // Backward pass: keep the row inside the system — a trailing hyphen
    // included.
    let mut limit = right_edge
        - placed
            .last()
            .filter(|p| p.hyphen_after)
            .map_or(0.0, |_| m.hyphen + m.pad);
    for i in (0..xs.len()).rev() {
        if xs[i] + widths[i] > limit {
            xs[i] = limit - widths[i];
        }
        limit = xs[i] - m.gap_before(&placed[i]);
    }
    xs
}

/// A mark between syllables.
#[derive(Debug, Clone, PartialEq)]
enum Mark {
    /// A hyphen with its left edge here.
    Hyphen(f64),
    /// A melisma line from x0 to x1.
    Extender(f64, f64),
}

/// Hyphens centred in the gap between the syllables of a word (or after
/// the last syllable when the word goes on in the next system), and
/// melisma lines from a held syllable to the next one.
fn connectors(
    row: &Row,
    xs: &[f64],
    widths: &[f64],
    m: &Metrics,
    left_edge: f64,
    right_edge: f64,
) -> Vec<Mark> {
    let mut marks = Vec::new();
    let min_line = m.space;
    if row.leading_extender {
        let end = xs.first().map_or(right_edge, |&x| x - m.pad);
        if end - left_edge >= min_line {
            marks.push(Mark::Extender(left_edge, end));
        }
    }
    for (i, p) in row.words.iter().enumerate() {
        let end = xs[i] + widths[i];
        let next = xs.get(i + 1).copied();
        if p.hyphen_after {
            match next {
                Some(nx) if row.words[i + 1].glued => {
                    marks.push(Mark::Hyphen((end + nx - m.hyphen) / 2.0));
                }
                Some(_) => {}
                None => marks.push(Mark::Hyphen(end + m.pad * 0.5)),
            }
        }
        if p.extender_after {
            let from = end + m.pad;
            let to = next.map_or(right_edge, |nx| nx - m.pad);
            if to - from >= min_line {
                marks.push(Mark::Extender(from, to));
            }
        }
    }
    marks
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: Metrics = Metrics {
        space: 5.0,
        hyphen: 4.0,
        pad: 1.0,
    };

    fn word(text: &str, target_x: f64) -> Placed {
        Placed {
            text: text.into(),
            target_x,
            ..Placed::default()
        }
    }

    #[test]
    fn a_word_sits_on_its_beat_when_there_is_room() {
        let placed = [word("Joy", 10.0), word("world", 60.0)];
        assert_eq!(flow(&placed, &[20.0, 30.0], &M, 500.0), vec![10.0, 60.0]);
    }

    #[test]
    fn crowded_words_run_on_instead_of_overlapping() {
        let placed = [word("Joy", 10.0), word("to", 12.0), word("the", 14.0)];
        let xs = flow(&placed, &[20.0, 10.0, 15.0], &M, 500.0);
        assert_eq!(xs, vec![10.0, 35.0, 50.0]);
    }

    #[test]
    fn the_row_stays_inside_the_system() {
        let placed = [word("one", 80.0), word("two", 90.0)];
        let xs = flow(&placed, &[10.0, 10.0], &M, 100.0);
        assert!(xs[1] + 10.0 <= 100.0);
        assert!(xs[0] + 10.0 + 5.0 <= xs[1]);
    }

    fn heaven(second_x: f64) -> Row {
        let mut heav = word("heav", 10.0);
        heav.hyphen_after = true;
        let mut en = word("en", second_x);
        en.glued = true;
        Row {
            words: vec![heav, en],
            leading_extender: false,
        }
    }

    #[test]
    fn a_hyphen_sits_centred_between_the_syllables_of_a_word() {
        let row = heaven(60.0);
        let widths = [20.0, 10.0];
        let xs = flow(&row.words, &widths, &M, 500.0);
        assert_eq!(xs, vec![10.0, 60.0]);
        let marks = connectors(&row, &xs, &widths, &M, 0.0, 500.0);
        // Gap 30..60, hyphen 4 wide: left edge at 43.
        assert_eq!(marks, vec![Mark::Hyphen(43.0)]);
        assert!(row.words.iter().all(|p| !p.text.ends_with('-')));
    }

    #[test]
    fn syllables_on_the_same_place_keep_room_for_their_hyphen() {
        let row = heaven(10.0);
        let widths = [20.0, 10.0];
        let xs = flow(&row.words, &widths, &M, 500.0);
        assert_eq!(xs[1], 30.0 + 4.0 + 2.0);
    }

    #[test]
    fn a_word_broken_at_the_system_end_keeps_its_hyphen() {
        let mut row = heaven(60.0);
        row.words.pop();
        let marks = connectors(&row, &[10.0], &[20.0], &M, 0.0, 500.0);
        assert_eq!(marks, vec![Mark::Hyphen(30.5)]);
    }

    #[test]
    fn a_held_syllable_draws_a_line_to_the_next_one() {
        let mut night = word("night", 10.0);
        night.extender_after = true;
        let row = Row {
            words: vec![night, word("Ho", 100.0)],
            leading_extender: false,
        };
        let marks = connectors(&row, &[10.0, 100.0], &[25.0, 10.0], &M, 0.0, 500.0);
        assert_eq!(marks, vec![Mark::Extender(36.0, 99.0)]);
    }

    #[test]
    fn a_held_syllable_at_the_system_end_runs_to_the_edge() {
        let mut night = word("night", 10.0);
        night.extender_after = true;
        let row = Row {
            words: vec![night],
            leading_extender: false,
        };
        let marks = connectors(&row, &[10.0], &[25.0], &M, 0.0, 200.0);
        assert_eq!(marks, vec![Mark::Extender(36.0, 200.0)]);
    }

    #[test]
    fn beats_spread_across_the_bar() {
        let b1 = beat_x(100.0, 200.0, 4.0, 0.0, 10.0);
        let b3 = beat_x(100.0, 200.0, 4.0, 2.0, 10.0);
        assert_eq!(b1, 105.0);
        assert_eq!(b3, 105.0 + 0.5 * 190.0);
    }
}
