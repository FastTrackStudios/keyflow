//! An edit written into the chart's text: each bar it changed replaced by
//! its new text and nothing else, so what the author wrote around it (their
//! layout, comments, other bars) stays as they wrote it.
//!
//! A bar finds its text by its `source_span` (the parser's, for every bar
//! written out between barlines). A bar without one is refused, not
//! guessed at. Every write is checked by reading the text back: the chart
//! must have as many bars as before and the bars written must read as
//! they were meant to, or nothing is written.
//!
//! The text is the chart's storage today; when the chart becomes a
//! document of its own, this is the one part that changes: the operations
//! ([`super::input`]) already work on the model.

use keyflow_proto::Chart;
use keyflow_proto::chart::types::{Measure, RhythmElement};

use super::chords::ChordBar;
use super::input::Score;
use super::music::Bar;
use crate::text::chart::parse_chart;
use crate::text::chart::parser::bar_cells;

/// Every bar of `chart`, in order, as the score counts them.
fn measures(chart: &Chart) -> Vec<&Measure> {
    chart
        .sections
        .iter()
        .flat_map(|s| s.measures().iter())
        .collect()
}

/// Bar `index`'s text range in the source (bytes), or why it has none.
fn span(measures: &[&Measure], index: usize) -> Result<(usize, usize), String> {
    let m = measures
        .get(index)
        .ok_or_else(|| format!("There is no bar {}", index + 1))?;
    let s = m.source_span.as_ref().ok_or_else(|| {
        format!(
            "Bar {} is not written out between barlines in the chart's text (it is laid out \
             by beat counting, or repeated from elsewhere), so it cannot be written back yet",
            index + 1
        )
    })?;
    Ok((s.start, s.start + s.len))
}

/// The top-level `m{ … }` in `cell`, as a byte range.
fn melody_range(cell: &str) -> Option<(usize, usize)> {
    let start = cell.find("m{")?;
    let mut depth = 0;
    for (i, c) in cell[start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((start, start + i + 1));
                }
            }
            _ => {}
        }
    }
    None
}

/// `cell` with its chord symbols made `chords` (`F#m_2 B_2`): the chords
/// of `<< chords ; m{ … } >>` replaced, a melody alone given chords beside
/// it, and text written before a bar's chords (`"Build" F`) kept.
fn with_chords(cell: &str, chords: &str) -> String {
    let trimmed = cell.trim();
    if let Some(inner) = trimmed
        .strip_prefix("<<")
        .and_then(|t| t.strip_suffix(">>"))
    {
        return match inner.find(';') {
            Some(at) => format!("<< {chords} ; {} >>", inner[at + 1..].trim()),
            None => format!("<< {chords} >>"),
        };
    }
    if trimmed.starts_with("m{") || trimmed.starts_with("m {") {
        return format!("<< {chords} ; {trimmed} >>");
    }
    // Quoted text and cues written before the chords stay before them.
    let mut keep = String::new();
    let mut rest = trimmed;
    while let Some(after) = rest.strip_prefix('"') {
        let Some(end) = after.find('"') else { break };
        keep.push_str(&rest[..end + 2]);
        keep.push(' ');
        rest = after[end + 1..].trim_start();
    }
    format!("{keep}{chords}")
}

/// `cell` with its melody made `melody`: the melody it has replaced, or
/// one put beside its chords (`<< chords ; m{ … } >>`).
fn with_melody(cell: &str, melody: &str) -> String {
    if let Some((a, b)) = melody_range(cell) {
        return format!("{}{melody}{}", &cell[..a], &cell[b..]);
    }
    let trimmed = cell.trim();
    if let Some(inner) = trimmed
        .strip_prefix("<<")
        .and_then(|t| t.strip_suffix(">>"))
    {
        return format!("<< {} ; {melody} >>", inner.trim());
    }
    format!("<< {trimmed} ; {melody} >>")
}

