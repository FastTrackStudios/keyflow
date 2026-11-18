//! Note input's bars are the engraved bars: bar `N` of the edit model is
//! the bar the engraver keys `bar#N` (where the pane draws the cursor and
//! where a click reads its bar from), whatever the chart has that is not
//! drawn as bars of its own: a count-in, drawn in the title header as a
//! countdown, does not push the edits two bars before the cursor.

#![cfg(feature = "paint")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use engraver::layout::chart::select::{Kind, Selectable};
use engraver::renderer::{ChartTheme, ChartView};
use keyflow_lang::edit::commit::{bar_text, write_bars};
use keyflow_lang::edit::{InputState, Mode, Op, Pos, Score, Target};
use keyflow_lang::text::chart::parse_chart;

/// Drive To LA's shape: a two-bar count-in, then a chord a line.
const COUNT_IN: &str = "\
Count In
#C 160bpm 4/4

Count 2

VS 4
C
Am
F
G

PRE 2
Dm
G7
";

const NO_COUNT_IN: &str = "\
No Count In
#C 160bpm 4/4

VS 4
C
Am
F
G

PRE 2
Dm
G7
";

fn selectables(source: &str) -> Vec<Selectable> {
    let chart = parse_chart(source).expect("the chart parses");
    let mut view = ChartView::new().unwrap();
    view.set_theme(ChartTheme::default());
    let mut scene = anyrender::Scene::new();
    let _ = view.paint(&mut scene, &chart, 1, 900.0, 1.0, 1.0, (0.0, 0.0), None);
    view.selectables()
}

/// The box the pane draws note input's cursor in for bar `bar`: the first
/// keyed `bar#N` (`paint_view`'s `bar_box`).
fn bar_box(items: &[Selectable], bar: usize) -> kurbo::Rect {
    let (key, part) = (format!("bar#{bar}"), format!("bar#{bar}."));
    items
        .iter()
        .find(|s| s.key == key || s.key.starts_with(&part))
        .unwrap_or_else(|| panic!("no box for bar {bar}"))
        .bounds
}

/// The chord symbol drawn over the bar box `rect` (above its staff).
fn chord_over(items: &[Selectable], rect: kurbo::Rect) -> &Selectable {
    items
        .iter()
        .filter(|s| s.kind == Kind::Chord)
        .find(|s| {
            let c = s.bounds.center();
            c.x > rect.x0 && c.x < rect.x1 && c.y < rect.y1 && c.y > rect.y0 - 3.0 * rect.height()
        })
        .unwrap_or_else(|| panic!("no chord drawn over the bar box {rect:?}"))
}

/// The text of the line chord symbol `chord` was read from.
fn line_of<'a>(source: &'a str, chord: &Selectable) -> &'a str {
    let at = chord.span.as_ref().expect("a chord knows its text").start;
    let start = source[..at].rfind('\n').map_or(0, |n| n + 1);
    let end = source[at..].find('\n').map_or(source.len(), |n| at + n);
    &source[start..end]
}

/// Bar `bar` of the model, as written in `source`.
fn model_bar_text(source: &str, bar: usize) -> String {
    let chart = parse_chart(source).unwrap();
    bar_text(source, &chart, bar).unwrap_or_default()
}

/// Every bar of the model is drawn in the box keyed with its number: the
/// chord symbol in box `bar#N` is read from the model's bar `N`'s text.
fn every_bar_is_drawn_where_the_cursor_goes(source: &str) {
    let chart = parse_chart(source).unwrap();
    let score = Score::from_text(source, &chart);
    let items = selectables(source);
    assert_eq!(score.bars.len(), 6, "the verse's and the pre-chorus's bars");
    for bar in 0..score.bars.len() {
        let chord = chord_over(&items, bar_box(&items, bar));
        assert!(
            line_of(source, chord).starts_with(model_bar_text(source, bar).as_str()),
            "bar {bar}'s box shows `{}`, the model's bar {bar} is `{}`",
            line_of(source, chord),
            model_bar_text(source, bar),
        );
    }
}

#[test]
fn a_count_in_is_not_a_bar_note_input_edits() {
    every_bar_is_drawn_where_the_cursor_goes(COUNT_IN);
}

#[test]
fn a_chart_with_no_count_in_numbers_its_bars_the_same_way() {
    every_bar_is_drawn_where_the_cursor_goes(NO_COUNT_IN);
}

/// The repro: to the pre-chorus's first bar (engraved bar 4, the fifth
/// drawn), a sharp armed, D entered as a chord root: that bar is the one
/// written, and nothing in the verse. (A chord sounds until the next one:
/// D# over the first beat of a bar of Dm holds through it, `D#_1`.)
#[test]
fn a_chord_entered_at_the_cursor_is_written_in_the_bar_the_cursor_is_drawn_in() {
    let chart = parse_chart(COUNT_IN).unwrap();
    let mut score = Score::from_text(COUNT_IN, &chart);
    let mut input = InputState {
        mode: Mode::NoteName,
        target: Target::Chords,
        ..InputState::default()
    };
    input.apply(&mut score, Op::MoveTo(Pos { bar: 4, tick: 0 }));
    input.apply(&mut score, Op::Accidental(1));
    let out = input.apply(&mut score, Op::Letter('D'));
    assert_eq!(out.refused, None);
    assert_eq!(out.changed, [4]);
    assert_eq!(input.selected, Some(Pos { bar: 4, tick: 0 }));
    let text = write_bars(COUNT_IN, &chart, &score, &out.changed).expect("written");

    let changed: Vec<_> = COUNT_IN
        .lines()
        .zip(text.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(
        changed,
        [("Dm", "D#_1")],
        "the pre-chorus's first bar:\n{text}"
    );

    // The cursor (and the selection) was drawn in box `bar#4`: the bar
    // that box shows is the one written.
    let items = selectables(COUNT_IN);
    let drawn = chord_over(&items, bar_box(&items, input.selected.unwrap().bar));
    assert_eq!(line_of(COUNT_IN, drawn), changed[0].0);
}
