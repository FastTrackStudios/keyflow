//! The values a note-input edit is made of: durations, pitches, and a
//! bar's notes.

use keyflow_proto::chart::melody::{MelodyNote, OctaveModifier};
use keyflow_proto::chart::types::Measure;

/// Ticks in a whole note. Divisible by every note value down to a dotted
/// 64th (6 × 1.5 = 9) and by three, for tuplets later.
pub const TICKS_PER_WHOLE: u32 = 384;

/// A note value, MuseScore's duration keys 1–7: 1 a 64th up to 7 a whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Value {
    SixtyFourth,
    ThirtySecond,
    Sixteenth,
    Eighth,
    Quarter,
    Half,
    Whole,
}

impl Value {
    /// Every value, shortest first.
    pub const ALL: [Self; 7] = [
        Self::SixtyFourth,
        Self::ThirtySecond,
        Self::Sixteenth,
        Self::Eighth,
        Self::Quarter,
        Self::Half,
        Self::Whole,
    ];

    /// Its ticks, undotted.
    #[must_use]
    pub const fn ticks(self) -> u32 {
        TICKS_PER_WHOLE >> (6 - self.index())
    }

    /// Its place, 0 (a 64th) to 6 (a whole).
    #[must_use]
    pub const fn index(self) -> u32 {
        match self {
            Self::SixtyFourth => 0,
            Self::ThirtySecond => 1,
            Self::Sixteenth => 2,
            Self::Eighth => 3,
            Self::Quarter => 4,
            Self::Half => 5,
            Self::Whole => 6,
        }
    }

    /// MuseScore's duration key for it (`1` a 64th … `5` a quarter … `7` a
    /// whole).
    #[must_use]
    pub const fn key(self) -> u8 {
        self.index() as u8 + 1
    }

    /// The value MuseScore's duration key `key` chooses.
    #[must_use]
    pub fn from_key(key: u8) -> Option<Self> {
        Self::ALL.get(usize::from(key.checked_sub(1)?)).copied()
    }

    /// Its Lilypond number: 4 a quarter, 8 an eighth.
    #[must_use]
    pub const fn lilypond(self) -> u8 {
        1 << (6 - self.index())
    }

    /// The value of Lilypond number `n`.
    #[must_use]
    pub fn from_lilypond(n: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.lilypond() == n)
    }

    /// Its name, as a status line says it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::SixtyFourth => "64th",
            Self::ThirtySecond => "32nd",
            Self::Sixteenth => "16th",
            Self::Eighth => "Eighth",
            Self::Quarter => "Quarter",
            Self::Half => "Half",
            Self::Whole => "Whole",
        }
    }
}

/// A written duration: a note value and its dots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Duration {
    pub value: Value,
    pub dots: u8,
}

impl Duration {
    #[must_use]
    pub const fn new(value: Value) -> Self {
        Self { value, dots: 0 }
    }

    #[must_use]
    pub const fn dotted(value: Value) -> Self {
        Self { value, dots: 1 }
    }

    /// Its length in ticks.
    #[must_use]
    pub const fn ticks(self) -> u32 {
        let base = self.value.ticks();
        let mut total = base;
        let mut add = base;
        let mut d = 0;
        while d < self.dots {
            add /= 2;
            total += add;
            d += 1;
        }
        total
    }

    /// `ticks` as written durations, longest first, each a value or a
    /// dotted one: how a remainder is filled (the rest after a cut) or a
    /// note split at a barline is written.
    #[must_use]
    pub fn split(mut ticks: u32) -> Vec<Self> {
        let mut out = Vec::new();
        while ticks > 0 {
            let fit = Value::ALL
                .iter()
                .rev()
                .flat_map(|v| [Self::dotted(*v), Self::new(*v)])
                .find(|d| d.ticks() <= ticks);
            let Some(d) = fit else { break };
            out.push(d);
            ticks -= d.ticks();
        }
        out
    }

