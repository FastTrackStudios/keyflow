//! Note input's actions and keys: every operation of `keyflow::edit` an
//! action of its own (`KEYFLOW_*`, `extension.toml`), so ⌘K, menus, MCP
//! and keys reach the same thing; and MuseScore's keys for them, each a
//! `[[keys]] when = "view:chart"` in the manifest: the app's keymap runs
//! them while the chart has the focus, and the person rebinds them in the
//! action list like any key.

use keyflow::edit::{Mode, Op, Value};

/// An action that applies one operation, and the key it has by default.
pub struct OpAction {
    pub action: &'static str,
    pub op: Op,
    pub key: &'static str,
}

const fn op(action: &'static str, op: Op, key: &'static str) -> OpAction {
    OpAction { action, op, key }
}

/// The operations that are actions without a toolbar button (the
/// toolbar's are `crate::tools::TOOLS`), with MuseScore's keys.
pub const OP_ACTIONS: &[OpAction] = &[
    op("KEYFLOW_NOTE_A", Op::Letter('A'), "A"),
    op("KEYFLOW_NOTE_B", Op::Letter('B'), "B"),
    op("KEYFLOW_NOTE_C", Op::Letter('C'), "C"),
    op("KEYFLOW_NOTE_D", Op::Letter('D'), "D"),
    op("KEYFLOW_NOTE_E", Op::Letter('E'), "E"),
    op("KEYFLOW_NOTE_F", Op::Letter('F'), "F"),
    op("KEYFLOW_NOTE_G", Op::Letter('G'), "G"),
    op("KEYFLOW_ENTER_REST", Op::Rest, "0"),
    op("KEYFLOW_PITCH_UP", Op::Semitone(true), "↑"),
    op("KEYFLOW_PITCH_DOWN", Op::Semitone(false), "↓"),
    op("KEYFLOW_OCTAVE_UP", Op::Octave(true), "⌘↑"),
    op("KEYFLOW_OCTAVE_DOWN", Op::Octave(false), "⌘↓"),
    op("KEYFLOW_NEXT", Op::Next, "→"),
    op("KEYFLOW_PREVIOUS", Op::Prev, "←"),
    op("KEYFLOW_NEXT_BAR", Op::NextBar, "⌘→"),
    op("KEYFLOW_PREVIOUS_BAR", Op::PrevBar, "⌘←"),
    op("KEYFLOW_FIRST_BAR", Op::First, "⌥⌘←"),
    op("KEYFLOW_LAST_BAR", Op::Last, "⌥⌘→"),
    op("KEYFLOW_DELETE", Op::Delete, "⌫"),
    op("KEYFLOW_ESCAPE", Op::Escape, "Esc"),
    op("KEYFLOW_ADD_A", Op::AddToChord('A'), "⇧A"),
    op("KEYFLOW_ADD_B", Op::AddToChord('B'), "⇧B"),
    op("KEYFLOW_ADD_C", Op::AddToChord('C'), "⇧C"),
    op("KEYFLOW_ADD_D", Op::AddToChord('D'), "⇧D"),
    op("KEYFLOW_ADD_E", Op::AddToChord('E'), "⇧E"),
    op("KEYFLOW_ADD_F", Op::AddToChord('F'), "⇧F"),
    op("KEYFLOW_ADD_G", Op::AddToChord('G'), "⇧G"),
    op("KEYFLOW_DOUBLE", Op::Double, "W"),
    op("KEYFLOW_HALVE", Op::Halve, "Q"),
    op("KEYFLOW_LONGER", Op::Longer, "⇧W"),
    op("KEYFLOW_SHORTER", Op::Shorter, "⇧Q"),
    // MuseScore's Ctrl+Alt+1–4 (⌘⌥ on a Mac) and Ctrl+3.
    op("KEYFLOW_VOICE_1", Op::Voice(0), "⌥⌘1"),
    op("KEYFLOW_VOICE_2", Op::Voice(1), "⌥⌘2"),
    op("KEYFLOW_VOICE_3", Op::Voice(2), "⌥⌘3"),
    op("KEYFLOW_VOICE_4", Op::Voice(3), "⌥⌘4"),
    op("KEYFLOW_TRIPLET", Op::Triplet, "⌘3"),
    // A chord symbol's quality, as ↑ ↓ step its root.
    op("KEYFLOW_QUALITY_UP", Op::Quality(true), "⌥↑"),
    op("KEYFLOW_QUALITY_DOWN", Op::Quality(false), "⌥↓"),
];

