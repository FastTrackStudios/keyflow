//! The note-input toolbar's tools and the input state they set, as
//! MuseScore's: a duration (with a dot, or as a rest), an accidental,
//! articulations, a tie, a push. Each tool is an action of its own
//! (`KEYFLOW_*`, `extension.toml`), a toggle whose button is lit while it
//! is on, so it is on ⌘K, keys and MCP as well as the toolbar.
//!
//! The state is what the next entry or the selection takes. Pure: the
//! actions read and write it through [`InputState::apply`].

use std::collections::BTreeSet;

/// A note value, from the 64th to the whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Duration {
    SixtyFourth,
    ThirtySecond,
    Sixteenth,
    Eighth,
    Quarter,
    Half,
    Whole,
}

/// An accidental the next note takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Accidental {
    DoubleFlat,
    Flat,
    Natural,
    Sharp,
    DoubleSharp,
}

/// An articulation the next note or chord takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Articulation {
    Accent,
    Marcato,
    Tenuto,
    Staccato,
}

/// What a tool does to the input state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Note input on or off.
    NoteInput,
    /// The duration: chosen, it stays chosen (one at a time).
    Duration(Duration),
    Dot,
    Rest,
    /// The accidental: one at a time, chosen again it goes.
    Accidental(Accidental),
    Tie,
    Articulation(Articulation),
    /// The chord is pushed (anticipated): `'` in the chart language.
    Push,
    /// The palette sidebar shown or hidden.
    Palettes,
}

/// The input state the toolbar shows and sets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputState {
    pub note_input: bool,
    pub duration: Duration,
    pub dot: bool,
    pub rest: bool,
    pub accidental: Option<Accidental>,
    pub tie: bool,
    pub articulations: BTreeSet<Articulation>,
    pub push: bool,
    pub palettes: bool,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            note_input: false,
            duration: Duration::Quarter,
            dot: false,
            rest: false,
            accidental: None,
            tie: false,
            articulations: BTreeSet::new(),
            push: false,
            palettes: true,
        }
    }
}

impl InputState {
    /// `command` applied.
    pub fn apply(&mut self, command: Command) {
        match command {
            Command::NoteInput => self.note_input = !self.note_input,
            Command::Duration(d) => self.duration = d,
            Command::Dot => self.dot = !self.dot,
            Command::Rest => self.rest = !self.rest,
            Command::Accidental(a) => {
                self.accidental = (self.accidental != Some(a)).then_some(a);
            }
            Command::Tie => self.tie = !self.tie,
            Command::Articulation(a) => {
                if !self.articulations.remove(&a) {
                    self.articulations.insert(a);
                }
            }
            Command::Push => self.push = !self.push,
            Command::Palettes => self.palettes = !self.palettes,
        }
    }

    /// Whether `command`'s button is lit.
    #[must_use]
    pub fn is_on(&self, command: Command) -> bool {
        match command {
            Command::NoteInput => self.note_input,
            Command::Duration(d) => self.duration == d,
            Command::Dot => self.dot,
            Command::Rest => self.rest,
            Command::Accidental(a) => self.accidental == Some(a),
            Command::Tie => self.tie,
            Command::Articulation(a) => self.articulations.contains(&a),
            Command::Push => self.push,
            Command::Palettes => self.palettes,
        }
    }
}

/// What a tool's button shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    /// A SMuFL glyph of the chart's music font, drawn as an SVG icon
    /// (`crate::icons`).
    Glyph(char),
    /// A Lucide icon of the app's.
    Icon(&'static str),
}

/// One tool: its action, its name, its face, what it does.
#[derive(Clone, Copy, Debug)]
pub struct Tool {
    pub action: &'static str,
    pub name: &'static str,
    pub face: Face,
    pub command: Command,
}

const fn tool(action: &'static str, name: &'static str, face: Face, command: Command) -> Tool {
    Tool {
        action,
        name,
        face,
        command,
    }
}

use Face::{Glyph, Icon};