    /// Its name, as a status line says it: `Dotted quarter`.
    #[must_use]
    pub fn name(self) -> String {
        match self.dots {
            0 => self.value.name().to_owned(),
            1 => format!("Dotted {}", self.value.name().to_lowercase()),
            _ => format!("Double-dotted {}", self.value.name().to_lowercase()),
        }
    }
}

/// A written pitch: a letter, its alteration (−2 to +2), its octave
/// (scientific: middle C is C4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pitch {
    pub letter: char,
    pub alter: i8,
    pub octave: i8,
    /// A natural sign written on it (`Bn`): a courtesy, or cancelling an
    /// accidental earlier in the bar. Only with `alter` 0.
    pub natural: bool,
}

const LETTERS: &str = "CDEFGAB";

/// Semitones above C of each letter.
fn letter_semitone(letter: char) -> i32 {
    match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        _ => 11,
    }
}

impl Pitch {
    #[must_use]
    pub const fn new(letter: char, alter: i8, octave: i8) -> Self {
        Self {
            letter,
            alter,
            octave,
            natural: false,
        }
    }

    /// It with a natural sign written on it.
    #[must_use]
    pub const fn with_natural(mut self, natural: bool) -> Self {
        self.natural = natural && self.alter == 0;
        self
    }

    /// Its MIDI number (C4 = 60).
    #[must_use]
    pub fn midi(self) -> i32 {
        12 * (i32::from(self.octave) + 1) + letter_semitone(self.letter) + i32::from(self.alter)
    }

    /// Its staff step: letters counted from C0, a line or a space each.
    #[must_use]
    pub fn step(self) -> i32 {
        i32::from(self.octave) * 7 + LETTERS.find(self.letter).map_or(0, |i| i as i32)
    }

    /// The natural pitch at staff step `step`.
    #[must_use]
    pub fn from_step(step: i32) -> Self {
        let letter = LETTERS
            .chars()
            .nth(step.rem_euclid(7) as usize)
            .unwrap_or('C');
        Self::new(letter, 0, step.div_euclid(7) as i8)
    }

    /// `letter` with `alter`, in the octave that puts it nearest `near`
    /// (MuseScore: "the octave that is closest to the previous note").
    #[must_use]
    pub fn nearest(letter: char, alter: i8, near: Self) -> Self {
        (near.octave - 1..=near.octave + 1)
            .map(|o| Self::new(letter, alter, o))
            .min_by_key(|p| (p.midi() - near.midi()).abs())
            .unwrap_or(Self::new(letter, alter, near.octave))
    }

    /// One semitone up (spelled with sharps) or down (with flats), as
    /// MuseScore's ↑ and ↓.
    #[must_use]
    pub fn semitone(self, up: bool) -> Self {
        let target = self.midi() + if up { 1 } else { -1 };
        let alters: [i8; 2] = if up { [0, 1] } else { [0, -1] };
        for alter in alters {
            for letter in LETTERS.chars() {
                let pc = (letter_semitone(letter) + i32::from(alter)).rem_euclid(12);
                if pc == target.rem_euclid(12) {
                    let octave = (target - letter_semitone(letter) - i32::from(alter)) / 12 - 1;
                    return Self::new(letter, alter, octave as i8);
                }
            }
        }
        self
    }

    /// An octave up or down.
    #[must_use]
    pub const fn octave_shift(self, up: bool) -> Self {
        Self::new(
            self.letter,
            self.alter,
            if up { self.octave + 1 } else { self.octave - 1 },
        )
    }

    /// Its name with octave: `F♯4`, `B♭3`.
    #[must_use]
    pub fn name(self) -> String {
        let accidental = match self.alter {
            2 => "𝄪",
            1 => "♯",
            -1 => "♭",
            -2 => "𝄫",
            _ => "",
        };
        format!("{}{accidental}{}", self.letter, self.octave)
    }
}