/// `source` (the text `chart` was read from) with `changed` bars of
/// `score` written into it.
///
/// # Errors
/// A bar not written out in the text, or a text that would not read back
/// as `score` says: the message says which and why, and nothing is
/// written.
pub fn write_bars(
    source: &str,
    chart: &Chart,
    score: &Score,
    changed: &[usize],
) -> Result<String, String> {
    let (text, bars) = bars_text(source, chart, score, changed)?;
    let measures = measures(chart);
    check(&text, &measures, &bars, |after| {
        for &index in &bars {
            let chords = ChordBar::from_measure(after[index]);
            if chords.folded() != score.chords[index].folded() {
                return Err(format!(
                    "Bar {}'s chords would not read back as entered, so they were not written",
                    index + 1
                ));
            }
            let read = Bar::from_measure(after[index]);
            if read.events != score.bars[index].events {
                return Err(format!(
                    "Bar {} would not read back as entered, so it was not written",
                    index + 1
                ));
            }
        }
        Ok(())
    })?;
    Ok(text)
}

/// The text [`write_bars`] would write, not yet checked, and the bars it
/// writes.
///
/// # Errors
/// A bar with no text of its own.
pub fn bars_text(
    source: &str,
    chart: &Chart,
    score: &Score,
    changed: &[usize],
) -> Result<(String, Vec<usize>), String> {
    let measures = measures(chart);
    let mut cells: Vec<(usize, String)> = Vec::new();
    let mut bars: Vec<usize> = changed.to_vec();
    bars.sort_unstable();
    bars.dedup();
    for &index in &bars {
        let (a, b) = span(&measures, index)?;
        let (Some(bar), Some(chords), Some(was)) = (
            score.bars.get(index),
            score.chords.get(index),
            measures.get(index),
        ) else {
            return Err(format!("There is no bar {}", index + 1));
        };
        // A `%` bar is written out as the bar it repeats, then edited.
        let mut own =
            written_out(source, &measures, index).unwrap_or_else(|| source[a..b].to_owned());
        if *chords != ChordBar::from_measure(was) {
            own = with_chords(&own, &chords.to_text()?);
        }
        if *bar != Bar::from_measure(was) {
            own = with_melody(&own, &bar.to_melody());
        }
        cells.push((index, own));
    }
    let text = splice(source, placed(source, &measures, cells)?);
    Ok((text, bars))
}

/// `source` with the bars typed in `typed` (Keyflow's syntax: `C / / / |
/// Am7 / F G`) written over the chart's bars from `from` on: the third
/// input mode. Returns the text and how many bars it wrote.
///
/// # Errors
/// Nothing typed, more bars typed than there are bars to write over, a
/// bar not written out in the text, or a text that does not read (the
/// parser's message), and nothing is written.
pub fn splice_syntax(
    source: &str,
    chart: &Chart,
    from: usize,
    typed: &str,
) -> Result<(String, usize), String> {
    let cells: Vec<&str> = bar_cells(typed)
        .into_iter()
        .map(|(a, len)| &typed[a..a + len])
        .collect();
    if cells.is_empty() {
        return Err("Type one or more bars, separated by |".to_owned());
    }
    let measures = measures(chart);
    if from + cells.len() > measures.len() {
        return Err(format!(
            "{} bars typed, but only {} from bar {} to the end of the chart",
            cells.len(),
            measures.len().saturating_sub(from),
            from + 1
        ));
    }
    let new: Vec<(usize, String)> = cells
        .iter()
        .enumerate()
        .map(|(k, cell)| (from + k, (*cell).to_owned()))
        .collect();
    let text = splice(source, placed(source, &measures, new)?);
    let written: Vec<usize> = (from..from + cells.len()).collect();
    check(&text, &measures, &written, |_| Ok(()))?;
    Ok((text, cells.len()))
}

