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
use super::input::{Score, replays};
use super::music::Bar;
use crate::text::chart::parse_chart;
use crate::text::chart::parser::bar_cells;

/// Every bar of `chart`, in order, as the score counts them.
fn measures(chart: &Chart) -> Vec<&Measure> {
    super::input::bars(chart)
}

/// Bar `index`'s text range in the source (bytes), or why it has none.
fn span(measures: &[&Measure], index: usize) -> Result<(usize, usize), String> {
    let m = measures
        .get(index)
        .ok_or_else(|| format!("There is no bar {}", index + 1))?;
    let s = m.source_span.as_ref().ok_or_else(|| {
        format!(
            "Bar {} is read-only: {}",
            index + 1,
            m.read_only
                .as_deref()
                .unwrap_or("it has no text of its own in the chart's text")
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
/// `new` (the bar `was` written again) still tied into the next bar when
/// `was` was (`>Fadd2_8 ~`) and still ends on the chord that was tied:
/// the bar's chord line has no tie of its own to write.
fn with_tie(was: &str, new: String) -> String {
    let last_chord = |t: &str| {
        t.split_whitespace()
            .rev()
            .find(|w| *w != "~")
            .map(|w| w.split('_').next().unwrap_or(w).to_owned())
    };
    let tied = was.trim_end().ends_with('~');
    if tied && !new.trim_end().ends_with('~') && last_chord(was) == last_chord(&new) {
        format!("{} ~", new.trim_end())
    } else {
        new
    }
}

/// `chords` in beat slashes (`Gmaj7 ////`, `C // Bdim //`: a chord, then a
/// slash a beat), when the bar `was` written so (slashes, no values) and
/// each chord is a whole number of beats.
fn in_slashes(was: &str, chords: &ChordBar, beat: u32) -> Option<String> {
    let words: Vec<&str> = was.split_whitespace().collect();
    let slashed = words
        .iter()
        .any(|w| !w.is_empty() && w.chars().all(|c| c == '/'));
    if !slashed || words.iter().any(|w| w.contains('_')) || beat == 0 {
        return None;
    }
    let mut out = Vec::new();
    for (slot, ticks) in chords.folded() {
        let super::chords::Slot::Chord(token) = slot else {
            return None;
        };
        if ticks % beat != 0 {
            return None;
        }
        out.push(format!("{token} {}", "/".repeat((ticks / beat) as usize)));
    }
    Some(out.join(" "))
}

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
    // Quoted text and cues written before the chords stay before them:
    // `"…"`, and above or below the staff, `^"…"` and `_"…"`.
    let mut keep = String::new();
    let mut rest = trimmed;
    loop {
        // A one-bar meter (`!T3/4`) written before them stays too.
        if rest.starts_with("!T") {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            keep.push_str(&rest[..end]);
            keep.push(' ');
            rest = rest[end..].trim_start();
            continue;
        }
        let mark = usize::from(rest.starts_with("^\"") || rest.starts_with("_\""));
        let Some(after) = rest[mark..].strip_prefix('"') else {
            break;
        };
        let Some(end) = after.find('"') else { break };
        keep.push_str(&rest[..mark + end + 2]);
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

/// The lanes of `inner` (a `<< >>` block's inside), split at the `;`s
/// outside braces.
fn lanes(inner: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut depth = 0i32;
    for c in inner.chars() {
        match c {
            '{' | '<' => depth += 1,
            '}' | '>' => depth -= 1,
            ';' if depth == 0 => {
                out.push(String::new());
                continue;
            }
            _ => {}
        }
        out.last_mut().expect("one lane at least").push(c);
    }
    out.into_iter().map(|l| l.trim().to_owned()).collect()
}

/// `cell` with voice `voice`'s melody made `melody`. Voice 1 is the bar's
/// melody ([`with_melody`]); a lower voice is the lane of `<< … ; m{ … } ;
/// m{ … } >>` that holds that voice, added when the bar has none, with
/// `filler` (a bar of rest) for any voice before it that is missing.
fn with_voice(cell: &str, voice: u8, melody: &str, filler: &str) -> String {
    if voice == 0 {
        return with_melody(cell, melody);
    }
    let trimmed = cell.trim();
    let mut parts = match trimmed
        .strip_prefix("<<")
        .and_then(|t| t.strip_suffix(">>"))
    {
        Some(inner) => lanes(inner),
        None => vec![trimmed.to_owned()],
    };
    // The lanes that hold a melody, in order: voice 1's, voice 2's, …
    let with_melody_at: Vec<usize> = (0..parts.len())
        .filter(|&i| melody_range(&parts[i]).is_some())
        .collect();
    let v = usize::from(voice);
    if let Some(&i) = with_melody_at.get(v) {
        let (a, b) = melody_range(&parts[i]).expect("a melody");
        parts[i] = format!("{}{melody}{}", &parts[i][..a], &parts[i][b..]);
    } else {
        for _ in with_melody_at.len()..v {
            parts.push(filler.to_owned());
        }
        parts.push(melody.to_owned());
    }
    let parts: Vec<&str> = parts
        .iter()
        .map(String::as_str)
        .filter(|p| !p.is_empty())
        .collect();
    format!("<< {} >>", parts.join(" ; "))
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
    // In the bar's own style first: a bar written in beat slashes
    // (`G ////`) is written in them again (`Gmaj7 ////`), else with values.
    match bars_written(source, chart, score, changed, true) {
        Ok(text) => Ok(text),
        Err(_) => bars_written(source, chart, score, changed, false),
    }
}

/// [`write_bars`], the chords of a bar written in beat slashes written in
/// them again when `slashes`, else with values.
fn bars_written(
    source: &str,
    chart: &Chart,
    score: &Score,
    changed: &[usize],
    slashes: bool,
) -> Result<String, String> {
    let (text, bars) = bars_text_in(source, chart, score, changed, slashes)?;
    let measures = measures(chart);
    check(&text, &measures, &replays(chart), &bars, |after| {
        for &index in &bars {
            let chords = ChordBar::from_measure(after[index]);
            if chords.folded() != score.chords[index].folded() {
                return Err(format!(
                    "Bar {}'s chords would not read back as entered, so they were not written",
                    index + 1
                ));
            }
            for voice in 0..4u8 {
                let Some(want) = score.voice_bars(voice).get(index) else {
                    continue;
                };
                let read = Bar::from_voice(after[index], voice);
                if read.events != want.events {
                    return Err(format!(
                        "Bar {}{} would not read back as entered, so it was not written",
                        index + 1,
                        if voice == 0 {
                            String::new()
                        } else {
                            format!(" (voice {})", voice + 1)
                        }
                    ));
                }
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
    bars_text_in(source, chart, score, changed, false)
}

fn bars_text_in(
    source: &str,
    chart: &Chart,
    score: &Score,
    changed: &[usize],
    slashes: bool,
) -> Result<(String, Vec<usize>), String> {
    let measures = measures(chart);
    let prefixes = prefixes(source, chart);
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
        let mut own = written_out(source, &measures, index)
            .or_else(|| own_text(source, &measures, &prefixes, index))
            .unwrap_or_else(|| source[a..b].to_owned());
        if *chords != ChordBar::from_measure(was) {
            let beat = super::music::TICKS_PER_WHOLE / u32::from(was.time_signature.1.max(1));
            let written = slashes
                .then(|| in_slashes(&own, chords, beat))
                .flatten()
                .map_or_else(|| chords.to_text(), Ok)?;
            own = with_tie(&own, with_chords(&own, &written));
        }
        let _ = bar;
        for voice in 0..4u8 {
            let Some(line) = score.voice_bars(voice).get(index) else {
                continue;
            };
            if *line != Bar::from_voice(was, voice) {
                let filler = Bar::empty(was.time_signature).to_melody();
                own = with_voice(&own, voice, &line.to_melody(), &filler);
            }
        }
        cells.push((index, own));
    }
    let text = splice(source, placed(source, &measures, &prefixes, cells)?);
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
    // A repeat sign is a barline, between the bars: typed here it was
    // read as a plain barline and the repeat dropped without a word.
    if has_repeat_sign(typed) {
        return Err(
            "Repeat signs (|: :| x4) are not written from here yet: the bars were not written, so the repeat is not lost. Type the bars without them"
                .to_owned(),
        );
    }
    let measures = measures(chart);
    if from > measures.len() {
        return Err(format!("There is no bar {}", from + 1));
    }
    // Bars typed past the chart's end are added after it (MuseScore adds
    // bars as note input runs off the end), so a chart can be sketched
    // from nothing.
    let fit = (measures.len() - from).min(cells.len());
    let new: Vec<(usize, String)> = cells[..fit]
        .iter()
        .enumerate()
        .map(|(k, cell)| (from + k, (*cell).to_owned()))
        .collect();
    let prefixes = prefixes(source, chart);
    let mut edits = placed(source, &measures, &prefixes, new)?;
    edits.extend(appended(source, chart, &measures, &cells[fit..]));
    let text = splice(source, edits);
    let written: Vec<usize> = (from..from + cells.len()).collect();
    check(&text, &measures, &replays(chart), &written, |_| Ok(()))?;
    Ok((text, cells.len()))
}

/// Whether `typed` writes a repeat barline (`|:`, `:|`, `:|x4`) outside a
/// melody or a parallel block.
fn has_repeat_sign(typed: &str) -> bool {
    let bytes = typed.as_bytes();
    let mut depth = 0i32;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => depth = (depth - 1).max(0),
            b'<' if bytes.get(i + 1) == Some(&b'<') => {
                depth += 1;
                i += 1;
            }
            b'>' if bytes.get(i + 1) == Some(&b'>') => {
                depth = (depth - 1).max(0);
                i += 1;
            }
            b'"' => {
                // A cue's text: a colon in it is not a barline.
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += 1;
                }
            }
            b'|' if depth == 0 && bytes.get(i + 1) == Some(&b':') => return true,
            b':' if depth == 0 && bytes.get(i + 1) == Some(&b'|') => return true,
            _ => {}
        }
        i += 1;
    }
    false
}

/// The text edits that add `extra` bars after the chart's last: on a line
/// of their own after the last bar's, the last section's bar count (`vs
/// 8`) raised to take them; or, in a chart with no bars, a section of
/// their own.
fn appended(
    source: &str,
    chart: &Chart,
    measures: &[&Measure],
    extra: &[&str],
) -> Vec<(usize, usize, String)> {
    if extra.is_empty() {
        return Vec::new();
    }
    let line = format!("| {} |", extra.join(" | "));
    let Some(last) = measures.last().and_then(|m| m.source_span.as_ref()) else {
        let end = source.trim_end().len();
        return vec![(end, source.len(), format!("\n\nvs\n{line}\n"))];
    };
    let end = last.start + last.len;
    let line_end = source[end..].find('\n').map_or(source.len(), |i| end + i);
    let mut edits = vec![(line_end, line_end, format!("\n{line}"))];
    // `vs 8` → `vs 12`: a header that counts its bars counts these too.
    if let Some(header) = chart.sections.last().and_then(|s| s.source_span.as_ref()) {
        let text = &source[header.start..header.start + header.len];
        let trimmed = text.trim_end();
        let digits = trimmed.len() - trimmed.trim_end_matches(|c: char| c.is_ascii_digit()).len();
        let before_digits = &trimmed[..trimmed.len() - digits];
        if digits > 0
            && before_digits.ends_with(char::is_whitespace)
            && let Ok(n) = trimmed[trimmed.len() - digits..].parse::<usize>()
        {
            let at = header.start + trimmed.len() - digits;
            edits.push((at, at + digits, (n + extra.len()).to_string()));
        }
    }
    edits
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
    prefixes: &[String],
    mut cells: Vec<(usize, String)>,
) -> Result<Vec<(usize, usize, String)>, String> {
    // The `%` bars right after an edited bar repeated it as it was: they
    // are written out as that, so they keep their music.
    let edited: Vec<usize> = cells
        .iter()
        .filter(|(i, text)| {
            own_text(source, measures, prefixes, *i).is_some_and(|own| own != *text)
        })
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
        let mut on_line: Vec<(usize, usize, usize, String, String)> = measures
            .iter()
            .enumerate()
            .filter_map(|(i, m)| {
                let s = m.source_span.as_ref()?;
                let (s, e) = (s.start, s.start + s.len);
                (s >= line.0 && e <= line.1).then(|| {
                    let own = own_text(source, measures, prefixes, i).unwrap_or_default();
                    let text = cells
                        .iter()
                        .find(|(j, _)| *j == i)
                        .map_or_else(|| own.clone(), |(_, t)| t.clone());
                    (i, s, e, text, own)
                })
            })
            .collect();
        on_line.sort_by_key(|x| (x.1, x.0));
        done.extend(on_line.iter().map(|x| x.0));
        done.push(index);
        if on_line.iter().all(|x| x.3 == x.4) {
            // Nothing on this line changes: its text stays as written.
            continue;
        }
        // One item per text in the line: a `%3`'s bars share theirs, and
        // are written out one `%` each, barlined, when one of them changes.
        // And a run of bars a chord is held across shares its text: written
        // out bar by bar when one of them changes (each other bar as it
        // reads alone, `alone`), kept as written when none does.
        let mut items: Vec<(usize, usize, String, bool)> = Vec::new();
        // The first bar of each item with no text alone, if any.
        let mut silent: Vec<Option<usize>> = Vec::new();
        for x in &on_line {
            let changed = x.3 != x.4;
            let none = x.3.is_empty().then_some(x.0);
            match items.last_mut() {
                Some(last) if last.0 == x.1 && last.1 == x.2 => {
                    last.2.push_str(" | ");
                    last.2.push_str(&x.3);
                    last.3 |= changed;
                    if let Some(s) = silent.last_mut() {
                        *s = s.or(none);
                    }
                }
                _ => {
                    items.push((x.1, x.2, x.3.clone(), changed));
                    silent.push(none);
                }
            }
        }
        for (item, none) in items.iter().zip(&silent) {
            if item.3
                && let Some(bar) = none
            {
                return Err(format!("Bar {} is read-only: {PUSHED_OVER}", bar + 1));
            }
        }
        let beat_counted = items.len() > 1
            && items.windows(2).all(|w| {
                source
                    .get(w[0].1..w[1].0)
                    .is_some_and(|gap| !gap.contains('|'))
            });
        for item in &mut items {
            // A text no bar of which changes stays as written (a `%3`, a
            // chord held across bars). But a line counted by beats is
            // written barlined when a bar of it changes, and between
            // barlines `%3` is a three-bar repeat, not the bar before three
            // times: its bars stay `% | % | %`.
            let text = &source[item.0..item.1];
            let multi_repeat = text
                .trim()
                .strip_prefix('%')
                .is_some_and(|n| n.parse::<u32>().is_ok_and(|n| n > 1));
            if !item.3 && !(beat_counted && multi_repeat) {
                item.2 = text.to_owned();
            }
        }
        if beat_counted {
            let first = items[0].0;
            let last = items[items.len() - 1].1;
            let joined: Vec<&str> = items.iter().map(|x| x.2.as_str()).collect();
            edits.push((first, last, joined.join(" | ")));
        } else {
            for (s, e, t, _) in items {
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
    let t = text.trim();
    t == "."
        || t.strip_prefix('%')
            .is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()))
}

/// A bar's text as written alone: a `%3`'s bars are each `%`; a bar a
/// chord is held into from the bar before (or out of, into the next) is
/// the text it reads the same as alone ([`alone`]), if it has one.
fn own_text(source: &str, measures: &[&Measure], prefixes: &[String], i: usize) -> Option<String> {
    let (a, b) = span(measures, i).ok()?;
    let text = &source[a..b];
    let shared = measures.iter().enumerate().any(|(j, m)| {
        j != i
            && m.source_span
                .is_some_and(|s| s.start == a && s.start + s.len == b)
    });
    if !shared {
        return Some(text.to_owned());
    }
    if is_repeat_text(text) {
        return Some("%".to_owned());
    }
    // The run it is in: the bars before and after it with this text.
    let has = |j: usize| {
        measures[j]
            .source_span
            .is_some_and(|s| s.start == a && s.start + s.len == b)
    };
    let first = (0..=i).rev().take_while(|&j| has(j)).last()?;
    let last = (i..measures.len()).take_while(|&j| has(j)).last()?;
    let run: Vec<&Measure> = measures[first..=last].to_vec();
    let prefix = prefixes.get(i)?;
    // This bar as it reads alone, between rest bars, if it does: what holds
    // whichever bar of the run is edited. Else as the whole run reads
    // written out (a push that takes its time from the bar before). An
    // edit is read back whole either way, and refused when another bar
    // would change.
    write_out_run(prefix, text, &[measures[i]])
        .and_then(|mut t| t.pop())
        .or_else(|| write_out_run(prefix, text, &run)?.get(i - first).cloned())
}

/// Why a bar of a run that shares one text cannot be edited, when it has
/// no text alone: in these charts, always a chord pushed over its barline.
pub const PUSHED_OVER: &str = "a chord is pushed over its barline (`'6m` on the and of 4, anticipating the next bar), and written out bar by bar the push would read differently: it stays as written for now";

/// The bars of `chart` (read from `source`) that share one text and have
/// none alone, read-only with why: those an edit is refused for.
#[must_use]
pub fn read_only_runs(source: &str, chart: &Chart) -> Vec<Option<String>> {
    let measures = measures(chart);
    let prefixes = prefixes(source, chart);
    (0..measures.len())
        .map(|i| {
            let shared = measures[i].source_span.is_some_and(|s| {
                measures
                    .iter()
                    .enumerate()
                    .any(|(j, o)| j != i && o.source_span == Some(s))
            });
            (shared && own_text(source, &measures, &prefixes, i).is_none())
                .then(|| PUSHED_OVER.to_owned())
        })
        .collect()
}

/// Bar `i`'s text as it would be written alone (a `%3`'s bars each `%`,
/// a bar a chord is held into the text it reads the same as alone), or
/// `None` when it has none.
#[must_use]
pub fn bar_text(source: &str, chart: &Chart, i: usize) -> Option<String> {
    own_text(source, &measures(chart), &prefixes(source, chart), i)
}

/// For each bar, a chart of one bar in its place: the chart's header (its
/// key, meter, tempo) and its section's header, made one bar long. What
/// [`alone`] reads a bar's text in.
fn prefixes(source: &str, chart: &Chart) -> Vec<String> {
    let first = chart
        .sections
        .iter()
        .find_map(|s| s.source_span.as_ref().map(|sp| sp.start))
        .unwrap_or(0);
    let header = source[..first].trim_end();
    super::input::sections(chart)
        .flat_map(|s| {
            let line = s.source_span.as_ref().map_or("VS", |sp| {
                let (a, b) = line_of(source, sp.start);
                source[a..b].trim()
            });
            // Its bar count left to fill in (`\u{0}`): a run, and a rest bar
            // each side for what is pushed or held over its barlines.
            let mut done = false;
            let one: Vec<String> = line
                .split_whitespace()
                .map(|w| {
                    if !done && w.chars().all(|c| c.is_ascii_digit()) {
                        done = true;
                        "\u{0}".to_owned()
                    } else {
                        w.to_owned()
                    }
                })
                .collect();
            let mut line = one.join(" ");
            if !done {
                line.push_str(" \u{0}");
            }
            let prefix = format!("{header}\n\n{line}\n");
            std::iter::repeat_n(prefix, s.measures().len())
        })
        .collect()
}

/// The texts `m` might be written as alone, best first: what was written
/// in it (`part`), its chords one after another with beat slashes
/// (`E /// G#m`), with their values (`E_2. G#m_4`); each with its cue
/// before it too, when it has one the text does not.
fn candidates(m: &Measure, part: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = part.map(str::to_owned).into_iter().collect();
    let chords = ChordBar::from_measure(m);
    let runs = if chords.locked.is_some() {
        Vec::new()
    } else {
        chords.folded()
    };
    let beat = super::music::TICKS_PER_WHOLE / u32::from(m.time_signature.1.max(1));
    // Beat slashes: every chord but the last a whole number of beats.
    if !runs.is_empty()
        && runs
            .iter()
            .all(|(slot, _)| matches!(slot, super::chords::Slot::Chord(_)))
        && runs[..runs.len() - 1]
            .iter()
            .all(|(_, t)| beat > 0 && t % beat == 0)
    {
        let words: Vec<String> = runs
            .iter()
            .enumerate()
            .filter_map(|(k, (slot, t))| {
                let super::chords::Slot::Chord(token) = slot else {
                    return None;
                };
                Some(if k + 1 == runs.len() {
                    token.clone()
                } else {
                    format!("{token} {}", "/".repeat((t / beat) as usize))
                })
            })
            .collect();
        out.push(words.join(" "));
    }
    if chords.locked.is_none()
        && let Ok(text) = chords.to_text()
    {
        out.push(text);
    }
    let cues: String = m
        .staff_text
        .iter()
        .map(|t| format!("\"{}\" ", t.text))
        .collect();
    let mut seen = std::collections::HashSet::new();
    out.into_iter()
        .flat_map(|c| {
            if cues.is_empty() || c.contains('"') {
                vec![c]
            } else {
                vec![format!("{cues}{c}"), c]
            }
        })
        .filter(|c| seen.insert(c.clone()))
        .collect()
}

/// Whether `b` (read back) is `m`'s music. Its repeats, barlines, line
/// cues (`@bass "Bass in"`) and dynamics (`dyn mp`) are written around a
/// run's text, not in it,
/// and stay as they are; the write is read back whole, cues and all, so
/// this only picks the text.
fn same_in_run(m: &Measure, b: &Measure) -> bool {
    let mut x = m.clone();
    x.text_cues.clone_from(&b.text_cues);
    x.dynamics.clone_from(&b.dynamics);
    x.classical_dynamics.clone_from(&b.classical_dynamics);
    x.hairpins.clone_from(&b.hairpins);
    x.repeat_count = b.repeat_count;
    x.start_repeat = b.start_repeat;
    x.end_repeat = b.end_repeat;
    x.end_barline = b.end_barline;
    x.volta_start.clone_from(&b.volta_start);
    music_of(&x, false) == music_of(b, false)
}

thread_local! {
    /// Runs already written out, by their chart prefix and text.
    static RUNS: std::cell::RefCell<std::collections::HashMap<(String, String), Option<Vec<String>>>> =
        std::cell::RefCell::default();
}

/// A run of bars that share one text (a chord held or pushed over their
/// barlines) written out bar by bar so that each reads as it did, read in
/// `prefix` ([`prefixes`]) between a rest bar each side: the text that
/// starts in each bar, barlined, if that reads the same; else, bar by bar,
/// another way of writing the ones that do not. `None` when some bar has
/// no way (one that starts on the chord before it, which no token says
/// alone).
fn write_out_run(prefix: &str, text: &str, run: &[&Measure]) -> Option<Vec<String>> {
    // The run's own parts too: a bar alone and its whole run share a text.
    let parts_key: String = run
        .iter()
        .map(|m| {
            format!(
                "{:?}{};",
                m.own_part.as_ref().map(|p| (p.start, p.len)),
                music_of(m, false)
            )
        })
        .collect();
    let key = (prefix.to_owned(), format!("{text}\u{1}{parts_key}"));
    if let Some(done) = RUNS.with(|r| r.borrow().get(&key).cloned()) {
        return done;
    }
    let parts: Vec<Option<&str>> = run
        .iter()
        .map(|m| {
            m.own_part.as_ref().and_then(|p| {
                text.get(p.start..p.start + p.len)
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
            })
        })
        .collect();
    let options: Vec<Vec<String>> = run
        .iter()
        .zip(&parts)
        .map(|(m, p)| candidates(m, *p))
        .collect();
    let result = (|| {
        if options.iter().any(Vec::is_empty) {
            return None;
        }
        let prefix = prefix.replace('\u{0}', &(run.len() + 2).to_string());
        // Every way of writing the run, those nearest to as written first
        // (the fewest bars written another way), at most so many.
        let mut combos: Vec<Vec<usize>> = vec![Vec::new()];
        for o in &options {
            combos = combos
                .into_iter()
                .flat_map(|c| {
                    (0..o.len()).map(move |k| {
                        let mut c = c.clone();
                        c.push(k);
                        c
                    })
                })
                .take(4096)
                .collect();
        }
        combos.sort_by_key(|c| {
            (
                c.iter().filter(|&&k| k > 0).count(),
                c.iter().sum::<usize>(),
            )
        });
        combos.into_iter().take(256).find_map(|pick| {
            let cells: Vec<&str> = pick
                .iter()
                .zip(&options)
                .map(|(k, o)| o[*k].as_str())
                .collect();
            let chart =
                parse_chart(&format!("{prefix}| r1 | {} | r1 |\n", cells.join(" | "))).ok()?;
            let bars = measures(&chart);
            (bars.len() == run.len() + 2
                && (0..run.len()).all(|k| same_in_run(run[k], bars[k + 1])))
            .then(|| cells.into_iter().map(str::to_owned).collect())
        })
    })();
    RUNS.with(|r| r.borrow_mut().insert(key, result.clone()));
    result
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
        // Its music: a cue written before the bar it repeats is that bar's.
        (!is_repeat_text(text)).then(|| without_cues(text).to_owned())
    })
}

/// `text` without the cues written before its music (`"…"`, `^"…"`,
/// `_"…"`).
fn without_cues(text: &str) -> &str {
    let mut rest = text.trim();
    loop {
        let mark = usize::from(rest.starts_with("^\"") || rest.starts_with("_\""));
        let Some(after) = rest[mark..].strip_prefix('"') else {
            return rest;
        };
        let Some(end) = after.find('"') else {
            return rest;
        };
        rest = after[end + 1..].trim_start();
    }
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
/// bar but `written` reads as it did, and `more` agrees. `replays` is
/// `before`'s bars' [`replays`]: a bar that plays a written one follows it.
fn check(
    text: &str,
    before: &[&Measure],
    replays: &[Option<usize>],
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
    // Bars written past the end were added.
    let grown = written.iter().filter(|&&i| i >= before.len()).count();
    if measures.len() != before.len() + grown {
        return Err(format!(
            "That would make the chart {} bars long instead of {}",
            measures.len(),
            before.len() + grown
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
        // A bar that plays a written bar again follows it.
        // (An `x2` copy of a line's last bar is it but for the `x2`, which
        // that bar carries.)
        let follows = replays.get(i).copied().flatten().is_some_and(|g| {
            written.contains(&g) && measures.get(g).is_some_and(|w| plays_again(b, w))
        });
        if !written.contains(&i) && !written_out && !follows && !same_music(a, b) {
            return Err(format!(
                "That would change bar {} as well, so it was not written",
                i + 1
            ));
        }
    }
    more(&measures)
}

/// Whether bar `copy` plays bar `src` again. An `x2` copy of a line plays
/// its first time through but for what is written once on it: the `x2`
/// itself (its last bar's), and the dynamics and cues at the line's start.
#[must_use]
pub fn plays_again(copy: &Measure, src: &Measure) -> bool {
    if copy.plays_back.is_none() {
        return same_music(copy, src);
    }
    let mut c = copy.clone();
    c.repeat_count = src.repeat_count;
    c.dynamics.clone_from(&src.dynamics);
    c.classical_dynamics.clone_from(&src.classical_dynamics);
    c.hairpins.clone_from(&src.hairpins);
    c.text_cues.clone_from(&src.text_cues);
    c.staff_text.clone_from(&src.staff_text);
    same_music(&c, src)
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
    #[test]
    fn a_repeat_sign_typed_is_refused_not_dropped() {
        let src = "Song - Test\n#C 120bpm 4/4\n\nVS 4\nC\nC\nC\nC\n";
        let chart = crate::text::chart::parse_chart(src).unwrap();
        for typed in ["|: G :|", "|: G | Am :|x4", "G :| F"] {
            let why = splice_syntax(src, &chart, 1, typed).unwrap_err();
            assert!(why.contains("Repeat signs"), "{typed}: {why}");
        }
        // A colon in a cue, a barline in a melody and plain barlines are
        // none of them repeats.
        for typed in ["\"Vox: in\" G", "G | Am", "<< G ; m{ C:44 D:44 E:42 } >>"] {
            assert!(splice_syntax(src, &chart, 1, typed).is_ok(), "{typed}");
        }
    }

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
    fn a_second_voice_is_written_into_a_lane_of_its_own() {
        let (chart, mut score) = setup();
        let mut input = InputState {
            mode: Mode::NoteName,
            target: Target::Notes,
            ..InputState::default()
        };
        // Bar 3 has voice 1 (`a:44 r2.`); voice 2 gets two halves.
        input.apply(&mut score, Op::MoveTo(crate::edit::Pos { bar: 2, tick: 0 }));
        let mut changed = Vec::new();
        for op in [
            Op::Voice(1),
            Op::Duration(Value::Half),
            Op::Letter('C'),
            Op::Letter('D'),
        ] {
            changed.extend(input.apply(&mut score, op).changed);
        }
        assert_eq!(input.voice, 1);
        let text = write_bars(CHART, &chart, &score, &changed).unwrap();
        assert!(
            text.contains("<< F ; m{ a:44 r2. } ; m{ C:52 D:52 } >>"),
            "{text}"
        );
        // Both voices read back: voice 1 as it was, voice 2 as entered.
        let back = parse_chart(&text).unwrap();
        let bar = measures(&back)[2];
        assert_eq!(
            Bar::from_voice(bar, 0),
            Bar::from_measure(measures(&chart)[2])
        );
        assert_eq!(
            Bar::from_voice(bar, 1).events,
            score.voice_bars(1)[2].events
        );
        assert_eq!(measures(&back).len(), 4, "no bar added");

        // A bar with no melody: voice 1 is written as a bar of rest first.
        let mut score = Score::from_chart(&back);
        input.apply(&mut score, Op::MoveTo(crate::edit::Pos { bar: 3, tick: 0 }));
        let out = input.apply(&mut score, Op::Letter('E'));
        let text2 = write_bars(&text, &back, &score, &out.changed).unwrap();
        assert!(text2.contains("<< G / / / ; m{ r1 } ; m{ E:"), "{text2}");
    }

    #[test]
    fn bars_typed_past_the_end_are_added() {
        // Into a blank chart: a section of their own.
        let blank = "Sketch\n4/4\n";
        let chart = parse_chart(blank).unwrap();
        let (text, n) = splice_syntax(blank, &chart, 0, "C / / / | Am / / / | F / G /").unwrap();
        assert_eq!(n, 3);
        assert_eq!(measures(&parse_chart(&text).unwrap()).len(), 3, "{text}");
        // Over the last two bars and on past the end: the count goes up.
        let (text2, _) = splice_syntax(
            CHART,
            &setup().0,
            2,
            "Dm / / / | G / / / | C / / / | E7 / / /",
        )
        .unwrap();
        let back = parse_chart(&text2).unwrap();
        assert_eq!(measures(&back).len(), 6, "{text2}");
        assert!(text2.contains("VS 6"), "{text2}");
        assert!(
            text2.contains("| Dm / / / | G / / / |\n| C / / / | E7 / / / |"),
            "{text2}"
        );
    }

    #[test]
    fn a_bar_a_push_crosses_into_has_its_own_text_and_is_edited() {
        // Know Me's intro: `'6m` anticipates the next bar's downbeat from
        // the bar before's last eighth. Each bar has its own text (the
        // parser opens and takes back a bar as it reads a push), so a bar
        // is written in place, and the push stays across its barline.
        let text = "Edit - Test\n#E 127bpm 4/4\n\nIN 8\n4 '6m 4 '6m // 5 //\n4 '6m // 5 // 2m 2m // 2m_8 1/3_4.\n";
        let chart = parse_chart(text).unwrap();
        let score = Score::from_text(text, &chart);
        assert!(
            score.read_only.iter().all(Option::is_none),
            "{:?}",
            score.read_only
        );
        let b = (0..measures(&chart).len())
            .find(|&i| bar_text(text, &chart, i).as_deref() == Some("'6m // 5 //"))
            .expect("a bar written `'6m // 5 //`");
        // The 5 after the pushed 6m: made a 5maj7, written in place.
        let mut score = score;
        let mut input = InputState::default();
        assert!(score.chords[b].locked.is_none(), "{:?}", score.chords[b]);
        let at = score.chords[b].starts()[1];
        input.selected = Some(crate::edit::Pos { bar: b, tick: at });
        let out = input.set_quality(&mut score, "maj7");
        let new = write_bars(text, &chart, &score, &out.changed).unwrap();
        assert!(new.contains("'6m // 5maj7 //"), "{new}");
        // The pushed 6m retyped 2m: its push kept, the bar before as it was.
        let chart = parse_chart(&new).unwrap();
        let mut score = Score::from_text(&new, &chart);
        input.selected = Some(crate::edit::Pos { bar: b, tick: 0 });
        let out = input.set_chord(&mut score, "2m");
        let new = write_bars(&new, &chart, &score, &out.changed).unwrap();
        assert!(new.contains("'2m // 5maj7 //"), "{new}");
        // Its push taken off would change the bar before: refused, and why.
        let chart = parse_chart(&new).unwrap();
        let mut score = Score::from_text(&new, &chart);
        let out = input.toggle_mark(&mut score, "'");
        assert!(out.changed.is_empty());
        assert!(out.refused.is_some_and(|r| r.contains("bar before")));
    }

    #[test]
    fn an_entry_on_a_replayed_bar_is_written_where_the_bar_it_plays_is() {
        // The second VS has no bars of its own: it plays the first's.
        let text = "Edit - Test\n4/4\n\nVS 2\n| C | G |\n\nCH 1\n| F |\n\nVS\n";
        let chart = parse_chart(text).unwrap();
        let mut score = Score::from_text(text, &chart);
        assert_eq!(score.replays, [None, None, None, Some(0), Some(1)]);
        let mut input = InputState {
            mode: Mode::NoteName,
            ..InputState::default()
        };
        input.apply(&mut score, Op::MoveTo(crate::edit::Pos { bar: 4, tick: 0 }));
        assert!(input.status(&score).contains("plays bar 2 again"));
        input.apply(&mut score, Op::Duration(Value::Whole));
        let out = input.apply(&mut score, Op::Letter('A'));
        assert_eq!(out.changed, [1]);
        assert!(out.note.is_some_and(|n| n.contains("Bar 5 plays bar 2")));
        let new = write_bars(text, &chart, &score, &out.changed).unwrap();
        assert!(new.contains("| C | A_1 |"), "{new}");
        // Read again, the replay plays the new bar.
        let back = parse_chart(&new).unwrap();
        assert_eq!(
            ChordBar::from_measure(measures(&back)[4])
                .to_text()
                .unwrap(),
            "A_1"
        );
    }

    #[test]
    fn an_entry_on_a_line_s_x2_copy_is_written_in_its_first_time_through() {
        // `x2` plays the line twice: bars 3 and 4 are bars 1 and 2 again.
        let text = "Edit - Test\n4/4\n\nVS 4\n| C | G | x2\n";
        let chart = parse_chart(text).unwrap();
        let mut score = Score::from_text(text, &chart);
        assert_eq!(score.replays, [None, None, Some(0), Some(1)]);
        let mut input = InputState {
            mode: Mode::NoteName,
            ..InputState::default()
        };
        input.apply(&mut score, Op::MoveTo(crate::edit::Pos { bar: 3, tick: 0 }));
        assert!(input.status(&score).contains("plays bar 2 again"));
        input.apply(&mut score, Op::Duration(Value::Whole));
        let out = input.apply(&mut score, Op::Letter('A'));
        assert_eq!(out.changed, [1]);
        let new = write_bars(text, &chart, &score, &out.changed).unwrap();
        assert!(new.contains("x2"), "{new}");
        // Read again, both times through play the new bar.
        let back = parse_chart(&new).unwrap();
        for bar in [1, 3] {
            assert_eq!(
                ChordBar::from_measure(measures(&back)[bar])
                    .to_text()
                    .unwrap(),
                "A_1",
                "bar {}: {new}",
                bar + 1
            );
        }
    }

    #[test]
    fn a_count_in_is_not_one_of_the_bars_note_input_edits() {
        // Drawn in the header as a countdown, the count-in has no measure
        // number: bar 0 is the verse's first, as the engraver numbers it,
        // and a replay counts the same bars.
        let text = "Edit - Test\n4/4\n\nCount 2\n\nVS 2\n| C | G |\n\nCH 1\n| F |\n\nVS\n";
        let chart = parse_chart(text).unwrap();
        let mut score = Score::from_text(text, &chart);
        assert_eq!(score.bars.len(), 5);
        assert_eq!(score.replays, [None, None, None, Some(0), Some(1)]);
        let mut input = InputState {
            mode: Mode::NoteName,
            ..InputState::default()
        };
        input.apply(&mut score, Op::MoveTo(crate::edit::Pos { bar: 2, tick: 0 }));
        assert!(input.status(&score).contains("bar 3, beat 1"));
        input.apply(&mut score, Op::Duration(Value::Whole));
        let out = input.apply(&mut score, Op::Letter('A'));
        assert_eq!(out.changed, [2]);
        let new = write_bars(text, &chart, &score, &out.changed).unwrap();
        assert!(new.contains("| C | G |\n\nCH 1\n| A_1 |"), "{new}");
        // An entry on the replayed verse is written at the first.
        let chart = parse_chart(&new).unwrap();
        let mut score = Score::from_text(&new, &chart);
        input.apply(&mut score, Op::MoveTo(crate::edit::Pos { bar: 4, tick: 0 }));
        let out = input.apply(&mut score, Op::Letter('E'));
        assert_eq!(out.changed, [1]);
        let newer = write_bars(&new, &chart, &score, &out.changed).unwrap();
        assert!(newer.contains("| C | E_1 |"), "{newer}");
    }

    #[test]
    fn a_chord_is_retyped_or_its_quality_stepped_without_the_bar() {
        let text = "Edit - Test\n4/4\n\nVS 2\n| C / G / | !Am / / / |\n";
        let chart = parse_chart(text).unwrap();
        let mut score = Score::from_text(text, &chart);
        let mut input = InputState::default();
        // The G (from beat 2: `C /` is one beat), selected: retyped, its
        // length kept.
        input.selected = Some(crate::edit::Pos { bar: 0, tick: 192 });
        assert_eq!(input.selected_symbol(&score).as_deref(), Some("G"));
        let out = input.set_chord(&mut score, "Gsus4");
        let new = write_bars(text, &chart, &score, &out.changed).unwrap();
        assert!(new.contains("C / Gsus4 /// | !Am / / /"), "{new}");
        // The accented Am: its quality stepped, its accent kept.
        let chart = parse_chart(&new).unwrap();
        let mut score = Score::from_text(&new, &chart);
        input.selected = Some(crate::edit::Pos { bar: 1, tick: 0 });
        let out = input.apply(&mut score, Op::Quality(true));
        let newer = write_bars(&new, &chart, &score, &out.changed).unwrap();
        assert!(newer.contains("!A7"), "{newer}");
    }

    #[test]
    fn a_palette_cell_sets_the_selected_chord_s_quality_or_marks_it() {
        let text = "Edit - Test\n4/4\n\nVS 2\n| C / G / | Am / / / |\n";
        let chart = parse_chart(text).unwrap();
        let mut score = Score::from_text(text, &chart);
        let mut input = InputState::default();
        // The G: made a maj7, its length kept.
        input.selected = Some(crate::edit::Pos { bar: 0, tick: 192 });
        let out = input.set_quality(&mut score, "maj7");
        let new = write_bars(text, &chart, &score, &out.changed).unwrap();
        assert!(new.contains("C / Gmaj7 /// | Am / / /"), "{new}");
        // An accent and a staccato on the G, read back as written; then
        // the accent off again.
        let mut text = new;
        for (mark, written) in [(">", ">Gmaj7"), (".", ">.Gmaj7"), (">", "C / .Gmaj7")] {
            let chart = parse_chart(&text).unwrap();
            let mut score = Score::from_text(&text, &chart);
            let out = input.toggle_mark(&mut score, mark);
            assert!(out.refused.is_none(), "{mark}: {:?}", out.refused);
            text = write_bars(&text, &chart, &score, &out.changed)
                .unwrap_or_else(|e| panic!("{mark} on {text}: {e}"));
            assert!(text.contains(written), "{mark}: {text}");
        }
        // Pushed, an eighth and then a sixteenth early, and back: the G
        // stays on the beat it is written on and sounds before it.
        for (mark, written) in [
            ("'", "C / .'Gmaj7"),
            ("''", "C / .''Gmaj7"),
            ("''", "C / .Gmaj7"),
        ] {
            let chart = parse_chart(&text).unwrap();
            let mut score = Score::from_text(&text, &chart);
            let out = input.toggle_mark(&mut score, mark);
            assert!(out.refused.is_none(), "{mark}: {:?}", out.refused);
            text = write_bars(&text, &chart, &score, &out.changed)
                .unwrap_or_else(|e| panic!("{mark} on {text}: {e}"));
            assert!(text.contains(written), "{mark}: {text}");
        }
        // On the Am, the bar's first chord, a push would sound in the bar
        // before: refused, and why.
        input.selected = Some(crate::edit::Pos { bar: 1, tick: 0 });
        let chart = parse_chart(&text).unwrap();
        let mut score = Score::from_text(&text, &chart);
        let out = input.toggle_mark(&mut score, "'");
        assert!(out.changed.is_empty(), "{:?}", out.changed);
        assert!(
            out.refused
                .as_deref()
                .is_some_and(|r| r.contains("bar before"))
        );
        // Accented.
        let out = input.toggle_mark(&mut score, ">");
        let text = write_bars(&text, &chart, &score, &out.changed).unwrap();
        assert!(text.contains("| >Am"), "{text}");
        // Nothing selected: said so, nothing written.
        let mut none = InputState::default();
        let chart = parse_chart(&text).unwrap();
        let mut score = Score::from_text(&text, &chart);
        let out = none.set_quality(&mut score, "m7");
        assert!(out.changed.is_empty() && out.refused.is_some());
    }

    #[test]
    fn a_bar_the_header_counts_but_the_text_does_not_write_takes_an_entry() {
        // `CH 4` over three bars written: the fourth, which the parser adds
        // empty, is written after the third when an entry is made on it.
        let text = "T - A\n4/4\n\nCH 4\nD E\n\"Drum fill\" >F#m_8 !STOP r8 r2.\n\nVS 1\nA\n";
        let chart = parse_chart(text).unwrap();
        let mut score = Score::from_text(text, &chart);
        assert_eq!(score.read_only[3], None);
        let mut input = InputState {
            mode: Mode::NoteName,
            ..InputState::default()
        };
        input.apply(&mut score, Op::MoveTo(crate::edit::Pos { bar: 3, tick: 0 }));
        input.apply(&mut score, Op::Duration(Value::Whole));
        let out = input.apply(&mut score, Op::Letter('G'));
        let new = write_bars(text, &chart, &score, &out.changed).unwrap();
        assert!(new.contains("r2. | G_1\n\nVS 1\nA"), "{new}");
    }

    #[test]
    fn a_one_bar_block_with_a_cue_before_it_is_found_by_its_replay() {
        // A section that replays one whose bar is a `<< >>` line with a cue
        // before it plays that bar.
        let text = "T - A\n4/4\n\nIN 1\n\"Stop groove\" << F_1 ; m{ C:41 } >>\n\nCH 1\nG\n\nIN\n";
        let chart = parse_chart(text).unwrap();
        let score = Score::from_text(text, &chart);
        assert_eq!(score.replays, [None, None, Some(0)]);
    }

    #[test]
    fn a_one_bar_meter_stays_on_its_bar_when_edited() {
        let text = "T - A\n4/4\n\nVS 2\nC_1 !T3/4 Fadd2 // Dm /\n";
        let chart = parse_chart(text).unwrap();
        let mut score = Score::from_text(text, &chart);
        let mut input = InputState {
            mode: Mode::NoteName,
            ..InputState::default()
        };
        input.apply(&mut score, Op::MoveTo(crate::edit::Pos { bar: 1, tick: 0 }));
        input.apply(&mut score, Op::Duration(Value::Quarter));
        let out = input.apply(&mut score, Op::Letter('G'));
        let new = write_bars(text, &chart, &score, &out.changed).unwrap();
        assert!(new.contains("!T3/4 G"), "{new}");
    }

    #[test]
    fn a_bar_tied_into_the_next_keeps_its_tie_when_edited() {
        let text = "T - A\nbF 4/4\n\nVS 2\nBbmaj7_8 Ebmaj7_8 Am7_16 Dm7_8. Abmaj9_8 Bbmaj7_8 Ebmaj7_8 >Fadd2_8 ~\nFadd2_1\n";
        let chart = parse_chart(text).unwrap();
        let mut score = Score::from_text(text, &chart);
        let mut input = InputState {
            mode: Mode::NoteName,
            ..InputState::default()
        };
        input.apply(&mut score, Op::MoveTo(crate::edit::Pos { bar: 0, tick: 0 }));
        let out = input.apply(&mut score, Op::Letter('C'));
        let new = write_bars(text, &chart, &score, &out.changed).unwrap();
        assert!(
            new.contains("C_4 Am7_16 Dm7_8. Abmaj9_8 Bbmaj7_8 Ebmaj7_8 >Fadd2_8 ~\n"),
            "{new}"
        );
    }

    #[test]
    fn a_line_with_a_cue_before_it_is_edited_and_the_cue_stays() {
        // `^"Intro groove" Bm % Gmaj7 %`: written barlined after an edit,
        // the cue stays on the first bar, takes no beat of it, and a `%`
        // written out is the bar's music without its cue.
        let text = "T - A\n#Bm 4/4\n\nINT 4\n^\"Intro groove\" Bm % Gmaj7 %\n";
        let chart = parse_chart(text).unwrap();
        for (at, want) in [
            (0usize, "^\"Intro groove\" C_1 | Bm | Gmaj7 | %"),
            (2, "^\"Intro groove\" Bm | % | C_1 | Gmaj7"),
        ] {
            let mut score = Score::from_text(text, &chart);
            let mut input = InputState {
                mode: Mode::NoteName,
                ..InputState::default()
            };
            input.apply(
                &mut score,
                Op::MoveTo(crate::edit::Pos { bar: at, tick: 0 }),
            );
            input.apply(&mut score, Op::Duration(Value::Whole));
            let out = input.apply(&mut score, Op::Letter('C'));
            let new = write_bars(text, &chart, &score, &out.changed)
                .unwrap_or_else(|e| panic!("bar {}: {e}", at + 1));
            assert!(new.contains(want), "{new}");
        }
    }

    #[test]
    fn a_bar_beside_a_percent_three_is_edited_and_the_repeat_kept() {
        // `Dm7 %3 Fmaj7/G %3`: Dm7 four times, Fmaj7/G four times. An edit
        // writes the line barlined, where `%3` would repeat three bars: the
        // other `%3` is written `% | % | %`, the bar before three times.
        let text = "T - A\n#C 4/4\n\nCH 8\nDm7 %3 Fmaj7/G %3\n";
        let chart = parse_chart(text).unwrap();
        for at in [0usize, 4] {
            let mut score = Score::from_text(text, &chart);
            let mut input = InputState {
                mode: Mode::NoteName,
                ..InputState::default()
            };
            input.apply(
                &mut score,
                Op::MoveTo(crate::edit::Pos { bar: at, tick: 0 }),
            );
            input.apply(&mut score, Op::Duration(Value::Whole));
            let out = input.apply(&mut score, Op::Letter('C'));
            let new = write_bars(text, &chart, &score, &out.changed)
                .unwrap_or_else(|e| panic!("bar {}: {e}", at + 1));
            let back = parse_chart(&new).unwrap();
            let chords: Vec<String> = measures(&back)
                .iter()
                .map(|m| ChordBar::from_measure(m).to_text().unwrap())
                .collect();
            let mut want = vec!["Dm7_1"; 4];
            want.extend(["Fmaj7/G_1"; 4]);
            want[at] = "C_1";
            assert_eq!(chords, want, "{new}");
        }
    }

    #[test]
    fn a_replay_of_a_held_chord_run_plays_each_of_its_bars() {
        // The second VS replays the first, whose line holds G#m and C#sus2
        // over barlines: its n-th bar plays the n-th, and follows an edit.
        let text =
            "Held\n4/4\n\nVS 4\nE /// G#m / G#m B /// C#sus2 / C#sus2\n\nCH 2\n| A | B |\n\nVS\n";
        let chart = parse_chart(text).unwrap();
        let mut score = Score::from_text(text, &chart);
        assert_eq!(&score.replays[6..], [Some(0), Some(1), Some(2), Some(3)]);
        let mut input = InputState {
            mode: Mode::NoteName,
            ..InputState::default()
        };
        input.apply(&mut score, Op::MoveTo(crate::edit::Pos { bar: 1, tick: 0 }));
        input.apply(&mut score, Op::Duration(Value::Whole));
        let out = input.apply(&mut score, Op::Letter('A'));
        let new = write_bars(text, &chart, &score, &out.changed).unwrap();
        assert!(
            new.contains("E /// G#m / | A_1 | B /// C#sus2 / | C#sus2"),
            "{new}"
        );
    }

    #[test]
    fn a_triplet_is_made_and_filled_and_written() {
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
            Op::Triplet,
            Op::Letter('C'),
            Op::Letter('D'),
            Op::Letter('E'),
            Op::Letter('F'),
        ] {
            changed.extend(input.apply(&mut score, op).changed);
        }
        // After the group: quarters again.
        assert_eq!(
            input.duration,
            crate::edit::music::Duration::new(Value::Quarter)
        );
        let text = write_bars(CHART, &chart, &score, &changed).unwrap();
        assert!(text.contains("m{ C:58t D:58t E:58t F:54 r2 }"), "{text}");
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
            text.contains("| C / / / | D // G // | << A_1 ; m{ a:44 r2. } >> |"),
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
        // Past the end: added, not refused.
        let (text, n) = splice_syntax(CHART, &chart, 3, "C | D").unwrap();
        assert_eq!(n, 2);
        assert!(text.contains("VS 5"), "{text}");
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
    fn a_bar_that_cannot_be_written_alone_keeps_its_line_as_written() {
        // `C_2 G_2. D_4` in two bars: the G does not fit bar 1 and moves to
        // bar 2, leaving bar 1 half full, which no bar of its own says.
        let text = "Edit - Test\n4/4\n\nVS 2\nC_2 G_2. D_4\n";
        let chart = parse_chart(text).unwrap();
        let mut score = Score::from_chart(&chart);
        let mut input = InputState {
            mode: Mode::NoteName,
            ..InputState::default()
        };
        input.apply(&mut score, Op::MoveTo(crate::edit::Pos { bar: 1, tick: 0 }));
        let out = input.apply(&mut score, Op::Letter('A'));
        let err = write_bars(text, &chart, &score, &out.changed).unwrap_err();
        assert!(err.contains("over its barline"), "{err}");
    }

    #[test]
    fn a_bar_a_chord_is_held_into_is_edited_and_its_line_written_out() {
        // G#m on beat 4 held into the next bar: the four bars share one
        // line's text. An edit to the second writes the line out bar by
        // bar, every other bar reading exactly as it did.
        let text = "Edit - Test\n4/4\n\nIN 4\nE /// G#m / G#m B /// C#sus2 / C#sus2\n";
        let chart = parse_chart(text).unwrap();
        assert!(
            measures(&chart).iter().all(|m| m.read_only.is_none()),
            "every bar editable"
        );
        let mut score = Score::from_chart(&chart);
        let mut input = InputState {
            mode: Mode::NoteName,
            ..InputState::default()
        };
        input.apply(&mut score, Op::MoveTo(crate::edit::Pos { bar: 1, tick: 0 }));
        let mut changed = Vec::new();
        for op in [Op::Duration(Value::Whole), Op::Letter('A')] {
            changed.extend(input.apply(&mut score, op).changed);
        }
        let new = write_bars(text, &chart, &score, &changed).unwrap();
        // The other run (C#sus2 held into bar 4) keeps its text.
        assert!(
            new.contains("E /// G#m / | A_1 | B /// C#sus2 / | C#sus2"),
            "{new}"
        );
        // Unchanged, typed back over itself: the text stays as written.
        let (same, _) = splice_syntax(text, &chart, 1, "G#m").unwrap();
        assert_eq!(same, text);
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