/// What an event is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    Note(Pitch),
    Rest,
}

/// The articulations a melody note can carry, by name, as the melody
/// model writes them (`-.` staccato, `->` accent …): bit `i` is `MARKS[i]`.
pub const MARKS: [&str; 7] = [
    "staccato",
    "accent",
    "tenuto",
    "marcato",
    "staccatissimo",
    "fall",
    "fermata",
];

/// One note or rest of a bar's line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub kind: EventKind,
    pub duration: Duration,
    /// Tied into the next note.
    pub tie: bool,
    /// Its articulations ([`MARKS`], a bit each).
    pub marks: u8,
}

impl Event {
    #[must_use]
    pub const fn rest(duration: Duration) -> Self {
        Self {
            kind: EventKind::Rest,
            duration,
            tie: false,
            marks: 0,
        }
    }

    #[must_use]
    pub const fn note(pitch: Pitch, duration: Duration) -> Self {
        Self {
            kind: EventKind::Note(pitch),
            duration,
            tie: false,
            marks: 0,
        }
    }

    #[must_use]
    pub const fn pitch(&self) -> Option<Pitch> {
        match self.kind {
            EventKind::Note(p) => Some(p),
            EventKind::Rest => None,
        }
    }
}

/// A bar's line of notes and rests, one after another, filling the bar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bar {
    /// Its length in ticks (its time signature's).
    pub length: u32,
    pub events: Vec<Event>,
    /// Whether the chart wrote a melody in it (`m{ }`): a bar without one
    /// is all rest here, and gets a melody when a note is entered.
    pub written: bool,
    /// Why it cannot be edited here, when it cannot (a tuplet, a chord of
    /// notes, a line longer than the bar): the edit is refused rather than
    /// losing what the bar holds.
    pub locked: Option<String>,
}

impl Bar {
    /// An empty bar of `(numerator, denominator)`: rests.
    #[must_use]
    pub fn empty(time: (u8, u8)) -> Self {
        let length = bar_ticks(time);
        Self {
            length,
            events: Duration::split(length)
                .into_iter()
                .map(Event::rest)
                .collect(),
            written: false,
            locked: None,
        }
    }

    /// `measure`'s line: its first melody's notes, or rests.
    #[must_use]
    pub fn from_measure(measure: &Measure) -> Self {
        let mut bar = Self::empty(measure.time_signature);
        let Some(melody) = measure.melodies.first() else {
            return bar;
        };
        bar.written = true;
        let mut events = Vec::new();
        for note in &melody.notes {
            match event_of(note) {
                Ok(e) => events.push(e),
                Err(why) => {
                    bar.locked = Some(why);
                    return bar;
                }
            }
        }
        let filled: u32 = events.iter().map(|e| e.duration.ticks()).sum();
        if filled > bar.length {
            bar.locked = Some("its notes run past its end".to_owned());
            return bar;
        }
        // A line shorter than its bar ends in rest.
        events.extend(
            Duration::split(bar.length - filled)
                .into_iter()
                .map(Event::rest),
        );
        bar.events = events;
        bar
    }

    /// Where each event starts, in ticks.
    #[must_use]
    pub fn starts(&self) -> Vec<u32> {
        let mut at = 0;
        self.events
            .iter()
            .map(|e| {
                let start = at;
                at += e.duration.ticks();
                start
            })
            .collect()
    }

    /// The index of the event sounding at tick `tick`.
    #[must_use]
    pub fn index_at(&self, tick: u32) -> Option<usize> {
        self.starts()
            .iter()
            .zip(&self.events)
            .position(|(s, e)| tick >= *s && tick < s + e.duration.ticks())
    }

