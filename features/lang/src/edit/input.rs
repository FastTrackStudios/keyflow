//! Note input, as MuseScore's: the state a person's input is in, and the
//! operations that change the chart's bars.
//!
//! Three ways in ([`Mode`]): **by note name** (choose a duration, then
//! type A–G: the note is entered at that duration and the cursor moves on,
//! MuseScore's default), **by duration** (move the pitch cursor to the
//! pitch, then the duration key enters it), and **syntax** (Keyflow's own
//! text typed at the cursor, [`super::commit::splice_syntax`]). Normal mode
//! edits what is selected.
//!
//! What MuseScore does and this does too: overwrite at the cursor (no
//! insert), the octave nearest the previous note, the key signature's
//! accidental unless one is armed (`-` `+` `=`, for the next note), ↑/↓ a
//! semitone (sharps up, flats down), Ctrl+↑/↓ an octave, `.` a dot, `0` a
//! rest, `T` a tied note of the same pitch, Del a rest in a note's place, a
//! note too long for the bar split and tied over the barline, Esc back to
//! normal mode. What it does not do yet: chords (Shift+letter), voices
//! 2–4, tuplets, insert, and an accidental carried through the bar.

use keyflow_proto::Chart;

use super::chords::{ChordBar, Slot, transpose};
use super::music::{Bar, Duration, Event, EventKind, Pitch, TICKS_PER_WHOLE, Value};

/// Where an input is: a bar (the chart's bars counted from 0 across its
/// sections, as the engraver numbers them) and a tick in it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Pos {
    pub bar: usize,
    pub tick: u32,
}

/// The chart's bars as note input edits them, and its key signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Score {
    /// Each bar's line in the voice being entered ([`Self::voice`]).
    pub bars: Vec<Bar>,
    /// The voice `bars` holds, 0 to 3.
    pub voice: u8,
    /// Every voice's lines, the one being entered left empty while it is
    /// `bars` (see [`Self::set_voice`]).
    pub others: Vec<Vec<Bar>>,
    /// Each bar's chord symbols.
    pub chords: Vec<ChordBar>,
    /// Why a bar cannot be written back, when it has no text of its own
    /// (the parser's reason): it is shown, and an entry there refused.
    pub read_only: Vec<Option<String>>,
    /// The key signature's alteration of each letter, C to B.
    pub key: [i8; 7],
}

impl Score {
    /// `chart`'s bars, in order, and its key's signature.
    #[must_use]
    pub fn from_chart(chart: &Chart) -> Self {
        let mut others: Vec<Vec<Bar>> = (0..4u8)
            .map(|v| {
                chart
                    .sections
                    .iter()
                    .flat_map(|s| s.measures().iter().map(move |m| Bar::from_voice(m, v)))
                    .collect()
            })
            .collect();
        let bars = std::mem::take(&mut others[0]);
        let chords = chart
            .sections
            .iter()
            .flat_map(|s| s.measures().iter().map(ChordBar::from_measure))
            .collect();
        let mut key = [0i8; 7];
        if let Some(k) = chart.initial_key.as_ref().or(chart.current_key.as_ref()) {
            for note in k.notes() {
                if let Some(i) = "CDEFGAB".find(note.letter) {
                    key[i] = note
                        .accidental
                        .as_ref()
                        .map_or(0, keyflow_proto::primitives::Accidental::semitone_offset);
                }
            }
        }
        let read_only = chart
            .sections
            .iter()
            .flat_map(|s| {
                s.measures().iter().map(|m| {
                    m.source_span.is_none().then(|| {
                        m.read_only.clone().unwrap_or_else(|| {
                            "it has no text of its own in the chart's text".to_owned()
                        })
                    })
                })
            })
            .collect();
        Self {
            bars,
            voice: 0,
            others,
            chords,
            read_only,
            key,
        }
    }

    /// `voice`'s lines made the ones entered (`bars`), the others kept.
    pub fn set_voice(&mut self, voice: u8) {
        let voice = voice.min(3);
        if voice == self.voice || self.others.len() < 4 {
            return;
        }
        std::mem::swap(&mut self.bars, &mut self.others[usize::from(self.voice)]);
        std::mem::swap(&mut self.bars, &mut self.others[usize::from(voice)]);
        self.voice = voice;
    }

    /// `voice`'s lines, 0 to 3.
    #[must_use]
    pub fn voice_bars(&self, voice: u8) -> &[Bar] {
        if voice == self.voice {
            &self.bars
        } else {
            self.others
                .get(usize::from(voice))
                .map_or(&[], Vec::as_slice)
        }
    }

    /// The key signature's alteration of `letter`.
    #[must_use]
    pub fn key_alter(&self, letter: char) -> i8 {
        "CDEFGAB".find(letter).map_or(0, |i| self.key[i])
    }

    /// The last note before `pos`, the one a new note's octave is chosen
    /// near.
    #[must_use]
    pub fn pitch_before(&self, pos: Pos) -> Option<Pitch> {
        if self.bars.is_empty() {
            return None;
        }
        for b in (0..=pos.bar.min(self.bars.len().saturating_sub(1))).rev() {
            let bar = &self.bars[b];
            let starts = bar.starts();
            for (e, s) in bar.events.iter().zip(starts).rev() {
                if (b < pos.bar || s < pos.tick)
                    && let Some(p) = e.pitch()
                {
                    return Some(p);
                }
            }
        }
        None
    }
}

/// What a letter enters: a chord symbol (these charts are chord charts
/// first) or a note of the melody. Shown on the toolbar and the status line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Target {
    /// A–G a chord symbol's root at the duration, the cursor moving on.
    #[default]
    Chords,
    /// A–G a note, Shift+A–G a note added to the chord of notes.
    Notes,
}

impl Target {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Chords => "Chord symbols",
            Self::Notes => "Notes",
        }
    }
}

/// How keys are read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Not entering: keys act on what is selected.
    #[default]
    Normal,
    /// Duration first, then a letter enters the note (MuseScore's default).
    NoteName,
    /// Pitch first (the pitch cursor), then a duration key enters it.
    Duration,
    /// Keyflow's syntax typed at the cursor.
    Syntax,
}

impl Mode {
    /// Its name, as the toolbar and the status line say it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Normal => "Normal mode",
            Self::NoteName => "Input by note name",
            Self::Duration => "Input by duration",
            Self::Syntax => "Input via syntax",
        }
    }

    /// Whether it enters notes.
    #[must_use]
    pub const fn is_input(self) -> bool {
        !matches!(self, Self::Normal)
    }
}

