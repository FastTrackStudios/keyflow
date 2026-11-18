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
use keyflow_proto::chart::types::Measure;

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
    let measures = measures(chart);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut bars: Vec<usize> = changed.to_vec();
    bars.sort_unstable();
    bars.dedup();
    for &index in &bars {
        let (a, b) = span(&measures, index)?;
        let bar = score
            .bars
            .get(index)
            .ok_or_else(|| format!("There is no bar {}", index + 1))?;
        edits.push((a, b, with_melody(&source[a..b], &bar.to_melody())));
    }
    let text = splice(source, edits);
    check(&text, measures.len(), |after| {
        for &index in &bars {
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
    let mut edits = Vec::new();
    for (k, cell) in cells.iter().enumerate() {
        let (a, b) = span(&measures, from + k)?;
        edits.push((a, b, (*cell).to_owned()));
    }
    let text = splice(source, edits);
    check(&text, measures.len(), |_| Ok(()))?;
    Ok((text, cells.len()))
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

/// `text` read back: it parses, it has `bars` bars, and `more` agrees.
fn check(
    text: &str,
    bars: usize,
    more: impl FnOnce(&[&Measure]) -> Result<(), String>,
) -> Result<(), String> {
    let after = parse_chart(text).map_err(|e| format!("That does not read: {e}"))?;
    let measures = measures(&after);
    if measures.len() != bars {
        return Err(format!(
            "That would make the chart {} bars long instead of {bars}",
            measures.len()
        ));
    }
    more(&measures)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::input::{InputState, Mode, Op};
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
    fn a_bar_with_a_melody_has_its_melody_replaced() {
        let (chart, mut score) = setup();
        let mut input = InputState {
            mode: Mode::NoteName,
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
    fn a_bar_without_its_own_text_is_refused() {
        let text = "Edit - Test\n4/4\n\nVS 4\nC / G / Am / F /\n";
        let chart = parse_chart(text).unwrap();
        let score = Score::from_chart(&chart);
        let err = write_bars(text, &chart, &score, &[0]).unwrap_err();
        assert!(err.contains("not written out"), "{err}");
    }
}
