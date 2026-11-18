//! A bar's chord symbols as note input edits them: the chord line of a
//! chord chart, what these charts are first. Each slot is a chord symbol
//! for a length (`F#m_4`), a rest (`r4`), or the chord before held on.

use keyflow_proto::chart::types::{Measure, RhythmElement};
use keyflow_proto::time::{MusicalDuration, TimeSignature};

use super::music::{Duration, TICKS_PER_WHOLE, bar_ticks};

/// What a slot of a bar's chord line holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Slot {
    /// A chord symbol, as written (`F#m7`, `!B`, `'C`, `4`): its marks and
    /// its notation (letters, degrees) kept.
    Chord(String),
    /// No chord: a rest.
    Rest,
    /// The chord before still sounding (the bar starts on it).
    Hold,
}

/// One slot and its length.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChordEvent {
    pub slot: Slot,
    pub duration: Duration,
}

/// A bar's chord symbols, one after another, filling the bar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChordBar {
    pub length: u32,
    pub events: Vec<ChordEvent>,
    /// Why its chords cannot be edited here, when they cannot.
    pub locked: Option<String>,
}

/// Ticks of `d` (beats as the measure counts them) in `time`.
fn ticks_of(d: &MusicalDuration, time: (u8, u8)) -> u32 {
    use keyflow_proto::time::MusicalPositionExt as _;
    let sig = TimeSignature::new(u32::from(time.0), u32::from(time.1));
    let beats = d.to_beats(sig);
    (beats * f64::from(TICKS_PER_WHOLE) / f64::from(time.1.max(1))).round() as u32
}

impl ChordBar {
    /// A bar of `(numerator, denominator)` with no chord: a rest.
    #[must_use]
    pub fn empty(time: (u8, u8)) -> Self {
        let length = bar_ticks(time);
        Self {
            length,
            events: Duration::split(length)
                .into_iter()
                .map(|d| ChordEvent {
                    slot: Slot::Rest,
                    duration: d,
                })
                .collect(),
            locked: None,
        }
    }

    /// `measure`'s chord line.
    #[must_use]
    pub fn from_measure(measure: &Measure) -> Self {
        let time = measure.time_signature;
        let length = bar_ticks(time);
        let mut bar = Self {
            length,
            events: Vec::new(),
            locked: None,
        };
        let mut slots: Vec<(Slot, u32)> = Vec::new();
        // Its rhythm, when written, if it fills the bar (`C / / /` keeps its
        // chord at three beats there): else its chords by where they start.
        let rhythm_fills = !measure.rhythm_elements.is_empty()
            && measure
                .rhythm_elements
                .iter()
                .map(|r| ticks_of(r.duration(), time))
                .sum::<u32>()
                == length;
        if !rhythm_fills {
            // A chord sounds until the next one: each lasts from where it
            // starts to where the next starts (the last, to the barline).
            let starts: Vec<u32> = measure
                .chords
                .iter()
                .map(|c| ticks_of(&c.position.total_duration, time) % length.max(1))
                .collect();
            if starts.windows(2).any(|w| w[1] <= w[0]) {
                bar.locked = Some("its chords are not one after another".to_owned());
                return bar;
            }
            if let Some(&first) = starts.first()
                && first > 0
            {
                slots.push((Slot::Hold, first));
            }
            for (i, c) in measure.chords.iter().enumerate() {
                let end = starts.get(i + 1).copied().unwrap_or(length);
                slots.push((Slot::Chord(token(&c.original_token)), end - starts[i]));
            }
            if measure.chords.is_empty() {
                slots.push((Slot::Hold, length));
            }
        } else {
            for r in &measure.rhythm_elements {
                match r {
                    RhythmElement::Chord(c) => {
                        slots.push((
                            Slot::Chord(token(&c.original_token)),
                            ticks_of(&c.duration, time),
                        ));
                    }
                    RhythmElement::Rest(r) => slots.push((Slot::Rest, ticks_of(&r.duration, time))),
                    RhythmElement::Space(s) => {
                        slots.push((Slot::Hold, ticks_of(&s.duration, time)))
                    }
                }
            }
        }
        let total: u32 = slots.iter().map(|s| s.1).sum();
        if total != length {
            bar.locked = Some("its chords do not fill it exactly".to_owned());
            return bar;
        }
        for (slot, len) in slots.into_iter().filter(|s| s.1 > 0) {
            let parts = Duration::split(len);
            if parts.len() != 1 {
                bar.locked = Some("a chord in it lasts no single note value".to_owned());
                return bar;
            }
            bar.events.push(ChordEvent {
                slot,
                duration: parts[0],
            });
        }
        bar
    }