/// One person's note input: their mode, tools and cursor. Presence, not
/// the chart: another person editing the same chart has their own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputState {
    pub mode: Mode,
    /// The duration the next note or rest takes.
    pub duration: Duration,
    /// The accidental armed for the next note (`-` `+` `=`), if any.
    pub accidental: Option<i8>,
    /// The rest tool: the next entry is a rest.
    pub rest: bool,
    /// Where the next note goes.
    pub cursor: Pos,
    /// The pitch cursor (input by duration), and the pitch the next
    /// letter's octave is chosen near.
    pub pitch: Pitch,
    /// The note or rest last entered or moved to: what ↑ ↓ Del T act on.
    pub selected: Option<Pos>,
    /// What a letter enters.
    pub target: Target,
    /// The voice notes go into, 0 to 3 (MuseScore's voices 1 to 4).
    pub voice: u8,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            mode: Mode::Normal,
            duration: Duration::new(Value::Quarter),
            accidental: None,
            rest: false,
            cursor: Pos::default(),
            // The middle line of a treble staff.
            pitch: Pitch::new('B', 0, 4),
            selected: None,
            target: Target::Chords,
            voice: 0,
        }
    }
}

/// What an operation does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// Into a mode (N, M, the syntax key) or back to normal.
    Mode(Mode),
    /// Esc: out of note input.
    Escape,
    /// A duration key (1–7): chosen; in input by duration, entered.
    Duration(Value),
    /// `.`: the duration dotted, or not.
    Dot,
    /// `-` `=` `+` (and the double ones): armed for the next note, or
    /// disarmed when chosen again. −2 to +2; 0 is a natural.
    Accidental(i8),
    /// The rest tool, on or off.
    RestTool,
    /// A–G.
    Letter(char),
    /// `0`: a rest at the duration.
    Rest,
    /// `T`: a note of the same pitch, tied from the one before.
    Tie,
    /// ↑ / ↓: a semitone (in input by duration, the pitch cursor a step).
    Semitone(bool),
    /// Ctrl+↑ / Ctrl+↓: an octave.
    Octave(bool),
    /// Del: the selected note becomes a rest.
    Delete,
    /// → / ←: the next or previous note or rest.
    Next,
    Prev,
    /// Ctrl+→ / Ctrl+←: the next or previous bar.
    NextBar,
    PrevBar,
    /// Ctrl+Home / Ctrl+End: the first bar, or the last.
    First,
    Last,
    /// A click: the cursor to the note or rest at `pos`.
    MoveTo(Pos),
    /// Chord symbols or notes: what a letter enters.
    Target(Target),
    /// Shift+A–G: a note added above the selected note's chord.
    AddToChord(char),
    /// A click on the staff in note input: a note at staff step `step`
    /// (`Pitch::step`) entered at the cursor, the bar's accidental for it
    /// (or the one armed) applied (MuseScore's mouse entry).
    EnterStep(i32),
    /// Ctrl+Alt+1–4: the voice notes go into (0 to 3).
    Voice(u8),
    /// Ctrl+3: a triplet group made at the cursor, as long as the chosen
    /// duration, its three values entered next (MuseScore's tuplets; a
    /// triplet is the one tuplet Keyflow's text writes).
    Triplet,
    /// W / Q: the selected note or chord's length doubled or halved.
    Double,
    Halve,
    /// Shift+W / Shift+Q: one step longer or shorter, dotted values
    /// between (a quarter, a dotted quarter, a half).
    Longer,
    Shorter,
}

/// A chord symbol's root as written: `F#`, `Bb`.
fn root(p: Pitch) -> String {
    let acc = match p.alter {
        2 => "##",
        1 => "#",
        -1 => "b",
        -2 => "bb",
        _ => "",
    };
    format!("{}{acc}", p.letter)
}

/// `token` with its root made `p`'s, its marks and quality kept.
fn with_root(token: &str, p: Pitch) -> Option<String> {
    let at = token
        .char_indices()
        .find(|(_, c)| c.is_ascii_alphabetic())?
        .0;
    let (marks, rest) = token.split_at(at);
    let mut chars = rest.chars();
    let first = chars.next()?;
    if !('A'..='G').contains(&first.to_ascii_uppercase()) {
        return None;
    }
    let after: String = chars.collect();
    let quality = after.strip_prefix(['#', 'b']).unwrap_or(&after);
    Some(format!("{marks}{}{quality}", root(p)))
}

/// What an operation did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The bars it changed, to be written back.
    pub changed: Vec<usize>,
    /// What it could not do, said as the status line says it.
    pub refused: Option<String>,
}

impl Outcome {
    fn refuse(why: impl Into<String>) -> Self {
        Self {
            changed: Vec::new(),
            refused: Some(why.into()),
        }
    }

    fn bars(changed: Vec<usize>) -> Self {
        Self {
            changed,
            refused: None,
        }
    }
}

