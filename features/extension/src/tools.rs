//! The note-input toolbar's tools, as MuseScore's: the input modes, a
//! duration (with a dot, or as a rest), an accidental, a tie,
//! articulations, a push. Each tool is an action of its own (`KEYFLOW_*`,
//! `extension.toml`), a toggle whose button is lit while it is on, so it
//! is on ⌘K, keys and MCP as well as the toolbar.
//!
//! The modes, durations, dot, rest, accidentals and tie are note input's
//! (`keyflow::edit`, applied by `crate::run_op` on the chart); the
//! articulations, the push and the palettes are this toolbar's own state.

use std::collections::BTreeSet;

use keyflow::edit::{self, Mode, Op};

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

impl Accidental {
    /// Its alteration in semitones.
    #[must_use]
    pub const fn alter(self) -> i8 {
        match self {
            Self::DoubleFlat => -2,
            Self::Flat => -1,
            Self::Natural => 0,
            Self::Sharp => 1,
            Self::DoubleSharp => 2,
        }
    }
}

/// An articulation the next note or chord takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Articulation {
    Accent,
    Marcato,
    Tenuto,
    Staccato,
}

/// What a tool does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Into an input mode, or out of it when it is the one on.
    Mode(Mode),
    /// The duration: chosen, it stays chosen (one at a time).
    Duration(Duration),
    Dot,
    Rest,
    /// The accidental: one at a time, chosen again it goes.
    Accidental(Accidental),
    /// A tied note of the same pitch (MuseScore's T).
    Tie,
    Articulation(Articulation),
    /// The chord is pushed (anticipated): `'` in the chart language.
    Push,
    /// The palette sidebar shown or hidden.
    Palettes,
}

impl Command {
    /// The note-input operation it is, given the mode `now`: a mode's tool
    /// leaves note input when that mode is on.
    #[must_use]
    pub fn op(self, now: Mode) -> Option<Op> {
        Some(match self {
            Self::Mode(m) if m == now => Op::Escape,
            Self::Mode(m) => Op::Mode(m),
            Self::Duration(d) => Op::Duration(crate::input::value_of(d)),
            Self::Dot => Op::Dot,
            Self::Rest => Op::RestTool,
            Self::Accidental(a) => Op::Accidental(a.alter()),
            Self::Tie => Op::Tie,
            Self::Articulation(_) | Self::Push | Self::Palettes => return None,
        })
    }
}

/// The toolbar's own state: what is not note input's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputState {
    pub articulations: BTreeSet<Articulation>,
    pub push: bool,
    pub palettes: bool,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            articulations: BTreeSet::new(),
            push: false,
            palettes: true,
        }
    }
}

impl InputState {
    /// `command` applied, when it is this toolbar's own.
    pub fn apply(&mut self, command: Command) {
        match command {
            Command::Articulation(a) => {
                if !self.articulations.remove(&a) {
                    self.articulations.insert(a);
                }
            }
            Command::Push => self.push = !self.push,
            Command::Palettes => self.palettes = !self.palettes,
            _ => {}
        }
    }