    /// `kind` written over `[at, at + len)` (within the bar), as
    /// MuseScore's note input overwrites: what was there is cut, and what
    /// is left of a cut event becomes rest. The new event is written as as
    /// many tied values as `len` needs; `tie_last` ties its last into the
    /// next bar. Returns the index of its first value.
    pub fn overwrite(&mut self, at: u32, len: u32, kind: EventKind, tie_last: bool) -> usize {
        let end = at + len;
        let rests = |ticks: u32| Duration::split(ticks).into_iter().map(Event::rest);
        let mut out: Vec<Event> = Vec::new();
        let mut inserted = None;
        let mut tick = 0;
        for event in &self.events {
            let (s, e) = (tick, tick + event.duration.ticks());
            tick = e;
            if e <= at || s >= end {
                if s >= end && inserted.is_none() {
                    inserted = Some(out.len());
                    out.extend(new_events(kind, len, tie_last));
                }
                out.push(*event);
                continue;
            }
            if s < at {
                out.extend(rests(at - s));
            }
            if inserted.is_none() {
                inserted = Some(out.len());
                out.extend(new_events(kind, len, tie_last));
            }
            if e > end {
                out.extend(rests(e - end));
            }
        }
        let first = *inserted.get_or_insert_with(|| {
            let i = out.len();
            out.extend(new_events(kind, len, tie_last));
            i
        });
        self.events = out;
        self.written = true;
        first
    }

    /// The bar's line as Keyflow writes a melody: `m{ F#:44 r2 }`, every
    /// octave explicit, so the bar reads the same wherever it moves.
    #[must_use]
    pub fn to_melody(&self) -> String {
        let notes: Vec<String> = self
            .events
            .iter()
            .map(|e| melody_note(e).to_string())
            .collect();
        format!("m{{ {} }}", notes.join(" "))
    }
}

/// The new event `kind` over `len` ticks: one value if one fits, else
/// tied values (a rest is not tied).
fn new_events(kind: EventKind, len: u32, tie_last: bool) -> Vec<Event> {
    let parts = Duration::split(len);
    let n = parts.len();
    parts
        .into_iter()
        .enumerate()
        .map(|(i, d)| Event {
            kind,
            duration: d,
            tie: matches!(kind, EventKind::Note(_)) && (i + 1 < n || tie_last),
            marks: 0,
        })
        .collect()
}

/// A bar's ticks in `(numerator, denominator)`.
#[must_use]
pub fn bar_ticks((num, den): (u8, u8)) -> u32 {
    u32::from(num.max(1)) * TICKS_PER_WHOLE / u32::from(den.max(1))
}

fn event_of(note: &MelodyNote) -> Result<Event, String> {
    if note.triplet {
        return Err("it holds a tuplet, which note input does not edit yet".to_owned());
    }
    if !note.extra_pitches.is_empty() {
        return Err("it holds a chord of notes, which note input does not edit yet".to_owned());
    }
    let value = Value::from_lilypond(note.duration)
        .ok_or_else(|| format!("a 1/{} note is not one note input edits", note.duration))?;
    let duration = Duration {
        value,
        dots: u8::from(note.dotted),
    };
    let mut marks = 0u8;
    for name in &note.articulations {
        let bit = MARKS
            .iter()
            .position(|m| m == name)
            .ok_or_else(|| format!("its `{name}` mark is not one note input keeps yet"))?;
        marks |= 1 << bit;
    }
    if note.is_rest() || note.pitch == "s" {
        return Ok(Event {
            marks,
            ..Event::rest(duration)
        });
    }
    let mut chars = note.pitch.chars();
    let letter = chars
        .next()
        .map(|c| c.to_ascii_uppercase())
        .filter(|c| ('A'..='G').contains(c))
        .ok_or_else(|| format!("`{}` is not a pitch note input edits", note.pitch))?;
    let rest: String = chars.collect();
    let alter = rest.chars().fold(0i8, |a, c| match c {
        '#' => a + 1,
        'b' => a - 1,
        _ => a,
    });
    let natural = rest.contains('n');
    let octave = note.octave.ok_or("a note without a resolved octave")?;
    Ok(Event {
        kind: EventKind::Note(Pitch::new(letter, alter, octave as i8).with_natural(natural)),
        duration,
        tie: note.tie_start,
        marks,
    })
}