/// The text edits that give bars `cells` (bar index, new text) their new
/// text. A bar between barlines has its cell replaced. A bar on a line laid
/// out by beat counting (`C F G C`, bars told apart by their length) cannot
/// take a cell that says its own length (`<< F ; m{ … } >>`, `Dm7 G7`), so
/// that line is written with a barline between each of its bars, every bar
/// keeping its own text: `C | << F ; m{ … } >> | G | C`.
fn placed(
    source: &str,
    measures: &[&Measure],
    mut cells: Vec<(usize, String)>,
) -> Result<Vec<(usize, usize, String)>, String> {
    // The `%` bars right after an edited bar repeated it as it was: they
    // are written out as that, so they keep their music.
    let edited: Vec<usize> = cells
        .iter()
        .filter(|(i, text)| span(measures, *i).is_ok_and(|(a, b)| source[a..b] != *text))
        .map(|c| c.0)
        .collect();
    let all: Vec<usize> = cells.iter().map(|c| c.0).collect();
    for &i in &edited {
        let mut j = i + 1;
        while j < measures.len() && !all.contains(&j) && is_simile(source, measures, j) {
            let was = written_out(source, measures, j)
                .ok_or_else(|| format!("Bar {} repeats a bar with no text of its own", j + 1))?;
            cells.push((j, was));
            j += 1;
        }
    }
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut done: Vec<usize> = Vec::new();
    let all: Vec<usize> = cells.iter().map(|c| c.0).collect();
    for index in all {
        if done.contains(&index) {
            continue;
        }
        let (a, _) = span(measures, index)?;
        let line = line_of(source, a);
        // Every bar on that line, in order, with its new text.
        let mut on_line: Vec<(usize, usize, usize, String)> = measures
            .iter()
            .enumerate()
            .filter_map(|(i, m)| {
                let s = m.source_span.as_ref()?;
                let (s, e) = (s.start, s.start + s.len);
                (s >= line.0 && e <= line.1).then(|| {
                    let text = cells
                        .iter()
                        .find(|(j, _)| *j == i)
                        .map_or_else(|| source[s..e].to_owned(), |(_, t)| t.clone());
                    (i, s, e, text)
                })
            })
            .collect();
        on_line.sort_by_key(|x| x.1);
        done.extend(on_line.iter().map(|x| x.0));
        done.push(index);
        if on_line.iter().all(|(_, s, e, t)| source[*s..*e] == *t) {
            // Nothing on this line changes: its text stays as written.
            continue;
        }
        // A line holding a `<< … >>` is read whole, as one, and a `%` there
        // does not repeat the bar before it: its `%`s are written out.
        if on_line.iter().any(|x| x.3.contains("<<")) {
            for x in &mut on_line {
                if is_repeat_text(&x.3) {
                    x.3 = written_out(source, measures, x.0).ok_or_else(|| {
                        format!("Bar {} repeats a bar with no text of its own", x.0 + 1)
                    })?;
                }
            }
        }
        let beat_counted = on_line.len() > 1
            && on_line.windows(2).all(|w| {
                source
                    .get(w[0].2..w[1].1)
                    .is_some_and(|gap| !gap.contains('|'))
            });
        if beat_counted {
            let first = on_line[0].1;
            let last = on_line[on_line.len() - 1].2;
            let joined: Vec<&str> = on_line.iter().map(|x| x.3.as_str()).collect();
            edits.push((first, last, joined.join(" | ")));
        } else {
            for (_, s, e, t) in on_line {
                if source[s..e] != t {
                    edits.push((s, e, t));
                }
            }
        }
    }
    Ok(edits)
}

/// Whether bar `i` is written as the bar before it again: `%`, or `.`
/// (the chord before, for another bar).
fn is_simile(source: &str, measures: &[&Measure], i: usize) -> bool {
    span(measures, i).is_ok_and(|(a, b)| is_repeat_text(&source[a..b]))
}

fn is_repeat_text(text: &str) -> bool {
    matches!(text.trim(), "%" | ".")
}

