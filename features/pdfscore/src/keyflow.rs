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

    // A `|:` that nothing closes is not a repeat (the closing sign was not
    // read, or the part ends first); a lone `:|` is — it repeats from the
    // top, as engravers write it.
    let mut measures = part.measures.clone();
    for i in 0..measures.len() {
        if measures[i].repeat_start {
            let closed = measures[i..]
                .iter()
                .enumerate()
                .find(|(k, m)| m.repeat_end || (*k > 0 && m.repeat_start))
                .is_some_and(|(_, m)| m.repeat_end);
            if !closed {
                measures[i].repeat_start = false;
            }
        }
    }
    let part = &engraver_score::model::Part { measures, ..part.clone() };
    let sections = sections_of(&part.measures);
    // One staff: each bar shows the top staff when it plays, else the bass
    // staff ("bass line below"). A section whose written lines all come
    // from the bass is set in the bass clef; one that mixes top and bass
    // lines keeps the top's clef and gives the bass bars as rhythm.
    let mut plan: Vec<(u8, Style)> = part
        .measures
        .iter()
        .map(|m| (if plays(m, 1) || !plays(m, 2) { 1 } else { 2 }, style))
        .collect();
    let mut bass_sections: Vec<bool> = Vec::new();
    for range in &sections {
        let line_from = |staff: u8| {
            range.clone().any(|i| plan[i].0 == staff && is_line(&part.measures[i], staff, style))
        };
        let (top, bass) = (line_from(1), line_from(2));
        bass_sections.push(bass && !top);
        if bass && top {
            for i in range.clone() {
                if plan[i].0 == 2 {
                    plan[i].1 = Style::Rhythm;
                }
            }
        }
    }
    let mut bars: Vec<String> = Vec::new();
    let mut last_chord: Option<String> = None;
    let mut last_attack: Option<String> = None;
    for (i, m) in part.measures.iter().enumerate() {
        // A section (and a repeat) opens with its chord named: `.` there
        // would point back across the boundary.
        if sections.iter().any(|r| r.start == i) || m.repeat_start || m.ending.is_some() {
            last_attack = None;
        }
        let (staff, bar_style) = plan[i];
        bars.push(bar_text(m, staff, bar_style, &mut last_chord, &mut last_attack));
    }
    let named = sections.len() > 1;
    for (k, range) in sections.iter().enumerate() {
        let name = if named {
            char::from(b'A' + (k % 26) as u8).to_string()
        } else {
            "Hook".to_string()
        };
        let _ = writeln!(out, "[{name}] {}", range.len());
        if bass_sections[k] && clef != "bass" {
            let _ = writeln!(out, "\\clef bass");
        }
        // Four bars to a line, as the chart reads.
        let idx: Vec<usize> = range.clone().collect();
        for line in idx.chunks(4) {
            let _ = writeln!(out, "{}", bar_line(&part.measures, &bars, line));
        }
        out.push('\n');
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

/// Where the chart's sections begin: at a repeat sign, after a double or
/// section barline, and after a repeat closes (unless an ending follows,
/// which belongs to the repeat).
fn sections_of(ms: &[Measure]) -> Vec<std::ops::Range<usize>> {
    let mut starts = vec![0];
    for i in 1..ms.len() {
        let prev = &ms[i - 1];
        let starts_here = ms[i].repeat_start
            || prev.barline.is_section_end() && ms[i].ending.is_none()
            || prev.repeat_end && ms[i].ending.is_none();
        if starts_here {
            starts.push(i);
        }
    }
    starts.dedup();
    starts
        .iter()
        .zip(starts.iter().skip(1).chain(std::iter::once(&ms.len())))
        .map(|(a, b)| *a..*b)
        .filter(|r| !r.is_empty())
        .collect()
}

/// One line of bars with their repeat signs and endings:
/// `|: A | Bm | [1:2] E | A :| [2] D |`.
fn bar_line(ms: &[Measure], bars: &[String], line: &[usize]) -> String {
    let mut s = String::new();
    for (k, &i) in line.iter().enumerate() {
        let m = &ms[i];
        if k == 0 {
            s.push_str(if m.repeat_start { "|: " } else { "| " });
        }
        if let Some(label) = &m.ending {
            let _ = write!(s, "[{}] ", ending_token(label, ending_length(ms, i)));
        }
        s.push_str(&bars[i]);
        let next = line.get(k + 1).map(|&j| &ms[j]);
        s.push_str(match (m.repeat_end, next.is_some_and(|n| n.repeat_start), next.is_some()) {
            (true, true, _) => " :| |: ",
            (true, false, true) => " :| ",
            (false, true, _) => " |: ",
            (false, false, true) => " | ",
            (true, _, false) => " :|",
            (false, _, false) => " |",
        });
    }
    s
}

/// `1.` / `1., 2.` as a chart ending: `1` / `1, 2`, with `:n` for an ending
/// longer than a bar.
fn ending_token(label: &str, length: usize) -> String {
    let nums: Vec<&str> = label
        .split(',')
        .map(|p| p.trim().trim_end_matches('.'))
        .filter(|p| !p.is_empty())
        .collect();
    if length > 1 {
        format!("{}:{length}", nums.join(", "))
    } else {
        nums.join(", ")
    }
}

/// An ending runs to the repeat sign that closes it (a first ending of two
/// bars), or is one bar.
fn ending_length(ms: &[Measure], i: usize) -> usize {
    for (j, m) in ms.iter().enumerate().skip(i) {
        if j > i && m.ending.is_some() {
            break;
        }
        if m.repeat_end {
            return j - i + 1;
        }
    }
    1
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
    // A voicing or a rhythm slash: comping, not a line.
    let stacked = attacks
        .iter()
        .filter(|c| c.notes.len() > 1 || c.articulations.contains("rhythm-slash"))
        .count();
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
    // The last attack this bar, for a tie to extend: (index in `out`,
    // chord, ticks so far).
    let mut held: Option<(usize, String, u32)> = None;
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
                held = None;
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
                // Tied on from the bar's previous attack of this chord: one
                // longer value when a single note value says it.
                let continues = c.notes.iter().any(|n| n.tie_stop);
                if continues
                    && let Some((idx, prev_name, prev_ticks)) = held.clone()
                    && prev_name == name
                {
                    let merged = pieces(prev_ticks + c.duration, m.divisions);
                    if merged.len() == 1 {
                        let on = if c.notes.iter().any(|n| n.tie_start) { "~" } else { "" };
                        let into = if out[idx].starts_with('~') { "~" } else { "" };
                        out[idx] = format!("{into}{name}_{}{on}", merged[0]);
                        held = Some((idx, name.clone(), prev_ticks + c.duration));
                        *last_attack = Some(out[idx].clone());
                        continue;
                    }
                }
                held = Some((out.len(), name.clone(), c.duration));
                // A tie that cannot merge is written as one (`Em_16~ ~Em_2.`,
                // and over the barline `Em_2.~ | ~Em_1`): a held chord, not a
                // second attack — so never `.`.
                let into = if continues { "~" } else { "" };
                let on = if c.notes.iter().any(|n| n.tie_start) { "~" } else { "" };
                if continues
                    && let Some(prev) = out.last_mut()
                    && !prev.ends_with('~')
                    && !prev.starts_with('r')
                {
                    prev.push('~');
                }
                let token = format!("{into}{name}_{}{on}", note_value(&c.written));
                if into.is_empty() && on.is_empty() && last_attack.as_deref() == Some(token.as_str()) {
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

/// Whether `staff` sounds in this bar (any note, not just rests).
fn plays(m: &Measure, staff: u8) -> bool {
    m.events.iter().any(|e| matches!(e, Event::Chord(c) if c.staff == staff))
}

/// Whether the bar's `staff` is a written line (single notes), which the
/// chart writes as notes rather than rhythm slashes.
fn is_line(m: &Measure, staff: u8, style: Style) -> bool {
    if style == Style::Rhythm {
        return false;
    }
    let attacks: Vec<&Chord> = first_voice(m, staff)
        .into_iter()
        .filter_map(|e| match e {
            Event::Chord(c) => Some(c),
            Event::Rest(_) => None,
        })
        .collect();
    // A voicing or a rhythm slash: comping, not a line.
    let stacked = attacks
        .iter()
        .filter(|c| c.notes.len() > 1 || c.articulations.contains("rhythm-slash"))
        .count();
    !attacks.is_empty() && (style == Style::Notation || stacked * 3 < attacks.len() * 2)
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
                // `~` after a note starts a tie, before one ends it.
                let into = if c.notes.iter().any(|n| n.tie_stop) { "~" } else { "" };
                // After the value: `~` ties on, `)` ends a slur, `(` starts one.
                // Articulations as LilyPond writes them.
                let arts: String = [
                    ("staccato", "-."),
                    ("accent", "->"),
                    ("tenuto", "--"),
                    ("strong-accent", "-^"),
                    ("staccatissimo", "-!"),
                    ("falloff", "-fall"),
                    ("fermata", "-fermata"),
                ]
                .iter()
                .filter(|(name, _)| c.articulations.contains(*name))
                .map(|(_, mark)| *mark)
                .collect();
                let on = format!(
                    "{}{}{}{}",
                    if c.notes.iter().any(|n| n.tie_start) { "~" } else { "" },
                    arts,
                    if c.slur_stop { ")" } else { "" },
                    if c.slur_start { "(" } else { "" },
                );
                if c.notes.len() == 1 {
                    parts.push(format!("{into}{}{v}{on}", pitch(&c.notes[0].pitch)));
                } else {
                    let mut ps: Vec<&WrittenPitch> = c.notes.iter().map(|n| &n.pitch).collect();
                    ps.sort_by_key(|p| p.midi());
                    let inner: Vec<String> = ps.iter().map(|p| pitch(p)).collect();
                    parts.push(format!("{into}<{}>{v}{on}", inner.join(" ")));
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