impl InputState {
    /// `op` applied: the state moved and `score`'s bars changed as it says.
    pub fn apply(&mut self, score: &mut Score, op: Op) -> Outcome {
        if score.bars.is_empty() {
            // A blank chart: its first bars are typed in syntax mode.
            return match op {
                Op::Mode(mode) => {
                    self.mode = mode;
                    self.cursor = Pos::default();
                    Outcome::default()
                }
                Op::Escape => {
                    self.mode = Mode::Normal;
                    Outcome::default()
                }
                _ => Outcome::refuse(
                    "The chart has no bars yet: K, then type some (`C / / / | G / / /`) and Enter",
                ),
            };
        }
        self.cursor.bar = self.cursor.bar.min(score.bars.len() - 1);
        score.set_voice(self.voice);
        match op {
            Op::Mode(mode) => {
                if mode == Mode::Duration
                    && let Some(p) = score.pitch_before(self.cursor)
                {
                    self.pitch = p;
                }
                self.mode = mode;
                Outcome::default()
            }
            Op::Escape => {
                self.mode = Mode::Normal;
                self.accidental = None;
                self.rest = false;
                Outcome::default()
            }
            Op::Duration(value) => {
                self.duration = Duration::new(value);
                match self.mode {
                    Mode::Duration if self.target == Target::Chords => {
                        let slot = if self.rest {
                            Slot::Rest
                        } else {
                            Slot::Chord(root(self.armed(self.pitch)))
                        };
                        self.enter_chord(score, &slot)
                    }
                    Mode::Duration => {
                        let kind = if self.rest {
                            EventKind::Rest
                        } else {
                            EventKind::Note(self.armed(self.pitch))
                        };
                        self.enter(score, kind)
                    }
                    Mode::Normal if self.target == Target::Chords => self.rechord_length(score),
                    Mode::Normal => self.redurate(score),
                    _ => Outcome::default(),
                }
            }
            Op::Dot => {
                self.duration.dots = u8::from(self.duration.dots == 0);
                if self.mode == Mode::Normal {
                    return self.redurate(score);
                }
                Outcome::default()
            }
            Op::Accidental(alter) => {
                if self.mode == Mode::Normal {
                    return self.repitch(score, |p| Pitch::new(p.letter, alter, p.octave));
                }
                self.accidental = if self.accidental == Some(alter) {
                    None
                } else {
                    Some(alter)
                };
                Outcome::default()
            }
            Op::RestTool => {
                self.rest = !self.rest;
                Outcome::default()
            }
            Op::Letter(letter) => {
                let letter = letter.to_ascii_uppercase();
                if !('A'..='G').contains(&letter) {
                    return Outcome::refuse(format!("{letter} is not a note name"));
                }
                if self.target == Target::Chords {
                    // A chord symbol's root: the letter as typed (chord
                    // symbols do not take the key signature), with an armed
                    // accidental.
                    let alter = self.accidental.unwrap_or(0);
                    let pitch = Pitch::new(letter, alter, self.pitch.octave);
                    return match self.mode {
                        Mode::NoteName => {
                            let slot = if self.rest {
                                Slot::Rest
                            } else {
                                Slot::Chord(root(pitch))
                            };
                            self.enter_chord(score, &slot)
                        }
                        Mode::Duration => {
                            self.pitch = pitch;
                            Outcome::default()
                        }
                        Mode::Normal => self.rechord(score, |t| with_root(t, pitch)),
                        Mode::Syntax => Outcome::default(),
                    };
                }
                let near = score.pitch_before(self.cursor).unwrap_or(self.pitch);
                let octave = Pitch::nearest(letter, score.key_alter(letter), near).octave;
                let alter = self
                    .accidental
                    .unwrap_or_else(|| self.carried(score, letter, octave));
                let pitch = Pitch::nearest(letter, alter, near);
                match self.mode {
                    Mode::NoteName => {
                        let kind = if self.rest {
                            EventKind::Rest
                        } else {
                            EventKind::Note(pitch)
                        };
                        self.enter(score, kind)
                    }
                    Mode::Duration => {
                        self.pitch = pitch;
                        Outcome::default()
                    }
                    Mode::Normal => self.repitch(score, |p| Pitch::nearest(letter, alter, p)),
                    Mode::Syntax => Outcome::default(),
                }
            }
            Op::Rest => {
                if self.mode == Mode::Normal {
                    return self.delete(score);
                }
                if self.target == Target::Chords {
                    return self.enter_chord(score, &Slot::Rest);
                }
                self.enter(score, EventKind::Rest)
            }
            Op::Tie if self.target == Target::Chords => {
                Outcome::refuse("A tie joins notes: switch to entering notes")
            }
            Op::Tie => self.tie(score),
            Op::Semitone(up) => {
                if self.mode == Mode::Duration {
                    self.pitch = Pitch::from_step(self.pitch.step() + if up { 1 } else { -1 });
                    return Outcome::default();
                }
                if self.target == Target::Chords {
                    return self.rechord(score, |t| transpose(t, up));
                }
                self.repitch(score, |p| p.semitone(up))
            }
            Op::Octave(up) => {
                if self.mode == Mode::Duration {
                    self.pitch = self.pitch.octave_shift(up);
                    return Outcome::default();
                }
                self.repitch(score, |p| p.octave_shift(up))
            }
            Op::Delete if self.target == Target::Chords => self.rechord_slot(score, Slot::Rest),
            Op::Delete => self.delete(score),
            Op::Next => {
                self.step_event(score, true);
                Outcome::default()
            }
            Op::Prev => {
                self.step_event(score, false);
                Outcome::default()
            }
            Op::NextBar | Op::PrevBar | Op::First | Op::Last => {
                let bar = match op {
                    Op::NextBar => (self.cursor.bar + 1).min(score.bars.len() - 1),
                    Op::PrevBar => self.cursor.bar.saturating_sub(1),
                    Op::First => 0,
                    _ => score.bars.len() - 1,
                };
                self.move_to(score, Pos { bar, tick: 0 });
                Outcome::default()
            }
            Op::MoveTo(pos) => {
                self.move_to(score, pos);
                Outcome::default()
            }
            Op::Target(target) => {
                self.target = target;
                self.move_to(score, self.cursor);
                Outcome::default()
            }
            Op::AddToChord(letter) => self.add_to_chord(score, letter),
            Op::Voice(voice) => {
                if voice > 3 {
                    return Outcome::refuse("There are four voices");
                }
                self.voice = voice;
                score.set_voice(voice);
                // Voices are notes' (chord symbols have one line).
                self.target = Target::Notes;
                self.move_to(score, self.cursor);
                Outcome::default()
            }
            Op::Triplet => self.triplet(score),
            Op::EnterStep(step) => {
                let natural = Pitch::from_step(step);
                let alter = self
                    .accidental
                    .unwrap_or_else(|| self.carried(score, natural.letter, natural.octave));
                let pitch = Pitch::new(natural.letter, alter, natural.octave);
                if self.rest {
                    return self.enter(score, EventKind::Rest);
                }
                self.enter(score, EventKind::Note(pitch))
            }
            Op::Double => self.rescale(score, |d| {
                Value::ALL
                    .get(d.value.index() as usize + 1)
                    .map(|v| Duration { value: *v, ..d })
            }),
            Op::Halve => self.rescale(score, |d| {
                (d.value.index() > 0)
                    .then(|| Value::ALL[d.value.index() as usize - 1])
                    .map(|v| Duration { value: v, ..d })
            }),
            Op::Longer => self.rescale(score, |d| {
                if d.dots == 0 && d.value != Value::Whole {
                    Some(Duration::dotted(d.value))
                } else {
                    Value::ALL
                        .get(d.value.index() as usize + 1)
                        .map(|v| Duration::new(*v))
                }
            }),
            Op::Shorter => self.rescale(score, |d| {
                if d.dots > 0 {
                    Some(Duration::new(d.value))
                } else {
                    (d.value.index() > 0)
                        .then(|| Duration::dotted(Value::ALL[d.value.index() as usize - 1]))
                }
            }),
        }
    }

