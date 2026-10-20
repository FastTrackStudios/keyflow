//! A recognised score, written as a keyflow Master Rhythm chart: chord
//! symbols on the chart staff with the notes in `m{ }` beside them, cues as
//! staff text, and the header from the title, key, tempo and meter.
//!
//! The chart staff is one staff, so a grand staff's top staff is the line
//! the chart shows. The lower staff is not written yet: the chart engine
//! draws one staff, and a second lane it would not draw is worse than none.

use std::fmt::Write as _;

use engraver_score::model::{
    Chord, ClefSign, DirectionKind, Event, Measure, NoteTypeValue, Score, Step, WrittenDuration,
    WrittenPitch,
};

/// How a bar's music goes on the one chart staff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Style {
    /// Per bar: comping (stacked chords voicing the symbols) as rhythmic
    /// slashes under the chord names, a single-note line as its notes. The
    /// original is always there for the voicings.
    #[default]
    Auto,
    /// Every bar as chord names over rhythmic slashes.
    Rhythm,
    /// Every bar's notes written out.
    Notation,
}

/// The chart text for `score`'s first part, bars chosen by [`Style::Auto`].
#[must_use]
pub fn to_keyflow(score: &Score, artist: Option<&str>) -> String {
    to_keyflow_with(score, artist, Style::Auto)
}

/// The chart text for `score`'s first part.
#[must_use]
pub fn to_keyflow_with(score: &Score, artist: Option<&str>, style: Style) -> String {
    let Some(part) = score.parts.first() else {
        return String::new();
    };
    let mut out = String::new();
    let title = score.work_title.as_deref().unwrap_or("Untitled");
    // A title that would read as music ("1901", "Em") is quoted.
    let musical = title.starts_with(|c: char| c.is_ascii_digit())
        || (title.len() <= 4 && title.starts_with(|c: char| ('A'..='G').contains(&c)));
    let title = if musical { format!("\"{title}\"") } else { title.to_string() };
    match artist {
        Some(a) => {
            let _ = writeln!(out, "{title} - {a}");
        }
        None => {
            let _ = writeln!(out, "{title}");
        }
    }
    let first = part.measures.first();
    let attrs = first.and_then(|m| m.attributes.first());
    let fifths = attrs.and_then(|a| a.key_fifths).unwrap_or(0);
    let (num, den) = attrs.and_then(|a| a.time).unwrap_or((4, 4));
    let tempo = part
        .measures
        .iter()
        .flat_map(|m| &m.directions)
        .find_map(|d| match &d.kind {
            DirectionKind::Tempo(t) => Some(t.clone()),
            _ => None,
        });
    let clef = attrs
        .and_then(|a| a.clefs.iter().find(|(s, _)| *s == 1))
        .map_or("treble", |(_, c)| match c.sign {
            ClefSign::F => "bass",
            ClefSign::C => "alto",
            _ => "treble",
        });
    let _ = write!(out, "{} ", key_name(fifths));
    if let Some(bpm) = tempo.as_ref().and_then(chart_bpm) {
        let _ = write!(out, "{bpm}bpm ");
    }
    let _ = writeln!(out, "{num}/{den} {clef}");
    out.push('\n');

    let _ = writeln!(out, "[Hook] {}", part.measures.len());
    let mut bars: Vec<String> = Vec::new();
    let mut last_chord: Option<String> = None;
    let mut last_attack: Option<String> = None;
    for m in &part.measures {
        bars.push(bar_text(m, 1, style, &mut last_chord, &mut last_attack));
    }
    // Four bars to a line, as the chart reads.
    for line in bars.chunks(4) {
        let _ = writeln!(out, "| {} |", line.join(" | "));
    }
    out
}

/// The metronome mark as printed: the chart header shows "♩ = N" and the
/// parser keeps one number, so the printed number is the faithful one —
/// even where the arranger's note value is ambiguous (♩ = 246 in 12/8).
fn chart_bpm(t: &engraver_score::model::TempoMark) -> Option<u32> {
    let per: f64 = t.per_minute.split('-').next()?.trim().parse().ok()?;
    Some(per.round() as u32)
}

fn key_name(fifths: i8) -> &'static str {
    const SHARP: [&str; 8] = ["#C", "#G", "#D", "#A", "#E", "#B", "#F#", "#C#"];
    const FLAT: [&str; 8] = ["#C", "bF", "bBb", "bEb", "bAb", "bDb", "bGb", "bCb"];
    if fifths >= 0 {
        SHARP[fifths.min(7) as usize]
    } else {
        FLAT[(-fifths).min(7) as usize]
    }
}