    /// Where each slot starts.
    #[must_use]
    pub fn starts(&self) -> Vec<u32> {
        let mut at = 0;
        self.events
            .iter()
            .map(|e| {
                let s = at;
                at += e.duration.ticks();
                s
            })
            .collect()
    }

    /// The slot sounding at `tick`.
    #[must_use]
    pub fn index_at(&self, tick: u32) -> Option<usize> {
        self.starts()
            .iter()
            .zip(&self.events)
            .position(|(s, e)| tick >= *s && tick < s + e.duration.ticks())
    }

    /// `slot` over `[at, at + len)`, overwriting; what is left of a cut
    /// chord keeps sounding (`Hold`), of a cut rest stays rest. Returns the
    /// index of the new slot's first value.
    pub fn overwrite(&mut self, at: u32, len: u32, slot: &Slot) -> usize {
        let end = at + len;
        let mut out: Vec<ChordEvent> = Vec::new();
        let mut first = None;
        let mut tick = 0;
        let fill = |ticks: u32, what: Slot| {
            Duration::split(ticks).into_iter().map(move |d| ChordEvent {
                slot: what.clone(),
                duration: d,
            })
        };
        let new = |out: &mut Vec<ChordEvent>, first: &mut Option<usize>| {
            if first.is_none() {
                *first = Some(out.len());
                // A chord over a length no one value has: the chord, then held.
                let mut parts = Duration::split(len).into_iter();
                if let Some(d) = parts.next() {
                    out.push(ChordEvent {
                        slot: slot.clone(),
                        duration: d,
                    });
                }
                let after = if matches!(slot, Slot::Rest) {
                    Slot::Rest
                } else {
                    Slot::Hold
                };
                out.extend(parts.map(|d| ChordEvent {
                    slot: after.clone(),
                    duration: d,
                }));
            }
        };
        for e in &self.events {
            let (s, t) = (tick, tick + e.duration.ticks());
            tick = t;
            if t <= at || s >= end {
                if s >= end {
                    new(&mut out, &mut first);
                }
                out.push(e.clone());
                continue;
            }
            if s < at {
                out.extend(fill(at - s, e.slot.clone()));
            }
            new(&mut out, &mut first);
            if t > end {
                let rest = if matches!(e.slot, Slot::Rest) {
                    Slot::Rest
                } else {
                    Slot::Hold
                };
                out.extend(fill(t - end, rest));
            }
        }
        new(&mut out, &mut first);
        self.events = out;
        first.unwrap_or(0)
    }

    /// The bar's chord line as Keyflow writes it (`F#m_4 B_4 E_2`), or why
    /// it cannot be: a bar that starts on the chord before has no token
    /// that says so alone.
    ///
    /// # Errors
    /// A held slot.
    pub fn to_text(&self) -> Result<String, String> {
        // A held slot lengthens the chord (or rest) before it: a chord
        // sounds until the next one.
        let mut runs: Vec<(Slot, u32)> = Vec::new();
        for e in &self.events {
            match (&e.slot, runs.last_mut()) {
                (Slot::Hold, Some(last)) => last.1 += e.duration.ticks(),
                (Slot::Hold, None) => {
                    return Err(
                        "this bar starts on the chord before it, which cannot be written alone yet"
                            .to_owned(),
                    );
                }
                (slot, _) => runs.push((slot.clone(), e.duration.ticks())),
            }
        }
        let mut out: Vec<String> = Vec::new();
        for (slot, ticks) in runs {
            let parts = Duration::split(ticks);
            let [d] = parts.as_slice() else {
                return Err("a chord in it would last no single note value".to_owned());
            };
            let dur = format!("{}{}", d.value.lilypond(), ".".repeat(usize::from(d.dots)));
            out.push(match slot {
                Slot::Chord(t) => format!("{t}_{dur}"),
                _ => format!("r{dur}"),
            });
        }
        Ok(out.join(" "))
    }

