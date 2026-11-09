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

use super::notation_renderer::{self, MeasureSkyline};

/// Lyric text size, in spatia. A little smaller than a cue, which is a
/// direction to be noticed; lyrics are a line to be read along.
const LYRIC_FONT_SIZE_SP: f64 = 2.2;
/// The font the chart's text metrics measure, so flow widths are real.
const LYRIC_FONT: &str = "MuseJazz Text";
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
}

/// One syllable to draw, with where its beat is.
#[derive(Debug, Clone, PartialEq)]
struct Placed {
    text: String,
    target_x: f64,
    /// Glued to the syllable before it (the second half of `re-ceive`).
    glued: bool,
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

/// Engrave the system's lyric rows (one per timed lyric track), each placed
/// below whatever the skyline already holds under the staff.
pub fn place_system_lyrics(
    skyline: &mut MeasureSkyline,
    system: &LyricSystem<'_>,
    text_metrics: &TextFontMetrics,
    id_counter: &mut u64,
) -> Vec<SceneNode> {
    let font_size = system.spatium * LYRIC_FONT_SIZE_SP;
    let baseline =
        system.staff_y + system.staff_height + system.spatium * LYRIC_BASELINE_BELOW_STAFF_SP;
    let right_edge = system.first_measure_x + system.widths.iter().sum::<f64>();
    let space = text_metrics
        .horizontal_advance(" ", font_size)
        .max(system.spatium * 0.6);

    let mut nodes = Vec::new();
    for line in timed_lines(system.section) {
        let placed = place_row(line, system);
        if placed.is_empty() {
            continue;
        }
        let widths: Vec<f64> = placed
            .iter()
            .map(|p| text_metrics.horizontal_advance(&p.text, font_size))
            .collect();
        let xs = flow(&placed, &widths, space, right_edge);
        let paints = placed
            .iter()
            .zip(xs)
            .map(|(p, x)| {
                PaintCommand::text(
                    &p.text,
                    LYRIC_FONT,
                    font_size,
                    Point::new(x, baseline),
                    lyric_color(),
                )
            })
            .collect();
        let mut node = SceneNode::leaf(SemanticId::new(ElementType::Lyrics, *id_counter), paints);
        node.set_element_type("lyric_lane");
        *id_counter += 1;
        if let Some(node) =
            notation_renderer::autoplace_node(skyline, node, false, system.spatium * 0.6)
        {
            nodes.push(node);
        }
    }
    nodes
}

/// The syllables of `line` that fall in this system, at their beats.
fn place_row(line: &crate::chart::lyrics::LyricLine, system: &LyricSystem<'_>) -> Vec<Placed> {
    let measures = system.section.measures();
    let mut out = Vec::new();
    let mut prev_hyphen = false;
    for syl in &line.syllables {
        let glued = prev_hyphen;
        prev_hyphen = syl.hyphen_after;
        let Some(local) = system
            .measure_indices
            .iter()
            .position(|&m| m == syl.measure_index)
        else {
            continue;
        };
        let measure_x = system.first_measure_x + system.widths[..local].iter().sum::<f64>();
        let width = system.widths[local];
        let beats = measures
            .get(syl.measure_index)
            .map(|m| m.time_signature.0)
            .filter(|&b| b > 0)
            .unwrap_or(4) as f64;
        let target_x = beat_x(measure_x, width, beats, syl.beat as f64, system.spatium);
        let mut text = syl.text.clone();
        if syl.hyphen_after {
            text.push('-');
        }
        out.push(Placed {
            text,
            target_x,
            glued,
        });
    }
    out
}

/// The x of a 0-based beat on an even grid across the bar — the same grid
/// the bar's slashes sit on, near enough for "this word is on beat 3".
fn beat_x(measure_x: f64, width: f64, beats: f64, beat: f64, spatium: f64) -> f64 {
    let usable = (width - spatium).max(0.0);
    let beat = beat.clamp(0.0, (beats - 0.25).max(0.0));
    measure_x + spatium * 0.5 + beat / beats * usable
}

/// Left edges for each word: at its beat when it fits, else right after
/// the word before it; then pulled back from the system's right edge so the
/// last words stay on the page. Words never overlap and never reorder.
fn flow(placed: &[Placed], widths: &[f64], space: f64, right_edge: f64) -> Vec<f64> {
    let mut xs = Vec::with_capacity(placed.len());
    let mut prev_end = f64::NEG_INFINITY;
    for (p, w) in placed.iter().zip(widths) {
        let gap = if p.glued { 0.0 } else { space };
        let x = p.target_x.max(prev_end + gap);
        xs.push(x);
        prev_end = x + w;
    }
    // Backward pass: keep the row inside the system.
    let mut limit = right_edge;
    for i in (0..xs.len()).rev() {
        if xs[i] + widths[i] > limit {
            xs[i] = limit - widths[i];
        }
        let gap = if placed[i].glued { 0.0 } else { space };
        limit = xs[i] - gap;
    }
    xs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, target_x: f64) -> Placed {
        Placed {
            text: text.into(),
            target_x,
            glued: false,
        }
    }

    #[test]
    fn a_word_sits_on_its_beat_when_there_is_room() {
        let placed = [word("Joy", 10.0), word("world", 60.0)];
        assert_eq!(flow(&placed, &[20.0, 30.0], 5.0, 500.0), vec![10.0, 60.0]);
    }

    #[test]
    fn crowded_words_run_on_instead_of_overlapping() {
        let placed = [word("Joy", 10.0), word("to", 12.0), word("the", 14.0)];
        let xs = flow(&placed, &[20.0, 10.0, 15.0], 5.0, 500.0);
        assert_eq!(xs, vec![10.0, 35.0, 50.0]);
    }

    #[test]
    fn the_row_stays_inside_the_system() {
        let placed = [word("one", 80.0), word("two", 90.0)];
        let xs = flow(&placed, &[10.0, 10.0], 2.0, 100.0);
        assert!(xs[1] + 10.0 <= 100.0);
        assert!(xs[0] + 10.0 + 2.0 <= xs[1]);
    }

    #[test]
    fn a_hyphenated_word_is_glued_back_together() {
        let placed = [
            word("re-", 10.0),
            Placed {
                text: "ceive".into(),
                target_x: 10.0,
                glued: true,
            },
        ];
        assert_eq!(flow(&placed, &[15.0, 25.0], 5.0, 500.0), vec![10.0, 25.0]);
    }

    #[test]
    fn beats_spread_across_the_bar() {
        let b1 = beat_x(100.0, 200.0, 4.0, 0.0, 10.0);
        let b3 = beat_x(100.0, 200.0, 4.0, 2.0, 10.0);
        assert_eq!(b1, 105.0);
        assert_eq!(b3, 105.0 + 0.5 * 190.0);
    }
}