/// One bar: staff text, the chord symbols, then the staff's first voice.
fn bar_text(
    m: &Measure,
    staff: u8,
    style: Style,
    last_chord: &mut Option<String>,
    last_attack: &mut Option<String>,
) -> String {
    let mut s = String::new();
    // A bar's cues read as one line ("(D) Keys Start"), not a stack: the
    // lowest line first, as the eye meets them coming down the page.
    let words: Vec<&str> = m
        .directions
        .iter()
        .filter_map(|d| match &d.kind {
            DirectionKind::Words(w) => Some(w.as_str()),
            _ => None,
        })
        .rev()
        .collect();
    if !words.is_empty() {
        let _ = write!(s, "^\"{}\" ", words.join(" ").replace('"', "'"));
    }
    if let Some(rhythm) = comp_rhythm(m, staff, style, last_chord, last_attack) {
        s.push_str(&rhythm);
        return s;
    }
    *last_attack = None;
    let mut chords: Vec<(u32, String)> = m
        .directions
        .iter()
        .filter_map(|d| match &d.kind {
            DirectionKind::Harmony(h) => Some((d.tick, h.clone())),
            _ => None,
        })
        .collect();
    let len = m.len_ticks.max(1);
    // Inside the bar, one chord to a spot: an overfull bar (two voices read
    // as one) can place a symbol past the barline.
    let eighth = (m.divisions / 2).max(1);
    for c in &mut chords {
        c.0 = c.0.min(len.saturating_sub(eighth));
    }
    chords.sort_by_key(|c| c.0);
    chords.dedup_by_key(|c| c.0);
    if chords.is_empty() {
        s.push_str("s1");
    } else if chords.len() == 1 && chords[0].0 == 0 {
        s.push_str(&chord_name(&chords[0].1));
    } else {
        // Several chords: each lasts until the next. A late first chord
        // starts after a space.
        let mut at = 0u32;
        let div = m.divisions.max(1);
        if chords[0].0 > 0 {
            for piece in pieces(chords[0].0, div) {
                let _ = write!(s, "s{piece} ");
            }
            at = chords[0].0;
        }
        for (i, (tick, name)) in chords.iter().enumerate() {
            let end = chords.get(i + 1).map_or(len, |c| c.0);
            // The chord takes the first piece; invisible spaces carry the
            // rest, so a five-eighth chord is `C_2 s8` and the bar adds up.
            let parts = pieces(end.saturating_sub(*tick).max(1), div);
            let _ = write!(s, "{}_{} ", chord_name(name), parts[0]);
            for piece in &parts[1..] {
                let _ = write!(s, "s{piece} ");
            }
            at = end;
        }
        let _ = at;
        s = s.trim_end().to_string();
    }
    // A voice that does not fit its bar was misread; the chart keeps the
    // bar's chords and leaves those notes out rather than spill them into
    // the next bar.
    let notes = if voice_ticks(m, staff) > len { String::new() } else { melody(m, staff) };
    if !notes.is_empty() {
        let _ = write!(s, " m{{ {notes} }}");
    }
    s
}

/// A chord symbol as keyflow spells it. Engravers bracket worded suffixes
/// (`C(sus4)`, `G(add9)`), but brackets in a chart are alterations (`C7(b9)`)
/// or rhythm groups, so a bracketed word loses its brackets.
fn chord_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut chars = name.chars().peekable();
    while let Some(c) = chars.next() {
        // A word (`sus4`, `add9`) starts with two letters; `(b9)` is an
        // alteration and keeps its brackets.
        let mut ahead = chars.clone();
        let word = ahead.next().is_some_and(|n| n.is_ascii_alphabetic())
            && ahead.next().is_some_and(|n| n.is_ascii_alphabetic());
        if c == '(' && word {
            let inner: String = chars.by_ref().take_while(|&n| n != ')').collect();
            out.push_str(&inner);
        } else {
            out.push(c);
        }
    }
    out
}