    /// The bar as its written text would read back: held slots folded into
    /// the chord before them (what [`ChordBar::to_text`] writes).
    #[must_use]
    pub fn folded(&self) -> Vec<(Slot, u32)> {
        let mut runs: Vec<(Slot, u32)> = Vec::new();
        for e in &self.events {
            match (&e.slot, runs.last_mut()) {
                (Slot::Hold, Some(last)) => last.1 += e.duration.ticks(),
                (slot, _) => runs.push((slot.clone(), e.duration.ticks())),
            }
        }
        runs
    }
}

/// A chord token without its written length: `F#m_4` → `F#m`.
fn token(original: &str) -> String {
    original.split('_').next().unwrap_or(original).to_owned()
}

/// `token`'s root moved a semitone (`up`: spelled with sharps), its
/// marks (`!`, `>`, `'`) and quality kept: `!F#m7` → `!Gm7`. `None` for a
/// token whose root is not a letter (a degree).
#[must_use]
pub fn transpose(token: &str, up: bool) -> Option<String> {
    let marks_len = token
        .char_indices()
        .find(|(_, c)| c.is_ascii_alphabetic())
        .map(|(i, _)| i)?;
    let (marks, rest) = token.split_at(marks_len);
    let mut chars = rest.chars();
    let letter = chars.next()?.to_ascii_uppercase();
    if !('A'..='G').contains(&letter) {
        return None;
    }
    let after: String = chars.collect();
    let (alter, quality) = if let Some(q) = after.strip_prefix('#') {
        (1, q)
    } else if let Some(q) = after.strip_prefix('b') {
        (-1, q)
    } else {
        (0, after.as_str())
    };
    let p = super::music::Pitch::new(letter, alter, 4).semitone(up);
    let acc = match p.alter {
        1 => "#",
        -1 => "b",
        _ => "",
    };
    Some(format!("{marks}{}{acc}{quality}", p.letter))
}