/// `event` as the melody model writes it.
fn melody_note(event: &Event) -> MelodyNote {
    let d = event.duration;
    let mut note = match event.kind {
        EventKind::Rest => MelodyNote::rest(d.value.lilypond()),
        EventKind::Note(p) => {
            let accidental = match p.alter {
                2 => "##",
                1 => "#",
                -1 => "b",
                -2 => "bb",
                _ if p.natural => "n",
                _ => "",
            };
            let mut n = MelodyNote::new(format!("{}{accidental}", p.letter), d.value.lilypond());
            n.octave = Some(p.octave.max(0) as u8);
            n.octave_modifier = OctaveModifier::None;
            n.tie_start = event.tie;
            n
        }
    };
    note.dotted = d.dots > 0;
    note.articulations = MARKS
        .iter()
        .enumerate()
        .filter(|(i, _)| event.marks & (1 << i) != 0)
        .map(|(_, m)| (*m).to_owned())
        .collect();
    note
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_are_musescore_s_keys_and_ticks() {
        assert_eq!(Value::from_key(5), Some(Value::Quarter));
        assert_eq!(Value::from_key(4), Some(Value::Eighth));
        assert_eq!(Value::from_key(6), Some(Value::Half));
        assert_eq!(Value::from_key(1), Some(Value::SixtyFourth));
        assert_eq!(Value::from_key(7), Some(Value::Whole));
        assert_eq!(Value::from_key(8), None);
        assert_eq!(Duration::new(Value::Quarter).ticks(), 96);
        assert_eq!(Duration::dotted(Value::Quarter).ticks(), 144);
        assert_eq!(Duration::dotted(Value::SixtyFourth).ticks(), 9);
        assert_eq!(
            Duration::split(96 + 48 + 24),
            vec![
                Duration::dotted(Value::Quarter),
                Duration::new(Value::Sixteenth)
            ]
        );
    }

    #[test]
    fn pitches_move_as_musescore_moves_them() {
        let c4 = Pitch::new('C', 0, 4);
        assert_eq!(c4.midi(), 60);
        assert_eq!(
            c4.semitone(true),
            Pitch::new('C', 1, 4),
            "up spells with sharps"
        );
        assert_eq!(c4.semitone(false), Pitch::new('B', 0, 3), "down to B");
        assert_eq!(
            Pitch::new('E', 0, 4).semitone(false),
            Pitch::new('E', -1, 4),
            "down spells with flats"
        );
        assert_eq!(Pitch::nearest('B', 0, c4), Pitch::new('B', 0, 3), "B below");
        assert_eq!(Pitch::nearest('D', 0, c4), Pitch::new('D', 0, 4));
        assert_eq!(c4.octave_shift(true), Pitch::new('C', 0, 5));
        assert_eq!(Pitch::from_step(c4.step() + 1), Pitch::new('D', 0, 4));
    }

    #[test]
    fn an_overwrite_cuts_what_was_there_and_rests_the_rest() {
        let mut bar = Bar::empty((4, 4));
        let q = Value::Quarter.ticks();
        let at = bar.overwrite(q, q, EventKind::Note(Pitch::new('D', 0, 4)), false);
        assert_eq!(at, 1);
        assert_eq!(bar.starts(), vec![0, q, 2 * q]);
        assert_eq!(bar.events[0], Event::rest(Duration::new(Value::Quarter)));
        assert_eq!(bar.events[2], Event::rest(Duration::new(Value::Half)));
        // A dotted half over the D and the half rest after it.
        bar.overwrite(q, 3 * q, EventKind::Note(Pitch::new('E', 0, 4)), false);
        assert_eq!(bar.events.len(), 2);
        assert_eq!(bar.events[1].duration, Duration::dotted(Value::Half));
    }
}