    /// The selected note or chord's length made `to` of it, what follows
    /// overwritten (or rest after it, made shorter).
    fn rescale(&mut self, score: &mut Score, to: impl Fn(Duration) -> Option<Duration>) -> Outcome {
        let current = match self.target {
            Target::Notes => self
                .selected_event(score)
                .map(|(b, i)| score.bars[b].events[i].duration),
            Target::Chords => self
                .selected_chord(score)
                .map(|(b, i)| score.chords[b].events[i].duration),
        };
        let Some(current) = current else {
            return Outcome::refuse("Nothing selected to change");
        };
        let Some(new) = to(current) else {
            return Outcome::refuse("It is as long, or as short, as it can be");
        };
        let saved = self.duration;
        self.duration = new;
        let out = match self.target {
            Target::Notes => self.redurate(score),
            Target::Chords => self.rechord_length(score),
        };
        self.duration = saved;
        out
    }

    /// The accidental a note of `letter` in `octave` has at the cursor: the
    /// last one written on that note earlier in the bar (an accidental holds
    /// to the barline), else the key signature's.
    fn carried(&self, score: &Score, letter: char, octave: i8) -> i8 {
        let bar = &score.bars[self.cursor.bar];
        bar.starts()
            .into_iter()
            .zip(&bar.events)
            .filter(|(s, _)| *s < self.cursor.tick)
            .flat_map(|(_, e)| {
                e.pitch()
                    .into_iter()
                    .chain(e.above.iter().flatten().copied())
            })
            .filter(|p| p.letter == letter && p.octave == octave)
            .next_back()
            .map_or_else(|| score.key_alter(letter), |p| p.alter)
    }

    /// The bar `bar`'s events in the target's line: `(start, length)`.
    fn line(&self, score: &Score, bar: usize) -> Vec<(u32, u32)> {
        match self.target {
            Target::Notes => {
                let b = &score.bars[bar];
                b.starts()
                    .into_iter()
                    .zip(&b.events)
                    .map(|(s, e)| (s, e.duration.ticks()))
                    .collect()
            }
            Target::Chords => {
                let b = &score.chords[bar];
                b.starts()
                    .into_iter()
                    .zip(&b.events)
                    .map(|(s, e)| (s, e.duration.ticks()))
                    .collect()
            }
        }
    }

    /// A chord symbol (or a rest) entered at the cursor at the duration,
    /// overwriting, and the cursor moved past it. It stops at the barline:
    /// a chord sounds until the next one, so the next bar is its own.
    fn enter_chord(&mut self, score: &mut Score, slot: &Slot) -> Outcome {
        let at = self.cursor;
        if let Some(why) = score.read_only.get(at.bar).cloned().flatten() {
            return Outcome::refuse(format!("Bar {} is read-only: {why}", at.bar + 1));
        }
        let Some(bar) = score.chords.get_mut(at.bar) else {
            return Outcome::refuse("The cursor is past the last bar");
        };
        if let Some(why) = &bar.locked {
            return Outcome::refuse(format!(
                "Bar {}'s chords cannot be edited here: {why}",
                at.bar + 1
            ));
        }
        if at.tick >= bar.length {
            return Outcome::refuse("The cursor is at the end of the chart");
        }
        let len = self.duration.ticks().min(bar.length - at.tick);
        bar.overwrite(at.tick, len, slot);
        let mut end = Pos {
            bar: at.bar,
            tick: at.tick + len,
        };
        if end.tick >= bar.length && end.bar + 1 < score.chords.len() {
            end = Pos {
                bar: end.bar + 1,
                tick: 0,
            };
        }
        self.selected = Some(at);
        self.cursor = end;
        self.accidental = None;
        Outcome::bars(vec![at.bar])
    }

    /// The selected chord symbol's text changed by `to` (`None`: it cannot
    /// be, such as a degree's root moved a semitone).
    fn rechord(&mut self, score: &mut Score, to: impl Fn(&str) -> Option<String>) -> Outcome {
        let Some((bar, i)) = self.selected_chord(score) else {
            return Outcome::refuse("Nothing selected to change");
        };
        let event = &mut score.chords[bar].events[i];
        let Slot::Chord(t) = &event.slot else {
            return Outcome::refuse("No chord there");
        };
        let Some(new) = to(t) else {
            return Outcome::refuse("That chord's root is not a letter");
        };
        event.slot = Slot::Chord(new);
        Outcome::bars(vec![bar])
    }

    /// The selected chord slot made `slot`.
    fn rechord_slot(&mut self, score: &mut Score, slot: Slot) -> Outcome {
        let Some((bar, i)) = self.selected_chord(score) else {
            return Outcome::refuse("Nothing selected to delete");
        };
        score.chords[bar].events[i].slot = slot;
        Outcome::bars(vec![bar])
    }

    /// The selected chord given the duration chosen (normal mode).
    fn rechord_length(&mut self, score: &mut Score) -> Outcome {
        let Some((bar, i)) = self.selected_chord(score) else {
            return Outcome::default();
        };
        let slot = score.chords[bar].events[i].slot.clone();
        let at = score.chords[bar].starts()[i];
        let saved = self.cursor;
        self.cursor = Pos { bar, tick: at };
        let out = self.enter_chord(score, &slot);
        self.cursor = saved;
        self.selected = Some(Pos { bar, tick: at });
        out
    }

    /// The selected chord slot: its bar and index.
    fn selected_chord(&self, score: &Score) -> Option<(usize, usize)> {
        let pos = self.selected?;
        let i = score.chords.get(pos.bar)?.index_at(pos.tick)?;
        Some((pos.bar, i))
    }

    /// Shift+letter: a note added above the selected note (MuseScore adds
    /// it above what is there, so a chord is built from the bottom up).
    fn add_to_chord(&mut self, score: &mut Score, letter: char) -> Outcome {
        if self.target == Target::Chords {
            return Outcome::refuse(
                "Shift+A–G adds a note to a chord of notes: switch to entering notes",
            );
        }
        let letter = letter.to_ascii_uppercase();
        let Some((bar, i)) = self.selected_event(score) else {
            return Outcome::refuse("Enter a note to add to first");
        };
        let alter = self.accidental.unwrap_or_else(|| score.key_alter(letter));
        let event = &mut score.bars[bar].events[i];
        let Some(base) = event.pitch() else {
            return Outcome::refuse("A rest takes no notes");
        };
        let top = event
            .above
            .iter()
            .flatten()
            .copied()
            .chain(std::iter::once(base))
            .max_by_key(|p| p.midi())
            .unwrap_or(base);
        let mut new = Pitch::nearest(letter, alter, top);
        while new.midi() <= top.midi() {
            new = new.octave_shift(true);
        }
        let Some(slot) = event.above.iter_mut().find(|p| p.is_none()) else {
            return Outcome::refuse("A chord of notes holds four notes at most here");
        };
        *slot = Some(new);
        self.accidental = None;
        Outcome::bars(vec![bar])
    }