/// Bar `i`'s text with a `%` written out: the text of the bar it repeats
/// (the last before it that is not `%`). `None` when it is not `%`, or the
/// bar it repeats has no text of its own.
fn written_out(source: &str, measures: &[&Measure], i: usize) -> Option<String> {
    if !is_simile(source, measures, i) {
        return None;
    }
    (0..i).rev().find_map(|k| {
        let (a, b) = span(measures, k).ok()?;
        let text = source[a..b].trim();
        (!is_repeat_text(text)).then(|| text.to_owned())
    })
}

/// The line of `source` that byte `at` is on: `(start, end)`.
fn line_of(source: &str, at: usize) -> (usize, usize) {
    let start = source[..at].rfind('\n').map_or(0, |n| n + 1);
    let end = source[at..].find('\n').map_or(source.len(), |n| at + n);
    (start, end)
}

/// `source` with each `(start, end, text)` put in place, from the last.
fn splice(source: &str, mut edits: Vec<(usize, usize, String)>) -> String {
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    let mut text = source.to_owned();
    for (a, b, new) in edits {
        text.replace_range(a..b, &new);
    }
    text
}

/// `text` read back: it parses, it has as many bars as `before`, every
/// bar but `written` reads as it did, and `more` agrees.
fn check(
    text: &str,
    before: &[&Measure],
    written: &[usize],
    more: impl FnOnce(&[&Measure]) -> Result<(), String>,
) -> Result<(), String> {
    let after = parse_chart(text).map_err(|e| {
        if e.contains("is a bar of its own") {
            "That bar shares its line with a tie or a chord carried over from the bar \
             next to it, so it cannot take notes here yet: put barlines around it in the \
             chart's text first"
                .to_owned()
        } else {
            format!("That does not read: {e}")
        }
    })?;
    let measures = measures(&after);
    if measures.len() != before.len() {
        return Err(format!(
            "That would make the chart {} bars long instead of {}",
            measures.len(),
            before.len()
        ));
    }
    for (i, (a, b)) in before.iter().zip(&measures).enumerate() {
        // A `%` written out (`placed`) is the same bar without the sign.
        // (A `%` copy's chords keep the places of the bar it copies.)
        let written_out = a.simile && !b.simile && {
            let mut x = (*a).clone();
            x.simile = false;
            music_of(&x, false) == music_of(b, false)
        };
        if !written.contains(&i) && !written_out && !same_music(a, b) {
            return Err(format!(
                "That would change bar {} as well, so it was not written",
                i + 1
            ));
        }
    }
    more(&measures)
}

/// Whether two bars hold the same music, however it is written: the same
/// chords at the same places for the same lengths, the same rests, melody,
/// marks, repeats and meter. A bar's text may change (`F` and `F_1` between
/// barlines are one whole-bar F) and its place in the text may move.
#[must_use]
pub fn same_music(a: &Measure, b: &Measure) -> bool {
    // Not their places in the song: an edit that makes an earlier bar
    // longer or shorter moves every bar after it, and they are the same.
    music_of(a, false) == music_of(b, false)
}

/// What a bar holds, as text, without where or how it was written: what
/// [`same_music`] compares.
#[must_use]
pub fn music(m: &Measure) -> String {
    music_of(m, false)
}