/// A chord token's parts: its marks (`!`, `>`, `'`, `.`), its root (`F#`,
/// `Bb`, a degree `6`, `b7`), its quality (`m7`, `sus4`, empty for a major
/// triad) and its bass (`/A`, empty). `None` when it has no root.
#[must_use]
pub fn parts(token: &str) -> Option<(&str, &str, &str, &str)> {
    let start = token.find(|c: char| c.is_ascii_alphanumeric())?;
    // A degree written flat or sharp (`b7`, `#4`) starts at its accidental.
    let start = if start > 0 && matches!(&token[start - 1..start], "#") {
        start - 1
    } else {
        start
    };
    let (marks, rest) = token.split_at(start);
    let mut end = 0;
    let bytes = rest.as_bytes();
    if bytes.first().is_some_and(|c| matches!(c, b'A'..=b'G')) {
        end = 1;
        while bytes.get(end).is_some_and(|c| matches!(c, b'#' | b'b')) {
            end += 1;
        }
    } else {
        while bytes.get(end).is_some_and(|c| matches!(c, b'#' | b'b')) {
            end += 1;
        }
        let digits = rest[end..].chars().take_while(char::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        end += digits;
    }
    let (root, after) = rest.split_at(end);
    let (quality, bass) = after.find('/').map_or((after, ""), |i| after.split_at(i));
    Some((marks, root, quality, bass))
}

/// The qualities Alt+↑ and Alt+↓ step through, in order.
pub const QUALITIES: &[&str] = &[
    "", "m", "7", "m7", "maj7", "sus4", "sus2", "6", "m6", "9", "add9", "dim", "aug",
];

/// `token` with its quality the next (or, not `up`, the previous) of
/// [`QUALITIES`], its root, bass and marks kept: `G` → `Gm` → `G7`. A
/// quality not in the list goes to the first or the last. `None` for a
/// token with no root.
#[must_use]
pub fn step_quality(token: &str, up: bool) -> Option<String> {
    let (marks, root, quality, bass) = parts(token)?;
    let n = QUALITIES.len();
    let next = match QUALITIES.iter().position(|q| *q == quality) {
        Some(i) if up => (i + 1) % n,
        Some(i) => (i + n - 1) % n,
        None if up => 0,
        None => n - 1,
    };
    Some(format!("{marks}{root}{}{bass}", QUALITIES[next]))
}

/// The symbol typed for a chord, its marks kept from `old` when it writes
/// none of its own (a pushed `'G` retyped as `Gsus4` stays pushed).
#[must_use]
pub fn retyped(old: &str, typed: &str) -> String {
    let typed = typed.trim();
    let typed_marks = parts(typed).map_or("", |p| p.0);
    match parts(old) {
        Some((marks, ..)) if typed_marks.is_empty() => format!("{marks}{typed}"),
        _ => typed.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chord_s_quality_steps_and_its_root_bass_and_marks_stay() {
        assert_eq!(step_quality("G", true).as_deref(), Some("Gm"));
        assert_eq!(step_quality("Gm", true).as_deref(), Some("G7"));
        assert_eq!(step_quality("G", false).as_deref(), Some("Gaug"));
        assert_eq!(step_quality("'F#m7/A", true).as_deref(), Some("'F#maj7/A"));
        assert_eq!(step_quality("b7", true).as_deref(), Some("b7m"));
        assert_eq!(step_quality("'6m", true).as_deref(), Some("'67"));
        assert_eq!(step_quality("C13#11", true).as_deref(), Some("C"));
        assert_eq!(retyped("'G", "Gsus4"), "'Gsus4");
        assert_eq!(retyped("'G", "!Gsus4"), "!Gsus4");
    }
    use crate::edit::music::Value;
    use crate::text::chart::parse_chart;

    fn bar(text: &str) -> ChordBar {
        let chart = parse_chart(&format!("T - A\n4/4\n\nVS 1\n| {text} |\n")).unwrap();
        ChordBar::from_measure(&chart.sections[0].measures()[0])
    }

    #[test]
    fn a_bar_s_chords_are_read_with_their_lengths() {
        let b = bar("Dm7_2 G7_4 C_4");
        let names: Vec<String> = b
            .events
            .iter()
            .map(|e| format!("{:?}{}", e.slot, e.duration.value.lilypond()))
            .collect();
        assert_eq!(
            names,
            ["Chord(\"Dm7\")2", "Chord(\"G7\")4", "Chord(\"C\")4"]
        );
        assert_eq!(b.to_text().unwrap(), "Dm7_2 G7_4 C_4");
    }

    #[test]
    fn a_chord_entered_overwrites_and_what_it_cuts_holds_on() {
        let mut b = bar("C_1");
        let q = Value::Quarter.ticks();
        b.overwrite(2 * q, q, &Slot::Chord("F".into()));
        // F from beat 3 sounds to the end of the bar: a chord lasts till the next.
        assert_eq!(b.to_text().unwrap(), "C_2 F_2");
    }

    #[test]
    fn a_root_moves_a_semitone_keeping_marks_and_quality() {
        assert_eq!(transpose("!F#m7", true).as_deref(), Some("!Gm7"));
        assert_eq!(transpose("Bbmaj7", false).as_deref(), Some("Amaj7"));
        assert_eq!(transpose("E", false).as_deref(), Some("Eb"));
        assert_eq!(transpose("4", true), None);
    }
}