    /// `pitch` with the armed accidental, which an entry uses up.
    fn armed(&self, pitch: Pitch) -> Pitch {
        self.accidental
            .map_or(pitch, |a| Pitch::new(pitch.letter, a, pitch.octave))
    }

    /// The cursor to the note or rest at `pos`, which is selected.
    fn move_to(&mut self, score: &Score, pos: Pos) {
        let bar = pos.bar.min(score.bars.len() - 1);
        let line = self.line(score, bar);
        let tick = line
            .iter()
            .find(|(s, l)| pos.tick >= *s && pos.tick < s + l)
            .map_or(0, |(s, _)| *s);
        self.cursor = Pos { bar, tick };
        self.selected = Some(self.cursor);
        let b = &score.bars[bar];
        if let Some(p) = b
            .index_at(tick)
            .and_then(|i| b.events.get(i))
            .and_then(Event::pitch)
        {
            self.pitch = p;
        }
    }

    /// The cursor one note or rest on (or back), over barlines.
    fn step_event(&mut self, score: &Score, forward: bool) {
        let line = self.line(score, self.cursor.bar);
        let starts: Vec<u32> = line.iter().map(|x| x.0).collect();
        let i = line
            .iter()
            .position(|(s, l)| self.cursor.tick >= *s && self.cursor.tick < s + l)
            .unwrap_or(starts.len());
        let to = if forward {
            match starts.get(i + 1) {
                Some(&tick) => Pos {
                    bar: self.cursor.bar,
                    tick,
                },
                None if self.cursor.bar + 1 < score.bars.len() => Pos {
                    bar: self.cursor.bar + 1,
                    tick: 0,
                },
                None => return,
            }
        } else if i > 0 {
            Pos {
                bar: self.cursor.bar,
                tick: starts[i - 1],
            }
        } else if self.cursor.bar > 0 {
            let prev = self.line(score, self.cursor.bar - 1);
            Pos {
                bar: self.cursor.bar - 1,
                tick: prev.last().map_or(0, |x| x.0),
            }
        } else {
            return;
        };
        self.move_to(score, to);
    }

    /// `kind` entered at the cursor at the duration, overwriting, and the
    /// cursor moved past it. Too long for the bar, it is split at the
    /// barline and tied into the next bar.
    fn enter(&mut self, score: &mut Score, kind: EventKind) -> Outcome {
        let kind = match kind {
            EventKind::Note(p) => EventKind::Note(self.armed(p)),
            EventKind::Rest => EventKind::Rest,
        };
        let at = self.cursor;
        if at.bar >= score.bars.len() {
            return Outcome::refuse("The cursor is past the last bar");
        }
        if let Some(why) = score.read_only.get(at.bar).cloned().flatten() {
            return Outcome::refuse(format!("Bar {} is read-only: {why}", at.bar + 1));
        }
        let bar = &score.bars[at.bar];
        if let Some(why) = &bar.locked {
            return Outcome::refuse(format!("Bar {} cannot be edited here: {why}", at.bar + 1));
        }
        if at.tick >= bar.length {
            return Outcome::refuse("The cursor is at the end of the chart");
        }
        let len = self.duration.ticks();
        let room = bar.length - at.tick;
        if self.duration.triplet && len > room {
            return Outcome::refuse("A triplet does not run over the barline");
        }
        let mut changed = vec![at.bar];
        let next_ok = score
            .bars
            .get(at.bar + 1)
            .is_some_and(|b| b.locked.is_none());
        let spills = len > room && next_ok;
        let tie_over = spills && matches!(kind, EventKind::Note(_));
        score.bars[at.bar].overwrite(at.tick, len.min(room), kind, tie_over);
        let mut end = Pos {
            bar: at.bar,
            tick: at.tick + len.min(room),
        };
        if spills {
            let rest = len - room;
            let next = &mut score.bars[at.bar + 1];
            let rest = rest.min(next.length);
            next.overwrite(0, rest, kind, false);
            changed.push(at.bar + 1);
            end = Pos {
                bar: at.bar + 1,
                tick: rest,
            };
        }
        // Past the end of a bar is the start of the next.
        if end.tick >= score.bars[end.bar].length && end.bar + 1 < score.bars.len() {
            end = Pos {
                bar: end.bar + 1,
                tick: 0,
            };
        }
        self.selected = Some(at);
        self.cursor = end;
        // Out of the triplet group: back to the value it was made from.
        if self.duration.triplet
            && !score.bars[end.bar]
                .index_at(end.tick)
                .is_some_and(|i| score.bars[end.bar].events[i].duration.triplet)
        {
            self.duration = Duration::new(
                Value::ALL[(self.duration.value.index() as usize + 1).min(Value::ALL.len() - 1)],
            );
        }
        if let EventKind::Note(p) = kind {
            self.pitch = p;
        }
        self.accidental = None;
        let mut outcome = Outcome::bars(changed);
        if len > room && !spills {
            outcome.refused =
                Some("Cut at the barline: there is no bar after it to tie into".into());
        }
        outcome
    }

    /// Ctrl+3: a triplet group at the cursor as long as the chosen
    /// duration (a quarter makes three triplet eighths), and the duration
    /// made its value, so the next three entries fill it.
    fn triplet(&mut self, score: &mut Score) -> Outcome {
        if !matches!(self.mode, Mode::NoteName | Mode::Duration) || self.target != Target::Notes {
            return Outcome::refuse(
                "A triplet is made in note input, on notes (N, then Y for notes)",
            );
        }
        let total = Duration::new(self.duration.value);
        let Some(unit) = (total.value.index() as usize)
            .checked_sub(1)
            .map(|i| Value::ALL[i])
        else {
            return Outcome::refuse("A triplet of 128ths is too short to write");
        };
        let at = self.cursor;
        if let Some(why) = score.read_only.get(at.bar).cloned().flatten() {
            return Outcome::refuse(format!("Bar {} is read-only: {why}", at.bar + 1));
        }
        let Some(bar) = score.bars.get_mut(at.bar) else {
            return Outcome::refuse("The cursor is past the last bar");
        };
        if let Some(why) = &bar.locked {
            return Outcome::refuse(format!("Bar {} cannot be edited here: {why}", at.bar + 1));
        }
        if at.tick + total.ticks() > bar.length {
            return Outcome::refuse("The triplet would run over the barline");
        }
        bar.triplet_group(at.tick, unit);
        self.duration = Duration::triplet(unit);
        self.selected = Some(at);
        Outcome::bars(vec![at.bar])
    }