/// The toolbar's tools, in its order.
pub const TOOLS: &[Tool] = &[
    tool("KEYFLOW_NOTE_INPUT", "Note input", Icon("pencil"), Command::NoteInput),
    tool("KEYFLOW_DURATION_64TH", "64th note", Glyph('\u{E1DD}'), Command::Duration(Duration::SixtyFourth)),
    tool("KEYFLOW_DURATION_32ND", "32nd note", Glyph('\u{E1DB}'), Command::Duration(Duration::ThirtySecond)),
    tool("KEYFLOW_DURATION_16TH", "16th note", Glyph('\u{E1D9}'), Command::Duration(Duration::Sixteenth)),
    tool("KEYFLOW_DURATION_EIGHTH", "Eighth note", Glyph('\u{E1D7}'), Command::Duration(Duration::Eighth)),
    tool("KEYFLOW_DURATION_QUARTER", "Quarter note", Glyph('\u{E1D5}'), Command::Duration(Duration::Quarter)),
    tool("KEYFLOW_DURATION_HALF", "Half note", Glyph('\u{E1D3}'), Command::Duration(Duration::Half)),
    tool("KEYFLOW_DURATION_WHOLE", "Whole note", Glyph('\u{E1D2}'), Command::Duration(Duration::Whole)),
    tool("KEYFLOW_DOT", "Augmentation dot", Glyph('\u{E1E7}'), Command::Dot),
    tool("KEYFLOW_REST", "Rest", Glyph('\u{E4E5}'), Command::Rest),
    tool("KEYFLOW_DOUBLE_FLAT", "Double flat", Glyph('\u{E264}'), Command::Accidental(Accidental::DoubleFlat)),
    tool("KEYFLOW_FLAT", "Flat", Glyph('\u{E260}'), Command::Accidental(Accidental::Flat)),
    tool("KEYFLOW_NATURAL", "Natural", Glyph('\u{E261}'), Command::Accidental(Accidental::Natural)),
    tool("KEYFLOW_SHARP", "Sharp", Glyph('\u{E262}'), Command::Accidental(Accidental::Sharp)),
    tool("KEYFLOW_DOUBLE_SHARP", "Double sharp", Glyph('\u{E263}'), Command::Accidental(Accidental::DoubleSharp)),
    tool("KEYFLOW_TIE", "Tie", Icon("spline"), Command::Tie),
    tool("KEYFLOW_ACCENT", "Accent", Glyph('\u{E4A0}'), Command::Articulation(Articulation::Accent)),
    tool("KEYFLOW_MARCATO", "Marcato", Glyph('\u{E4AC}'), Command::Articulation(Articulation::Marcato)),
    tool("KEYFLOW_TENUTO", "Tenuto", Glyph('\u{E4A4}'), Command::Articulation(Articulation::Tenuto)),
    tool("KEYFLOW_STACCATO", "Staccato", Glyph('\u{E4A2}'), Command::Articulation(Articulation::Staccato)),
    tool("KEYFLOW_PUSH", "Push", Icon("chevrons-left"), Command::Push),
    tool("KEYFLOW_PALETTES", "Palettes", Icon("panel-left"), Command::Palettes),
];

/// The tool whose action is `action`.
#[must_use]
pub fn by_action(action: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|t| t.action == action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_duration_is_one_at_a_time_and_stays_chosen() {
        let mut s = InputState::default();
        s.apply(Command::Duration(Duration::Eighth));
        s.apply(Command::Duration(Duration::Eighth));
        assert!(s.is_on(Command::Duration(Duration::Eighth)));
        assert!(!s.is_on(Command::Duration(Duration::Quarter)));
    }

    #[test]
    fn an_accidental_is_one_at_a_time_and_chosen_again_it_goes() {
        let mut s = InputState::default();
        s.apply(Command::Accidental(Accidental::Flat));
        s.apply(Command::Accidental(Accidental::Sharp));
        assert_eq!(s.accidental, Some(Accidental::Sharp));
        s.apply(Command::Accidental(Accidental::Sharp));
        assert_eq!(s.accidental, None);
    }

    #[test]
    fn articulations_are_independent_toggles() {
        let mut s = InputState::default();
        s.apply(Command::Articulation(Articulation::Accent));
        s.apply(Command::Articulation(Articulation::Staccato));
        assert!(s.is_on(Command::Articulation(Articulation::Accent)));
        assert!(s.is_on(Command::Articulation(Articulation::Staccato)));
        s.apply(Command::Articulation(Articulation::Accent));
        assert!(!s.is_on(Command::Articulation(Articulation::Accent)));
    }

    #[test]
    fn every_tool_has_its_own_action() {
        let ids: BTreeSet<_> = TOOLS.iter().map(|t| t.action).collect();
        assert_eq!(ids.len(), TOOLS.len());
        assert!(TOOLS.iter().all(|t| t.action.starts_with("KEYFLOW_")));
    }
}