/// The toolbar's actions that are note input's operations, and their keys
/// (MuseScore's: 1–7 the durations, `.` the dot, `-` `=` `+` flat, natural,
/// sharp, `T` a tie, N and M the modes).
pub const TOOL_KEYS: &[(&str, &str)] = &[
    ("KEYFLOW_NOTE_INPUT", "N"),
    ("KEYFLOW_INPUT_BY_DURATION", "M"),
    ("KEYFLOW_INPUT_SYNTAX", "K"),
    ("KEYFLOW_DURATION_64TH", "1"),
    ("KEYFLOW_DURATION_32ND", "2"),
    ("KEYFLOW_DURATION_16TH", "3"),
    ("KEYFLOW_DURATION_EIGHTH", "4"),
    ("KEYFLOW_DURATION_QUARTER", "5"),
    ("KEYFLOW_DURATION_HALF", "6"),
    ("KEYFLOW_DURATION_WHOLE", "7"),
    ("KEYFLOW_DOT", "."),
    ("KEYFLOW_FLAT", "-"),
    ("KEYFLOW_NATURAL", "="),
    ("KEYFLOW_SHARP", "+"),
    ("KEYFLOW_TIE", "T"),
];

/// Undo and redo of note input's own edits: keys in every mode.
pub const HISTORY_KEYS: &[(&str, &str)] = &[("KEYFLOW_UNDO", "⌘Z"), ("KEYFLOW_REDO", "⇧⌘Z")];

/// Y: chord symbols or notes, what a letter enters (not Tab, which the
/// app keeps for moving focus).
pub const TARGET_KEY: (&str, &str) = ("KEYFLOW_TARGET_SWITCH", "Y");

/// ↩: the selected chord symbol retyped, in a bar under the chart.
pub const CHORD_EDIT_KEY: (&str, &str) = ("KEYFLOW_CHORD_EDIT", "↵");

/// Every action with a key, and its key by default.
pub fn default_keys() -> impl Iterator<Item = (&'static str, &'static str)> {
    OP_ACTIONS
        .iter()
        .map(|a| (a.action, a.key))
        .chain(TOOL_KEYS.iter().copied())
        .chain(HISTORY_KEYS.iter().copied())
        .chain(std::iter::once(TARGET_KEY))
        .chain(std::iter::once(CHORD_EDIT_KEY))
}

/// The chords `key` is bound as: itself (`⌘` the Mac's command key), and
/// with `⌃` in its place off a Mac, where the command key is Ctrl
/// (`⇧⌘Z` and `⌃⇧Z`), in the Mac order (`⌃⌥⇧⌘`).
#[must_use]
pub fn chords(key: &str) -> Vec<String> {
    let mut out = vec![key.to_owned()];
    if key.contains('⌘') {
        let mods: String = ['⌃', '⌥', '⇧']
            .into_iter()
            .filter(|m| *m == '⌃' || key.contains(*m))
            .collect();
        let rest = key.trim_start_matches(['⌃', '⌥', '⇧', '⌘']);
        out.push(format!("{mods}{rest}"));
    }
    out
}

/// The operation `action` applies, if it is one of [`OP_ACTIONS`].
#[must_use]
pub fn op_of(action: &str) -> Option<Op> {
    OP_ACTIONS.iter().find(|a| a.action == action).map(|a| a.op)
}

/// The value a duration tool chooses.
#[must_use]
pub const fn value_of(d: crate::tools::Duration) -> Value {
    use crate::tools::Duration as D;
    match d {
        D::SixtyFourth => Value::SixtyFourth,
        D::ThirtySecond => Value::ThirtySecond,
        D::Sixteenth => Value::Sixteenth,
        D::Eighth => Value::Eighth,
        D::Quarter => Value::Quarter,
        D::Half => Value::Half,
        D::Whole => Value::Whole,
    }
}

/// A mode's tool, by its action.
#[must_use]
pub fn mode_of(action: &str) -> Option<Mode> {
    match action {
        "KEYFLOW_NOTE_INPUT" => Some(Mode::NoteName),
        "KEYFLOW_INPUT_BY_DURATION" => Some(Mode::Duration),
        "KEYFLOW_INPUT_SYNTAX" => Some(Mode::Syntax),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_key_is_bound_as_ctrl_off_a_mac_too() {
        assert_eq!(chords("N"), ["N"]);
        assert_eq!(chords("⌘Z"), ["⌘Z", "⌃Z"]);
        assert_eq!(chords("⇧⌘Z"), ["⇧⌘Z", "⌃⇧Z"]);
        assert_eq!(chords("⌥⌘1"), ["⌥⌘1", "⌃⌥1"]);
        assert_eq!(chords("⌥↑"), ["⌥↑"]);
    }

    #[test]
    fn every_key_is_bound_once() {
        let mut keys: Vec<&str> = default_keys().map(|(_, k)| k).collect();
        keys.sort_unstable();
        let n = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), n, "a key bound twice");
        for (action, _) in default_keys() {
            assert!(action.starts_with("KEYFLOW_"));
        }
    }
}