    /// The selected note's pitch changed by `to` (all of a tied note in
    /// its bar).
    fn repitch(&mut self, score: &mut Score, to: impl Fn(Pitch) -> Pitch) -> Outcome {
        let Some((bar, i)) = self.selected_event(score) else {
            return Outcome::refuse("Nothing selected to change");
        };
        let b = &mut score.bars[bar];
        let Some(old) = b.events[i].pitch() else {
            return Outcome::refuse("A rest has no pitch");
        };
        let new = to(old);
        let mut j = i;
        loop {
            b.events[j].kind = EventKind::Note(new);
            if !b.events[j].tie || j + 1 >= b.events.len() {
                break;
            }
            j += 1;
        }
        self.pitch = new;
        Outcome::bars(vec![bar])
    }

    /// The selected note or rest given the duration chosen (normal mode's
    /// duration keys and dot), overwriting what follows.
    fn redurate(&mut self, score: &mut Score) -> Outcome {
        let Some((bar, i)) = self.selected_event(score) else {
            return Outcome::default();
        };
        let kind = score.bars[bar].events[i].kind;
        let at = score.bars[bar].starts()[i];
        let saved = (self.cursor, self.mode);
        self.cursor = Pos { bar, tick: at };
        let out = self.enter(score, kind);
        self.cursor = saved.0;
        self.mode = saved.1;
        self.selected = Some(Pos { bar, tick: at });
        out
    }

    /// The selected note made a rest of its duration.
    fn delete(&mut self, score: &mut Score) -> Outcome {
        let Some((bar, i)) = self.selected_event(score) else {
            return Outcome::refuse("Nothing selected to delete");
        };
        let event = &mut score.bars[bar].events[i];
        if event.pitch().is_none() {
            return Outcome::default();
        }
        *event = Event::rest(event.duration);
        Outcome::bars(vec![bar])
    }

    /// `T`: the selected note tied to a new one of its pitch at the cursor.
    fn tie(&mut self, score: &mut Score) -> Outcome {
        let Some((bar, i)) = self.selected_event(score) else {
            return Outcome::refuse("Enter a note to tie from first");
        };
        let Some(pitch) = score.bars[bar].events[i].pitch() else {
            return Outcome::refuse("A rest cannot be tied");
        };
        // The last value of the selected note is the one tied on.
        let b = &mut score.bars[bar];
        let mut j = i;
        while b.events[j].tie && j + 1 < b.events.len() {
            j += 1;
        }
        b.events[j].tie = true;
        let saved = self.accidental.take();
        let mut out = self.enter(score, EventKind::Note(pitch));
        self.accidental = saved;
        if !out.changed.contains(&bar) {
            out.changed.insert(0, bar);
        }
        out
    }

    /// The selected event: its bar and index.
    fn selected_event(&self, score: &Score) -> Option<(usize, usize)> {
        let pos = self.selected?;
        let i = score.bars.get(pos.bar)?.index_at(pos.tick)?;
        Some((pos.bar, i))
    }

    /// What the status line says: the mode, what the next key enters, and
    /// where.
    #[must_use]
    pub fn status(&self, score: &Score) -> String {
        let line = self.status_line(score);
        match score.read_only.get(self.cursor.bar).cloned().flatten() {
            Some(why) if self.mode.is_input() => {
                format!("Bar {} is read-only: {why} · {line}", self.cursor.bar + 1)
            }
            _ => line,
        }
    }