/// What a bar holds, as text, without where or how it was written; with
/// its chords' places in the song, or not.
fn music_of(m: &Measure, places: bool) -> String {
    let place = |p: &dyn std::fmt::Debug| {
        if places {
            format!("{p:?}")
        } else {
            String::new()
        }
    };
    use std::fmt::Write as _;
    let mut out = format!(
        "{:?} simile={} repeat={} {:?} {:?} {:?} {:?}\n",
        m.time_signature,
        m.simile,
        m.repeat_count,
        m.start_repeat,
        m.end_repeat,
        m.end_barline,
        m.volta_start
    );
    for c in &m.chords {
        let _ = writeln!(
            out,
            "chord {} {:?} {} {:?} {:?}",
            c.full_symbol,
            c.duration,
            place(&c.position),
            c.push_pull,
            c.commands
        );
    }
    for r in &m.rhythm_elements {
        let _ = match r {
            RhythmElement::Chord(c) => writeln!(
                out,
                "hit {} {:?} {}",
                c.full_symbol,
                c.duration,
                place(&c.position)
            ),
            RhythmElement::Rest(r) => writeln!(out, "rest {:?} {}", r.duration, place(&r.position)),
            RhythmElement::Space(s) => writeln!(out, "space {:?}", s.duration),
        };
    }
    for melody in &m.melodies {
        let notes: Vec<String> = melody.notes.iter().map(ToString::to_string).collect();
        let _ = writeln!(out, "melody {}", notes.join(" "));
    }
    let _ = writeln!(
        out,
        "{:?} {:?} {:?} {:?} {:?} {:?} {:?}",
        m.text_cues,
        m.dynamics,
        m.classical_dynamics,
        m.hairpins,
        m.staff_text,
        m.figured_bass,
        m.suspensions
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::input::{InputState, Mode, Op, Target};
    use crate::edit::music::Value;

    const CHART: &str =
        "Edit - Test\n4/4\n\nVS 4\n| C / / / | Am7 / / / | << F ; m{ a:44 r2. } >> | G / / / |\n";

    fn setup() -> (Chart, Score) {
        let chart = parse_chart(CHART).unwrap();
        let score = Score::from_chart(&chart);
        (chart, score)
    }

    #[test]
    fn every_written_bar_knows_its_text() {
        let (chart, _) = setup();
        let texts: Vec<&str> = measures(&chart)
            .iter()
            .map(|m| {
                let s = m.source_span.as_ref().expect("a span");
                &CHART[s.start..s.start + s.len]
            })
            .collect();
        assert_eq!(
            texts,
            ["C / / /", "Am7 / / /", "<< F ; m{ a:44 r2. } >>", "G / / /"]
        );
    }

    #[test]
    fn notes_entered_are_written_into_their_bar_and_nothing_else() {
        let (chart, mut score) = setup();
        let mut input = InputState {
            mode: Mode::NoteName,
            target: Target::Notes,
            ..InputState::default()
        };
        input.apply(&mut score, Op::MoveTo(crate::edit::Pos { bar: 1, tick: 0 }));
        let mut changed = Vec::new();
        for op in [
            Op::Duration(Value::Quarter),
            Op::Letter('E'),
            Op::Letter('G'),
            Op::Duration(Value::Half),
            Op::Rest,
        ] {
            changed.extend(input.apply(&mut score, op).changed);
        }
        let text = write_bars(CHART, &chart, &score, &changed).unwrap();
        assert!(
            text.contains(
                "| C / / / | << Am7 / / / ; m{ E:54 G:54 r2 } >> | << F ; m{ a:44 r2. } >> |"
            ),
            "{text}"
        );
        // And it reads back as entered.
        let again = Score::from_chart(&parse_chart(&text).unwrap());
        assert_eq!(again.bars[1].events, score.bars[1].events);
    }

    #[test]
    fn chord_symbols_entered_are_written_into_their_bar() {
        let (chart, mut score) = setup();
        let mut input = InputState {
            mode: Mode::NoteName,
            ..InputState::default()
        };
        input.apply(&mut score, Op::MoveTo(crate::edit::Pos { bar: 1, tick: 0 }));
        let mut changed = Vec::new();
        for op in [Op::Duration(Value::Half), Op::Letter('D'), Op::Letter('G')] {
            changed.extend(input.apply(&mut score, op).changed);
        }
        // Then bar 3, which has a melody: its chords change, its melody stays.
        input.apply(&mut score, Op::MoveTo(crate::edit::Pos { bar: 2, tick: 0 }));
        changed.extend(input.apply(&mut score, Op::Duration(Value::Whole)).changed);
        changed.extend(input.apply(&mut score, Op::Letter('A')).changed);
        let text = write_bars(CHART, &chart, &score, &changed).unwrap();
        assert!(
            text.contains("| C / / / | D_2 G_2 | << A_1 ; m{ a:44 r2. } >> |"),
            "{text}"
        );
    }

    #[test]
    fn a_bar_with_a_melody_has_its_melody_replaced() {
        let (chart, mut score) = setup();
        let mut input = InputState {
            mode: Mode::NoteName,
            target: Target::Notes,
            ..InputState::default()
        };
        input.apply(&mut score, Op::MoveTo(crate::edit::Pos { bar: 2, tick: 0 }));
        let out = input.apply(&mut score, Op::Semitone(true));
        let text = write_bars(CHART, &chart, &score, &out.changed).unwrap();
        assert!(text.contains("<< F ; m{ A#:44 r2. } >>"), "{text}");
    }

    #[test]
    fn syntax_typed_at_the_cursor_overwrites_bars_from_there() {
        let (chart, _) = setup();
        let (text, n) = splice_syntax(CHART, &chart, 1, "Dm7 / G7 / | Cmaj7").unwrap();
        assert_eq!(n, 2);
        assert!(
            text.contains("| C / / / | Dm7 / G7 / | Cmaj7 | G / / / |"),
            "{text}"
        );
        let err = splice_syntax(CHART, &chart, 3, "C | D").unwrap_err();
        assert!(err.contains("only 1"), "{err}");
        let err = splice_syntax(CHART, &chart, 0, "  ").unwrap_err();
        assert!(err.contains("Type"), "{err}");
    }

    #[test]
    fn a_bar_laid_out_by_beat_counting_is_found_and_its_line_barlined_on_edit() {
        let text = "Edit - Test\n4/4\n\nVS 4\nC F % G\n";
        let chart = parse_chart(text).unwrap();
        let found: Vec<&str> = measures(&chart)
            .iter()
            .map(|m| {
                let s = m.source_span.as_ref().expect("a span");
                &text[s.start..s.start + s.len]
            })
            .collect();
        assert_eq!(found, ["C", "F", "%", "G"]);
        let mut score = Score::from_chart(&chart);
        let mut input = InputState {
            mode: Mode::NoteName,
            target: Target::Notes,
            ..InputState::default()
        };
        input.apply(&mut score, Op::MoveTo(crate::edit::Pos { bar: 1, tick: 0 }));
        let out = input.apply(&mut score, Op::Letter('A'));
        let new = write_bars(text, &chart, &score, &out.changed).unwrap();
        // Barlined, the `%` written out as the F it repeated.
        assert!(
            new.contains("C | << F ; m{ A:44 r4 r2 } >> | F | G"),
            "{new}"
        );
        // Typed in syntax mode over the `%`, the line is barlined too.
        let (typed, _) = splice_syntax(text, &chart, 2, "Dm7 G7").unwrap();
        assert!(typed.contains("C | F | Dm7 G7 | G"), "{typed}");
    }

    #[test]
    fn a_bar_whose_chord_runs_over_the_barline_is_refused() {
        // G#m_2. starts in bar 1 and ends in bar 2: neither has text alone.
        let text = "Edit - Test\n4/4\n\nVS 2\nC_2 G_2. D_4\n";
        let chart = parse_chart(text).unwrap();
        let score = Score::from_chart(&chart);
        let err = write_bars(text, &chart, &score, &[1]).unwrap_err();
        assert!(err.contains("not written out"), "{err}");
    }

    #[test]
    fn an_edit_that_would_change_another_bar_is_not_written() {
        let text = "Edit - Test\n4/4\n\nVS 4\n| C | F | % | G |\n";
        let chart = parse_chart(text).unwrap();
        let before = measures(&chart)
            .iter()
            .map(|m| music(m))
            .collect::<Vec<_>>();
        let (new, _) = splice_syntax(text, &chart, 1, "Bb").unwrap();
        // The `%` after it repeated F: written out, it still is F.
        let after = parse_chart(&new).unwrap();
        let after = measures(&after);
        assert!(new.contains("| C | Bb | F | G |"), "{new}");
        assert_eq!(music(after[3]), before[3]);
    }
}