    /// Whether `command`'s button is lit, with note input in `edit`.
    #[must_use]
    pub fn is_on(&self, edit: &edit::InputState, command: Command) -> bool {
        match command {
            Command::Mode(m) => edit.mode == m,
            Command::Duration(d) => edit.duration.value == crate::input::value_of(d),
            Command::Dot => edit.duration.dots > 0,
            Command::Rest => edit.rest,
            Command::Accidental(a) => edit.accidental == Some(a.alter()),
            Command::Tie => false,
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
    tool(
        "KEYFLOW_NOTE_INPUT",
        "Input by note name",
        Icon("pencil"),
        Command::Mode(Mode::NoteName),
    ),
    tool(
        "KEYFLOW_INPUT_BY_DURATION",
        "Input by duration",
        Icon("ruler"),
        Command::Mode(Mode::Duration),
    ),
    tool(
        "KEYFLOW_INPUT_SYNTAX",
        "Input via syntax",
        Icon("code"),
        Command::Mode(Mode::Syntax),
    ),
    tool(
        "KEYFLOW_DURATION_64TH",
        "64th note",
        Glyph('\u{E1DD}'),
        Command::Duration(Duration::SixtyFourth),
    ),
    tool(
        "KEYFLOW_DURATION_32ND",
        "32nd note",
        Glyph('\u{E1DB}'),
        Command::Duration(Duration::ThirtySecond),
    ),
    tool(
        "KEYFLOW_DURATION_16TH",
        "16th note",
        Glyph('\u{E1D9}'),
        Command::Duration(Duration::Sixteenth),
    ),
    tool(
        "KEYFLOW_DURATION_EIGHTH",
        "Eighth note",
        Glyph('\u{E1D7}'),
        Command::Duration(Duration::Eighth),
    ),
    tool(
        "KEYFLOW_DURATION_QUARTER",
        "Quarter note",
        Glyph('\u{E1D5}'),
        Command::Duration(Duration::Quarter),
    ),
    tool(
        "KEYFLOW_DURATION_HALF",
        "Half note",
        Glyph('\u{E1D3}'),
        Command::Duration(Duration::Half),
    ),
    tool(
        "KEYFLOW_DURATION_WHOLE",
        "Whole note",
        Glyph('\u{E1D2}'),
        Command::Duration(Duration::Whole),
    ),
    tool(
        "KEYFLOW_DOT",
        "Augmentation dot",
        Glyph('\u{E1E7}'),
        Command::Dot,
    ),
    tool("KEYFLOW_REST", "Rest", Glyph('\u{E4E5}'), Command::Rest),
    tool(
        "KEYFLOW_DOUBLE_FLAT",
        "Double flat",
        Glyph('\u{E264}'),
        Command::Accidental(Accidental::DoubleFlat),
    ),
    tool(
        "KEYFLOW_FLAT",
        "Flat",
        Glyph('\u{E260}'),
        Command::Accidental(Accidental::Flat),
    ),
    tool(
        "KEYFLOW_NATURAL",
        "Natural",
        Glyph('\u{E261}'),
        Command::Accidental(Accidental::Natural),
    ),
    tool(
        "KEYFLOW_SHARP",
        "Sharp",
        Glyph('\u{E262}'),
        Command::Accidental(Accidental::Sharp),
    ),
    tool(
        "KEYFLOW_DOUBLE_SHARP",
        "Double sharp",
        Glyph('\u{E263}'),
        Command::Accidental(Accidental::DoubleSharp),
    ),
    tool("KEYFLOW_TIE", "Tie", Icon("spline"), Command::Tie),
    tool(
        "KEYFLOW_ACCENT",
        "Accent",
        Glyph('\u{E4A0}'),
        Command::Articulation(Articulation::Accent),
    ),
    tool(
        "KEYFLOW_MARCATO",
        "Marcato",
        Glyph('\u{E4AC}'),
        Command::Articulation(Articulation::Marcato),
    ),
    tool(
        "KEYFLOW_TENUTO",
        "Tenuto",
        Glyph('\u{E4A4}'),
        Command::Articulation(Articulation::Tenuto),
    ),
    tool(
        "KEYFLOW_STACCATO",
        "Staccato",
        Glyph('\u{E4A2}'),
        Command::Articulation(Articulation::Staccato),
    ),
    tool("KEYFLOW_PUSH", "Push", Icon("chevrons-left"), Command::Push),
    tool(
        "KEYFLOW_PALETTES",
        "Palettes",
        Icon("panel-left"),
        Command::Palettes,
    ),
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
    fn note_input_s_tools_are_its_operations() {
        let now = Mode::Normal;
        assert_eq!(
            Command::Mode(Mode::NoteName).op(now),
            Some(Op::Mode(Mode::NoteName))
        );
        assert_eq!(
            Command::Mode(Mode::NoteName).op(Mode::NoteName),
            Some(Op::Escape),
            "the mode that is on, chosen again, leaves"
        );
        assert_eq!(
            Command::Duration(Duration::Eighth).op(now),
            Some(Op::Duration(edit::Value::Eighth))
        );
        assert_eq!(
            Command::Accidental(Accidental::Flat).op(now),
            Some(Op::Accidental(-1))
        );
        assert_eq!(Command::Palettes.op(now), None);
    }

    #[test]
    fn a_button_is_lit_as_note_input_is() {
        let tools = InputState::default();
        let mut e = edit::InputState::default();
        e.duration = edit::Duration::new(edit::Value::Eighth);
        e.accidental = Some(1);
        assert!(tools.is_on(&e, Command::Duration(Duration::Eighth)));
        assert!(!tools.is_on(&e, Command::Duration(Duration::Quarter)));
        assert!(tools.is_on(&e, Command::Accidental(Accidental::Sharp)));
        assert!(!tools.is_on(&e, Command::Mode(Mode::NoteName)));
    }

    #[test]
    fn articulations_are_independent_toggles() {
        let mut s = InputState::default();
        s.apply(Command::Articulation(Articulation::Accent));
        s.apply(Command::Articulation(Articulation::Staccato));
        let e = edit::InputState::default();
        assert!(s.is_on(&e, Command::Articulation(Articulation::Accent)));
        assert!(s.is_on(&e, Command::Articulation(Articulation::Staccato)));
        s.apply(Command::Articulation(Articulation::Accent));
        assert!(!s.is_on(&e, Command::Articulation(Articulation::Accent)));
    }

    #[test]
    fn every_tool_has_its_own_action() {
        let ids: BTreeSet<_> = TOOLS.iter().map(|t| t.action).collect();
        assert_eq!(ids.len(), TOOLS.len());
        assert!(TOOLS.iter().all(|t| t.action.starts_with("KEYFLOW_")));
    }
}