    fn status_line(&self, score: &Score) -> String {
        let bar = score.bars.get(self.cursor.bar);
        let beat_ticks = TICKS_PER_WHOLE / 4;
        let beat = f64::from(self.cursor.tick) / f64::from(beat_ticks) + 1.0;
        let at = format!(
            "bar {}, beat {}",
            self.cursor.bar + 1,
            if beat.fract() == 0.0 {
                format!("{beat:.0}")
            } else {
                format!("{beat:.2}").trim_end_matches('0').to_owned()
            }
        );
        let accidental = match self.accidental {
            Some(2) => " · double sharp armed",
            Some(1) => " · sharp armed",
            Some(0) => " · natural armed",
            Some(-1) => " · flat armed",
            Some(-2) => " · double flat armed",
            _ => "",
        };
        let what = if self.rest { "rest" } else { "note" };
        let locked = bar
            .and_then(|b| b.locked.as_deref())
            .map(|why| format!(" · this bar cannot be edited here: {why}"))
            .unwrap_or_default();
        match self.mode {
            Mode::Normal => match self.selected {
                Some(p) => format!(
                    "Normal mode · bar {} selected · N: input by note name · M: input by duration",
                    p.bar + 1
                ),
                None => "Normal mode · N: input by note name · M: input by duration".to_owned(),
            },
            Mode::NoteName if self.target == Target::Chords => format!(
                "Input by note name · chord symbols · {} {}{accidental} · next at {at} · A–G a chord root, 0 no chord, ↑ ↓ the root, Y: notes instead, Esc leave{}",
                self.duration.name(),
                if self.rest { "rest" } else { "chord" },
                score
                    .chords
                    .get(self.cursor.bar)
                    .and_then(|b| b.locked.as_deref())
                    .map(|why| format!(" · this bar's chords cannot be edited here: {why}"))
                    .unwrap_or_default()
            ),
            Mode::NoteName => format!(
                "Input by note name · notes · voice {} · {} {what}{accidental} · next at {at} · A–G enter, Shift+A–G add to the chord, 0 rest, T tie, Y: chord symbols instead, Esc leave{locked}",
                self.voice + 1,
                self.duration.name()
            ),
            Mode::Duration => format!(
                "Input by duration · voice {} · pitch {}{accidental} · next at {at} · 1–7 enter at that duration, A–G or ↑ ↓ move the pitch, Esc leave{locked}",
                self.voice + 1,
                self.armed(self.pitch).name()
            ),
            Mode::Syntax => format!(
                "Input via syntax · from {at} · type Keyflow bars (`C / / / | Am7 / F G`), Enter to write them, Esc to leave"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::music::Value::{Eighth, Half, Quarter, Whole};

    fn score(bars: usize) -> Score {
        let empty = || (0..bars).map(|_| Bar::empty((4, 4))).collect::<Vec<_>>();
        Score {
            bars: empty(),
            voice: 0,
            others: vec![Vec::new(), empty(), empty(), empty()],
            chords: (0..bars).map(|_| ChordBar::empty((4, 4))).collect(),
            read_only: vec![None; bars],
            key: [0; 7],
        }
    }

    fn notes(bar: &Bar) -> Vec<String> {
        bar.events
            .iter()
            .map(|e| {
                let d = format!(
                    "{}{}",
                    e.duration.value.lilypond(),
                    if e.duration.dots > 0 { "." } else { "" }
                );
                match e.kind {
                    EventKind::Note(p) => {
                        format!("{}{d}{}", p.name(), if e.tie { "~" } else { "" })
                    }
                    EventKind::Rest => format!("r{d}"),
                }
            })
            .collect()
    }

    fn input(mode: Mode) -> InputState {
        InputState {
            mode,
            target: Target::Notes,
            ..InputState::default()
        }
    }

    fn chords(bar: &ChordBar) -> String {
        bar.to_text().unwrap()
    }

    #[test]
    fn chord_symbols_are_what_a_letter_enters_first() {
        assert_eq!(InputState::default().target, Target::Chords);
        let mut s = score(2);
        let mut i = InputState {
            mode: Mode::NoteName,
            ..InputState::default()
        };
        i.apply(&mut s, Op::Duration(Half));
        i.apply(&mut s, Op::Letter('d'));
        i.apply(&mut s, Op::Accidental(-1));
        i.apply(&mut s, Op::Letter('B'));
        assert_eq!(chords(&s.chords[0]), "D_2 Bb_2");
        assert_eq!(i.cursor, Pos { bar: 1, tick: 0 }, "on to the next bar");
        // ↑ on the chord just entered moves its root, quality kept.
        i.apply(&mut s, Op::Semitone(true));
        assert_eq!(chords(&s.chords[0]), "D_2 B_2");
        // No melody was touched.
        assert!(s.bars[0].events.iter().all(|e| e.pitch().is_none()));
        // Y: notes instead.
        i.apply(&mut s, Op::Target(Target::Notes));
        i.apply(&mut s, Op::Letter('E'));
        assert!(s.bars[1].events[0].pitch().is_some());
    }

    #[test]
    fn a_click_on_the_staff_enters_that_note() {
        let mut s = score(1);
        s.key = [0, 0, 0, 1, 0, 0, 0]; // G major: F#
        let mut i = input(Mode::NoteName);
        i.apply(&mut s, Op::EnterStep(Pitch::new('F', 0, 5).step()));
        i.apply(&mut s, Op::EnterStep(Pitch::new('A', 0, 4).step()));
        assert_eq!(
            &notes(&s.bars[0])[..2],
            ["F♯54", "A44"],
            "the key's sharp applies"
        );
    }

    #[test]
    fn an_accidental_holds_to_the_barline() {
        let mut s = score(2);
        let mut i = input(Mode::NoteName);
        i.apply(&mut s, Op::Accidental(1));
        i.apply(&mut s, Op::Letter('F')); // F#
        i.apply(&mut s, Op::Letter('G'));
        i.apply(&mut s, Op::Letter('F')); // still F#: the sharp holds
        i.apply(&mut s, Op::Letter('E'));
        i.apply(&mut s, Op::Letter('F')); // next bar: natural again
        let n = notes(&s.bars[0]);
        assert_eq!(&n[..3], ["F♯44", "G44", "F♯44"]);
        assert_eq!(notes(&s.bars[1])[0], "F44");
    }

    #[test]
    fn q_and_w_halve_double_and_step_through_dotted_values() {
        let mut s = score(1);
        let mut i = input(Mode::NoteName);
        i.apply(&mut s, Op::Letter('C'));
        i.apply(&mut s, Op::Longer);
        assert_eq!(notes(&s.bars[0])[0], "C54.");
        i.apply(&mut s, Op::Longer);
        assert_eq!(notes(&s.bars[0])[0], "C52");
        i.apply(&mut s, Op::Shorter);
        assert_eq!(notes(&s.bars[0])[0], "C54.");
        i.apply(&mut s, Op::Halve);
        assert_eq!(notes(&s.bars[0])[0], "C58.");
        i.apply(&mut s, Op::Double);
        assert_eq!(notes(&s.bars[0])[0], "C54.");
        // The rest after it is regrouped by the beat.
        assert_eq!(&notes(&s.bars[0])[1..], ["r8", "r2"]);
    }

    #[test]
    fn shift_letter_adds_a_note_above_the_chord_of_notes() {
        let mut s = score(1);
        let mut i = input(Mode::NoteName);
        i.apply(&mut s, Op::Duration(Half));
        i.apply(&mut s, Op::Letter('C'));
        i.apply(&mut s, Op::AddToChord('E'));
        i.apply(&mut s, Op::AddToChord('G'));
        let e = &s.bars[0].events[0];
        let above: Vec<String> = e.above.iter().flatten().map(|p| p.name()).collect();
        assert_eq!(e.pitch().unwrap().name(), "C5");
        assert_eq!(above, ["E5", "G5"], "each above the last");
        // As chord symbols, Shift+letter says what it is for.
        i.target = Target::Chords;
        assert!(i.apply(&mut s, Op::AddToChord('B')).refused.is_some());
    }

    #[test]
    fn by_note_name_a_letter_enters_at_the_duration_and_advances() {
        let mut s = score(2);
        let mut i = input(Mode::NoteName);
        i.apply(&mut s, Op::Duration(Quarter));
        let out = i.apply(&mut s, Op::Letter('c'));
        assert_eq!(out.changed, vec![0]);
        i.apply(&mut s, Op::Letter('D'));
        i.apply(&mut s, Op::Duration(Half));
        i.apply(&mut s, Op::Letter('E'));
        assert_eq!(notes(&s.bars[0]), ["C54", "D54", "E52"]);
        assert_eq!(
            i.cursor,
            Pos { bar: 1, tick: 0 },
            "the cursor moved to the next bar"
        );
    }

    #[test]
    fn the_octave_is_the_one_nearest_the_note_before() {
        let mut s = score(1);
        let mut i = input(Mode::NoteName);
        i.apply(&mut s, Op::Duration(Eighth));
        for l in ['G', 'C', 'B', 'D'] {
            i.apply(&mut s, Op::Letter(l));
        }
        let names: Vec<String> = notes(&s.bars[0]).into_iter().take(4).collect();
        // From B4 (the default): G4, then C5 (nearer G4 than C4), B4, D5.
        assert_eq!(names, ["G48", "C58", "B48", "D58"]);
    }

    #[test]
    fn the_key_signature_and_an_armed_accidental_spell_the_letter() {
        let mut s = score(1);
        s.key = [1, 0, 0, 1, 0, 0, 0]; // D major: C# and F#
        let mut i = input(Mode::NoteName);
        i.apply(&mut s, Op::Letter('F'));
        i.apply(&mut s, Op::Accidental(0));
        i.apply(&mut s, Op::Letter('F'));
        i.apply(&mut s, Op::Accidental(-1));
        i.apply(&mut s, Op::Letter('B'));
        let n = notes(&s.bars[0]);
        assert_eq!(&n[..3], ["F♯44", "F44", "B♭44"]);
        assert_eq!(
            i.accidental, None,
            "an accidental is used up by the note it armed"
        );
    }

    #[test]
    fn rest_dot_and_a_note_too_long_for_the_bar() {
        let mut s = score(2);
        let mut i = input(Mode::NoteName);
        i.apply(&mut s, Op::Duration(Half));
        i.apply(&mut s, Op::Rest);
        i.apply(&mut s, Op::Duration(Quarter));
        i.apply(&mut s, Op::Rest);
        i.apply(&mut s, Op::Dot);
        assert_eq!(i.duration, Duration::dotted(Quarter));
        let out = i.apply(&mut s, Op::Letter('C'));
        // A dotted quarter on beat 4 of 4: a quarter, tied over to an eighth.
        assert_eq!(out.changed, vec![0, 1]);
        assert_eq!(notes(&s.bars[0]), ["r2", "r4", "C54~"]);
        assert_eq!(notes(&s.bars[1])[0], "C58");
        assert_eq!(
            i.cursor,
            Pos {
                bar: 1,
                tick: Eighth.ticks()
            }
        );
    }

    #[test]
    fn tie_enters_the_same_pitch_tied_from_the_note_before() {
        let mut s = score(1);
        let mut i = input(Mode::NoteName);
        i.apply(&mut s, Op::Letter('G'));
        i.apply(&mut s, Op::Duration(Eighth));
        let out = i.apply(&mut s, Op::Tie);
        assert_eq!(out.changed, vec![0]);
        assert_eq!(&notes(&s.bars[0])[..2], ["G44~", "G48"]);
    }

    #[test]
    fn up_down_octave_and_delete_act_on_the_note_just_entered() {
        let mut s = score(1);
        let mut i = input(Mode::NoteName);
        i.apply(&mut s, Op::Letter('E'));
        i.apply(&mut s, Op::Semitone(false));
        // E nearest B4 is E5.
        assert_eq!(notes(&s.bars[0])[0], "E♭54");
        i.apply(&mut s, Op::Semitone(true));
        i.apply(&mut s, Op::Semitone(true));
        assert_eq!(notes(&s.bars[0])[0], "F54");
        i.apply(&mut s, Op::Octave(true));
        assert_eq!(notes(&s.bars[0])[0], "F64");
        i.apply(&mut s, Op::Delete);
        assert_eq!(notes(&s.bars[0])[0], "r4");
    }

    #[test]
    fn by_duration_the_pitch_comes_first_and_the_duration_key_enters_it() {
        let mut s = score(1);
        let mut i = InputState {
            target: Target::Notes,
            ..InputState::default()
        };
        i.apply(&mut s, Op::Mode(Mode::Duration));
        i.apply(&mut s, Op::Letter('A'));
        assert!(
            s.bars[0].events.iter().all(|e| e.pitch().is_none()),
            "a letter enters nothing"
        );
        i.apply(&mut s, Op::Semitone(true)); // a step up: B
        let out = i.apply(&mut s, Op::Duration(Half));
        assert_eq!(out.changed, vec![0]);
        i.apply(&mut s, Op::Octave(false));
        i.apply(&mut s, Op::Duration(Half));
        assert_eq!(notes(&s.bars[0]), ["B42", "B32"]);
    }

    #[test]
    fn escape_leaves_note_input_and_disarms_the_tools() {
        let mut s = score(1);
        let mut i = input(Mode::NoteName);
        i.apply(&mut s, Op::Accidental(1));
        i.apply(&mut s, Op::RestTool);
        i.apply(&mut s, Op::Escape);
        assert_eq!((i.mode, i.accidental, i.rest), (Mode::Normal, None, false));
    }

    #[test]
    fn a_blank_chart_goes_into_syntax_mode_and_takes_nothing_else() {
        let mut s = score(0);
        assert_eq!(s.pitch_before(Pos { bar: 5, tick: 0 }), None, "no panic");
        let mut i = InputState::default();
        assert!(i.apply(&mut s, Op::Mode(Mode::Syntax)).refused.is_none());
        assert_eq!(i.mode, Mode::Syntax);
        assert!(i.apply(&mut s, Op::Letter('C')).refused.is_some());
    }

    #[test]
    fn a_read_only_bar_says_why_and_takes_nothing() {
        let mut s = score(2);
        s.read_only[1] = Some("it is a repeat of its line made by `x2`".into());
        let before = s.clone();
        let mut i = input(Mode::NoteName);
        i.apply(&mut s, Op::MoveTo(Pos { bar: 1, tick: 0 }));
        assert!(
            i.status(&s)
                .starts_with("Bar 2 is read-only: it is a repeat")
        );
        let out = i.apply(&mut s, Op::Letter('C'));
        assert!(out.refused.unwrap().contains("read-only"));
        i.target = Target::Chords;
        assert!(i.apply(&mut s, Op::Letter('C')).refused.is_some());
        assert_eq!(s, before);
    }

    #[test]
    fn a_locked_bar_is_refused_not_rewritten() {
        let mut s = score(1);
        s.bars[0].locked = Some("it holds a tuplet".into());
        let before = s.clone();
        let mut i = input(Mode::NoteName);
        let out = i.apply(&mut s, Op::Letter('C'));
        assert!(out.changed.is_empty());
        assert!(out.refused.unwrap().contains("tuplet"));
        assert_eq!(s, before);
    }

    #[test]
    fn arrows_walk_the_notes_over_barlines() {
        let mut s = score(2);
        let mut i = input(Mode::NoteName);
        i.apply(&mut s, Op::Duration(Whole));
        i.apply(&mut s, Op::Letter('C'));
        i.apply(&mut s, Op::Duration(Half));
        i.apply(&mut s, Op::Letter('D'));
        i.apply(&mut s, Op::Prev);
        assert_eq!(i.cursor, Pos { bar: 1, tick: 0 });
        i.apply(&mut s, Op::Prev);
        assert_eq!(i.cursor, Pos { bar: 0, tick: 0 });
        i.apply(&mut s, Op::NextBar);
        assert_eq!(i.cursor, Pos { bar: 1, tick: 0 });
        assert!(i.status(&s).contains("bar 2, beat 1"));
    }
}