/// `ticks` as note values longest first (`2.`, `4`, `16`, …), each one
/// written value; together they add up exactly (to a sixteenth).
fn pieces(ticks: u32, div: u32) -> Vec<String> {
    // (length in sixteenths, value)
    const VALUES: [(u32, &str); 9] = [
        (24, "1."),
        (16, "1"),
        (12, "2."),
        (8, "2"),
        (6, "4."),
        (4, "4"),
        (3, "8."),
        (2, "8"),
        (1, "16"),
    ];
    let mut left = (f64::from(ticks) * 4.0 / f64::from(div.max(1))).round() as u32;
    let mut out = Vec::new();
    while left > 0 {
        let (n, v) = VALUES.iter().find(|(n, _)| *n <= left).copied().unwrap_or((1, "16"));
        out.push(v.to_string());
        left -= n;
    }
    if out.is_empty() {
        out.push("16".into());
    }
    out
}

/// A length in quarters as a note value (1 = whole … 32) and a dot.
fn value_of(q: f64) -> (u32, bool) {
    for (v, len) in [(1, 4.0), (2, 2.0), (4, 1.0), (8, 0.5), (16, 0.25), (32, 0.125)] {
        if (q - len).abs() < 1e-6 {
            return (v, false);
        }
        if (q - len * 1.5).abs() < 1e-6 {
            return (v, true);
        }
    }
    (4, false)
}

fn note_value(w: &WrittenDuration) -> String {
    let base = match w.note_type {
        Some(NoteTypeValue::Whole) => "1",
        Some(NoteTypeValue::Half) => "2",
        Some(NoteTypeValue::Eighth) => "8",
        Some(NoteTypeValue::Sixteenth) => "16",
        Some(NoteTypeValue::ThirtySecond) => "32",
        _ => "4",
    };
    let mut s = base.to_string();
    for _ in 0..w.dots {
        s.push('.');
    }
    if w.tuplet == Some((3, 2)) {
        s.push('t');
    }
    s
}

fn pitch(p: &WrittenPitch) -> String {
    let step = match p.step {
        Step::A => 'A',
        Step::B => 'B',
        Step::C => 'C',
        Step::D => 'D',
        Step::E => 'E',
        Step::F => 'F',
        Step::G => 'G',
    };
    let acc = match p.alter {
        1 => "#",
        -1 => "b",
        _ => "",
    };
    format!("{step}{acc}:{}", p.octave)
}

/// The bar as chord names over rhythmic slashes — one chord token per
/// attack at its written value, rests as rests — when it is comping: at
/// least two thirds of its attacks stacked chords (or always, in
/// [`Style::Rhythm`]). `None` keeps the bar's notes. `last_chord` carries
/// the harmony across barlines, for attacks before the bar's first symbol.
///
/// An attack repeating the previous one — same chord, same value — is `.`,
/// so the symbol is printed where the harmony changes and the slashes carry
/// the rhythm. `last_attack` carries the last token across the barline;
/// a rest ends the run.
fn comp_rhythm(
    m: &Measure,
    staff: u8,
    style: Style,
    last_chord: &mut Option<String>,
    last_attack: &mut Option<String>,
) -> Option<String> {
    let mut symbols: Vec<(u32, String)> = m
        .directions
        .iter()
        .filter_map(|d| match &d.kind {
            DirectionKind::Harmony(h) => Some((d.tick, chord_name(h))),
            _ => None,
        })
        .collect();
    symbols.sort_by_key(|c| c.0);
    let evs = first_voice(m, staff);
    let attacks: Vec<&Chord> = evs
        .iter()
        .filter_map(|e| match e {
            Event::Chord(c) => Some(c),
            Event::Rest(_) => None,
        })
        .collect();
    let stacked = attacks.iter().filter(|c| c.notes.len() > 1).count();
    let comping = !attacks.is_empty() && stacked * 3 >= attacks.len() * 2;
    let fits = voice_ticks(m, staff) == m.len_ticks;
    let wanted = match style {
        Style::Notation => false,
        Style::Rhythm => true,
        Style::Auto => comping,
    };
    // Whatever the bar becomes, its last symbol is the harmony going on.
    let end_chord = symbols.last().map(|c| c.1.clone()).or_else(|| last_chord.clone());
    if !wanted || !fits || attacks.is_empty() {
        *last_chord = end_chord;
        return None;
    }
    let mut out = Vec::new();
    for e in evs {
        match e {
            Event::Rest(r) => {
                let v = if r.measure_rest {
                    pieces(m.len_ticks, m.divisions).join(" r")
                } else {
                    note_value(&r.written)
                };
                out.push(format!("r{v}"));
                *last_attack = None;
            }
            Event::Chord(c) => {
                let name = symbols
                    .iter()
                    .rev()
                    .find(|(t, _)| *t <= c.tick)
                    .map(|(_, n)| n.clone())
                    .or_else(|| last_chord.clone())
                    .or_else(|| symbols.first().map(|(_, n)| n.clone()));
                let Some(name) = name else {
                    *last_chord = end_chord;
                    return None;
                };
                let token = format!("{name}_{}", note_value(&c.written));
                if last_attack.as_deref() == Some(token.as_str()) {
                    out.push(".".to_string());
                } else {
                    out.push(token.clone());
                }
                *last_attack = Some(token);
            }
        }
    }
    *last_chord = end_chord;
    Some(out.join(" "))
}

