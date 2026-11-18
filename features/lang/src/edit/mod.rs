//! Editing a chart on the chart: note input as MuseScore does it, as
//! operations on the chart's model.
//!
//! - [`music`]: the values an edit is made of: a [`Duration`] (a note value
//!   and its dots, in [`TICKS_PER_WHOLE`] ticks), a [`Pitch`] (letter,
//!   alteration, octave), and a bar's notes ([`Bar`]: events one after
//!   another, filling the bar).
//! - [`input`]: the note-input state ([`InputState`]: the mode, the
//!   duration chosen, the accidental armed, the cursor) and the operations
//!   ([`Op`]) that enter, re-pitch, tie, dot, rest and delete, applied to
//!   the bars ([`Score`]) as MuseScore's note input applies them
//!   (`handbook.musescore.org/basics/entering-notes-and-rests`,
//!   `…/input-by-duration-mode`, `…/editing-notes-and-rests`).
//! - [`commit`]: the bars an operation changed written back into the
//!   chart's text, each bar's own text and nothing else (a bar knows its
//!   text by its `source_span`), and Keyflow's own syntax typed at the
//!   cursor ([`commit::splice_syntax`]): the third input mode, which parses
//!   into the same chart the other two edit.
//!
//! The state is the person's (their cursor, their tools: presence); the
//! bars are the chart's. Nothing here keeps a copy of the chart between
//! operations: a [`Score`] is read from the chart, changed, written back,
//! and the chart is read again from what was written.

pub mod chords;
pub mod commit;
pub mod input;
pub mod music;

pub use chords::{ChordBar, ChordEvent, Slot};
pub use input::{InputState, Mode, Op, Outcome, Pos, Score, Target, bars, replays, sections};
pub use music::{Bar, Duration, Event, EventKind, Pitch, TICKS_PER_WHOLE, Value};
