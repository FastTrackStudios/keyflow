//! Thin notation IR — the written score as engraving input.
//!
//! This is deliberately NOT an editor DOM (see docs/spec/score-engraving.md):
//! it preserves exactly what layout needs from MusicXML — written pitch
//! spelling, voices, rests, clefs/keys/meters, tuplets, ties — in an
//! immutable, measure-partwise shape. Playback concerns (QN timelines, CC
//! curves) stay in `keyflow-orchestra`; chart concerns stay in
//! `keyflow-musicxml`.

use std::collections::BTreeSet;

/// A full score: ordered parts, each with the same measure grid.
#[derive(Debug, Clone, Default)]
pub struct Score {
    pub work_title: Option<String>,
    pub movement_title: Option<String>,
    pub composer: Option<String>,
    pub arranger: Option<String>,
    pub lyricist: Option<String>,
    /// `<rights>` (copyright line).
    pub rights: Option<String>,
    pub parts: Vec<Part>,
}

impl Score {
    /// Rehearsal marks at measure `index` that every extracted part shows:
    /// the score's top part carries them (MusicXML's convention, and what
    /// Finale's staff lists export), plus any `part` has of its own.
    #[must_use]
    pub fn rehearsals_for_part(&self, part: usize, index: usize) -> Vec<(u32, RehearsalMark)> {
        let of = |p: usize| -> Vec<(u32, RehearsalMark)> {
            self.parts
                .get(p)
                .and_then(|p| p.measures.get(index))
                .map(|m| {
                    m.directions
                        .iter()
                        .filter_map(|d| match &d.kind {
                            DirectionKind::Rehearsal(r) => Some((d.tick, r.clone())),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        let mut marks: Vec<(u32, RehearsalMark)> = Vec::new();
        for m in of(0) {
            if !marks.iter().any(|(_, k)| k.text == m.1.text) {
                marks.push(m);
            }
        }
        if part != 0 {
            for own in of(part) {
                if !marks.iter().any(|(_, m)| m.text == own.1.text) {
                    marks.push(own);
                }
            }
        }
        marks
    }
}

/// One instrument part (may span multiple staves, e.g. piano/harp = 2).
#[derive(Debug, Clone)]
pub struct Part {
    /// MusicXML part id (`P1`, …).
    pub id: String,
    /// Display name from `<part-list>` (entities decoded).
    pub name: String,
    /// Abbreviated name for subsequent systems (`Vln.`, `Tpt.`).
    pub abbreviation: Option<String>,
    /// Highest `<staves>` value seen (1 for single-staff parts).
    pub staves: u8,
    /// Written transposition in effect (B♭/F parts), if any.
    pub transpose: Option<Transposition>,
    pub measures: Vec<Measure>,
}

/// Written transposition (`<transpose>`): sounding = written + chromatic
/// + 12·octave_change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transposition {
    pub diatonic: i8,
    pub chromatic: i8,
    pub octave_change: i8,
}

/// One measure of one part: attribute changes + timed events.
#[derive(Debug, Clone, Default)]
pub struct Measure {
    /// MusicXML `number` attribute (usually numeric; "0" for pickups).
    pub number: String,
    /// Pickup / non-counting measure (`implicit="yes"`).
    pub implicit: bool,
    /// Attribute changes in this measure, in document order.
    pub attributes: Vec<AttrChange>,
    /// Notes/rests, each with a tick onset from measure start.
    pub events: Vec<Event>,
    /// Directions (dynamics, hairpins, texts) at tick positions.
    pub directions: Vec<Direction>,
    /// Divisions-per-quarter in effect for this measure's ticks.
    pub divisions: u32,
    /// Measure length in ticks (from the inherited meter; content may
    /// overflow on malformed input — layout clamps).
    pub len_ticks: u32,
    /// Closing barline style (`<barline location="right">`).
    pub barline: BarStyle,
    /// A repeat begins at this measure (`|:` before it).
    pub repeat_start: bool,
    /// A repeat ends with this measure (`:|` after it).
    pub repeat_end: bool,
    /// The ending (volta) that begins at this measure, by its label
    /// ("1.", "2.", "1., 2.").
    pub ending: Option<String>,
    /// How many bars the ending's bracket covers, when it was measured (0
    /// = not known).
    pub ending_bars: u32,
    /// The bracket has no closing hook (the last ending, played through).
    pub ending_open: bool,
}

/// Barline drawn at a measure's end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BarStyle {
    #[default]
    Regular,
    /// Double thin (section end).
    LightLight,
    /// Final barline.
    LightHeavy,
    HeavyLight,
    Dashed,
    None,
}

impl BarStyle {
    /// A section-ending barline: multirests break after it.
    #[must_use]
    pub fn is_section_end(self) -> bool {
        !matches!(self, BarStyle::Regular | BarStyle::Dashed | BarStyle::None)
    }
}

/// A `<direction>` at a tick position within its measure.
#[derive(Debug, Clone, PartialEq)]
pub struct Direction {
    pub tick: u32,
    pub kind: DirectionKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DirectionKind {
    /// Dynamic mark by conventional name ("p", "mf", "sfz", …).
    Dynamic(String),
    /// Hairpin start (`<wedge>` crescendo/diminuendo).
    WedgeStart { crescendo: bool },
    /// Hairpin stop.
    WedgeStop,
    /// Free staff text (`<words>`).
    Words(String),
    /// Rehearsal mark / section cue ("Verse 1", "B", "79").
    Rehearsal(RehearsalMark),
    /// Tempo mark: `Allegro ♩ = 120`, `♩ = 63 BPM`.
    Tempo(TempoMark),
    /// Chord symbol as written ("Bm", "F#m7", "Asus4"), drawn above the
    /// staff at its tick.
    Harmony(String),
}

/// A metronome mark with the words around it in the same direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TempoMark {
    /// Words before the note ("Allegro", "New Tempo:").
    pub text: Option<String>,
    /// MusicXML note type of the beat ("quarter", "eighth", "half").
    pub beat_unit: String,
    pub dots: u8,
    /// Beats per minute as written ("63", "120-126").
    pub per_minute: String,
    /// Words after the number ("BPM").
    pub suffix: Option<String>,
}

/// A rehearsal mark with the enclosure and font the source gave it.
#[derive(Debug, Clone, PartialEq)]
pub struct RehearsalMark {
    pub text: String,
    pub enclosure: Enclosure,
    /// Point size, when the source states one.
    pub font_size: Option<f64>,
    pub bold: bool,
    pub italic: bool,
    /// Source font family, when stated ("Arial Bold", "Times New Roman").
    pub font_family: Option<String>,
}

impl RehearsalMark {
    /// A plain boxed mark (MusicXML's default rehearsal look).
    #[must_use]
    pub fn boxed(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            enclosure: Enclosure::Rectangle,
            font_size: None,
            bold: true,
            italic: false,
            font_family: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Enclosure {
    #[default]
    Rectangle,
    Oval,
    None,
}

/// A mid-stream attribute change (clef/key/time/divisions/staves).
#[derive(Debug, Clone, Default)]
pub struct AttrChange {
    /// Tick position within the measure (usually 0).
    pub tick: u32,
    pub divisions: Option<u32>,
    /// Key signature as circle-of-fifths count.
    pub key_fifths: Option<i8>,
    /// Time signature (beats, beat-type).
    pub time: Option<(u32, u32)>,
    pub staves: Option<u8>,
    /// Clef changes: (staff number 1-based, clef).
    pub clefs: Vec<(u8, Clef)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clef {
    pub sign: ClefSign,
    /// Staff line (2 = treble G, 4 = bass F, 3 = alto C…). None = sign default.
    pub line: Option<u8>,
    /// Octave transposition marks (−1 = tenor-voice G clef with 8 below).
    pub octave_change: i8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClefSign {
    G,
    F,
    C,
    Percussion,
    Tab,
    None,
}

/// A timed measure event: a chord (≥1 notes sharing a stem) or a rest.
#[derive(Debug, Clone)]
pub enum Event {
    Chord(Chord),
    Rest(Rest),
}

impl Event {
    #[must_use]
    pub fn tick(&self) -> u32 {
        match self {
            Event::Chord(c) => c.tick,
            Event::Rest(r) => r.tick,
        }
    }

    #[must_use]
    pub fn voice(&self) -> u32 {
        match self {
            Event::Chord(c) => c.voice,
            Event::Rest(r) => r.voice,
        }
    }
}

/// Written duration info shared by chords and rests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WrittenDuration {
    /// Written note type (`<type>`), if given. Whole=1 … 1024th.
    pub note_type: Option<NoteTypeValue>,
    pub dots: u8,
    /// Tuplet time modification (actual, normal), e.g. (3, 2) for triplets.
    pub tuplet: Option<(u8, u8)>,
}

/// One stem's worth of notes (MusicXML principal note + `<chord>` followers).
#[derive(Debug, Clone)]
pub struct Chord {
    /// Tick onset from measure start.
    pub tick: u32,
    /// Duration in ticks (0 for grace notes).
    pub duration: u32,
    pub voice: u32,
    /// Staff within the part, 1-based.
    pub staff: u8,
    pub written: WrittenDuration,
    pub grace: bool,
    /// Stacked notes, in document order.
    pub notes: Vec<Note>,
    pub slur_start: bool,
    pub slur_stop: bool,
    /// Open articulation tag set (staccato, accent, fermata, …), MusicXML
    /// kebab-case child names.
    pub articulations: BTreeSet<String>,
    /// Articulations the source pins to a side (`placement`): tag → above.
    /// Unlisted marks follow the stem rule.
    pub articulation_above: std::collections::BTreeMap<String, bool>,
}

#[derive(Debug, Clone)]
pub struct Note {
    pub pitch: WrittenPitch,
    pub tie_start: bool,
    pub tie_stop: bool,
    /// Printed accidental (`<accidental>` text: "sharp", "natural", …),
    /// when the file states one explicitly.
    pub accidental: Option<String>,
}

/// Written pitch spelling: step letter + alteration + octave. Never reduced
/// to MIDI — F♯ and G♭ engrave differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrittenPitch {
    pub step: Step,
    /// Chromatic alteration: −2 (𝄫) … +2 (𝄪).
    pub alter: i8,
    /// Scientific octave (4 = middle-C octave).
    pub octave: i8,
}

impl WrittenPitch {
    /// Sounding MIDI note number (before part transposition).
    #[must_use]
    pub fn midi(&self) -> i32 {
        let step = match self.step {
            Step::C => 0,
            Step::D => 2,
            Step::E => 4,
            Step::F => 5,
            Step::G => 7,
            Step::A => 9,
            Step::B => 11,
        };
        (i32::from(self.octave) + 1) * 12 + step + i32::from(self.alter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
}

/// Written note type values (`<type>`), longest to shortest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteTypeValue {
    Maxima,
    Long,
    Breve,
    Whole,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
    SixtyFourth,
    OneHundredTwentyEighth,
    TwoHundredFiftySixth,
    FiveHundredTwelfth,
    OneThousandTwentyFourth,
}

#[derive(Debug, Clone)]
pub struct Rest {
    pub tick: u32,
    pub duration: u32,
    pub voice: u32,
    pub staff: u8,
    pub written: WrittenDuration,
    /// `<rest measure="yes"/>` — a whole-measure rest regardless of meter.
    pub measure_rest: bool,
}