/// `staff`'s first voice, in time order.
fn first_voice(m: &Measure, staff: u8) -> Vec<&Event> {
    let voice = m
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Chord(c) if c.staff == staff => Some(c.voice),
            Event::Rest(r) if r.staff == staff => Some(r.voice),
            _ => None,
        })
        .min();
    let mut evs: Vec<&Event> = m.events.iter().filter(|e| Some(e.voice()) == voice).collect();
    evs.sort_by_key(|e| e.tick());
    evs
}

/// Length of `staff`'s first voice in ticks.
fn voice_ticks(m: &Measure, staff: u8) -> u32 {
    let voice = m
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Chord(c) if c.staff == staff => Some(c.voice),
            Event::Rest(r) if r.staff == staff => Some(r.voice),
            _ => None,
        })
        .min();
    m.events
        .iter()
        .filter(|e| Some(e.voice()) == voice)
        .map(|e| match e {
            Event::Chord(c) => c.duration,
            Event::Rest(r) => r.duration,
        })
        .sum()
}

/// The first voice of `staff`, every note with its octave and value.
fn melody(m: &Measure, staff: u8) -> String {
    let voice = m
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Chord(c) if c.staff == staff => Some(c.voice),
            Event::Rest(r) if r.staff == staff => Some(r.voice),
            _ => None,
        })
        .min();
    let Some(voice) = voice else {
        return String::new();
    };
    let mut evs: Vec<&Event> = m.events.iter().filter(|e| e.voice() == voice).collect();
    evs.sort_by_key(|e| e.tick());
    // A bar of nothing but rests is a whole-bar rest, not slashes (which
    // on a rhythm chart mean "keep playing").
    if evs.iter().all(|e| matches!(e, Event::Rest(_))) {
        let q = f64::from(m.len_ticks) / f64::from(m.divisions.max(1));
        let (v, dot) = value_of(q);
        return format!("r{v}{}", if dot { "." } else { "" });
    }
    let mut parts = Vec::new();
    for e in evs {
        match e {
            Event::Rest(r) => {
                if r.measure_rest {
                    parts.push(format!("r{}", value_of(f64::from(m.len_ticks) / f64::from(m.divisions.max(1))).0));
                } else {
                    parts.push(format!("r{}", note_value(&r.written)));
                }
            }
            Event::Chord(c) => {
                let v = note_value(&c.written);
                if c.notes.len() == 1 {
                    parts.push(format!("{}{v}", pitch(&c.notes[0].pitch)));
                } else {
                    let mut ps: Vec<&WrittenPitch> = c.notes.iter().map(|n| &n.pitch).collect();
                    ps.sort_by_key(|p| p.midi());
                    let inner: Vec<String> = ps.iter().map(|p| pitch(p)).collect();
                    parts.push(format!("<{}>{v}", inner.join(" ")));
                }
            }
        }
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    #[test]
    fn worded_suffixes_lose_their_brackets() {
        assert_eq!(super::chord_name("C(sus4)"), "Csus4");
        assert_eq!(super::chord_name("G(add9)"), "Gadd9");
        assert_eq!(super::chord_name("C7(b9)"), "C7(b9)");
        assert_eq!(super::chord_name("F#m7"), "F#m7");
    }

    #[test]
    fn odd_lengths_split_into_written_values() {
        // Five eighths: a half and an eighth.
        assert_eq!(super::pieces(5 * 240, 480), vec!["2", "8"]);
        assert_eq!(super::pieces(3 * 480, 480), vec!["2."]);
    }
}
