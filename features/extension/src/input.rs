//! Note input's actions and keys: every operation of `keyflow::edit` an
//! action of its own (`KEYFLOW_*`, `extension.toml`), so ⌘K, menus, MCP
//! and keys reach the same thing; and the keys that run them, MuseScore's
//! by default and each one a setting a person can change
//! (`keyflow.keys.<action>`, a `shortcut` control on the settings page).
//!
//! The view binds no keys: it hands each key pressed on the chart to the
//! pane ([`keyflow_ui::paint_view::ChartGesture`]), which looks it up here
//! and runs the action through the host.

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
    op("KEYFLOW_DELETE", Op::Delete, "⌦"),
    op("KEYFLOW_ESCAPE", Op::Escape, "⎋"),
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

/// Every action with a key, and its key by default.
pub fn default_keys() -> impl Iterator<Item = (&'static str, &'static str)> {
    OP_ACTIONS
        .iter()
        .map(|a| (a.action, a.key))
        .chain(TOOL_KEYS.iter().copied())
        .chain(HISTORY_KEYS.iter().copied())
        .chain(std::iter::once(TARGET_KEY))
}

/// The setting that holds `action`'s key.
#[must_use]
pub fn setting_key(action: &str) -> String {
    format!(
        "keyflow.keys.{}",
        action.trim_start_matches("KEYFLOW_").to_lowercase()
    )
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

/// A key as the view hands it (`KeyboardEvent.key` and the modifiers held)
/// written as a shortcut is (`⌘↑`, `⇧C`, `5`): what the keymap looks up.
/// Shift is part of a letter's chord (Shift+A is not A) but not of a
/// symbol's, which needs it to be typed at all (`+`).
#[must_use]
pub fn chord(key: &str, mods: &[&str]) -> String {
    let name = match key {
        "ArrowUp" => "↑".to_owned(),
        "ArrowDown" => "↓".to_owned(),
        "ArrowLeft" => "←".to_owned(),
        "ArrowRight" => "→".to_owned(),
        "Escape" => "⎋".to_owned(),
        "Delete" => "⌦".to_owned(),
        "Backspace" => "⌫".to_owned(),
        "Enter" => "↩".to_owned(),
        k if k.chars().count() == 1 => k.to_uppercase(),
        k => k.to_owned(),
    };
    // A typed symbol (`+`, `.`): Shift is how it was typed, not a chord.
    let symbol = name.chars().count() == 1
        && name
            .chars()
            .all(|c| c.is_ascii() && !c.is_ascii_alphanumeric());
    // Off a Mac, Ctrl is the command key (the app's ⌘ shortcuts are Ctrl).
    let command = mods.contains(&"Mod") || (!cfg!(target_os = "macos") && mods.contains(&"Ctrl"));
    let has = |m: &str| mods.contains(&m);
    let mut out = String::new();
    if has("Ctrl") && !command {
        out.push('⌃');
    }
    if has("Alt") {
        out.push('⌥');
    }
    if has("Shift") && !symbol {
        out.push('⇧');
    }
    if command {
        out.push('⌘');
    }
    out.push_str(&name);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_written_as_its_shortcut() {
        assert_eq!(chord("c", &[]), "C");
        assert_eq!(chord("C", &["Shift"]), "⇧C", "Shift+C is not C");
        assert_eq!(
            chord("+", &["Shift"]),
            "+",
            "a symbol's Shift is how it is typed"
        );
        assert_eq!(chord("ArrowUp", &["Mod", "Ctrl"]), "⌘↑");
        assert_eq!(chord("ArrowUp", &["Shift"]), "⇧↑");
        assert_eq!(chord("Escape", &[]), "⎋");
        assert_eq!(chord("5", &[]), "5");
        assert_eq!(chord("z", &["Mod", "Shift"]), "⇧⌘Z");
    }

    #[test]
    fn every_key_is_bound_once_and_every_action_has_a_setting() {
        let mut keys: Vec<&str> = default_keys().map(|(_, k)| k).collect();
        keys.sort_unstable();
        let n = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), n, "a key bound twice");
        for (action, _) in default_keys() {
            assert!(action.starts_with("KEYFLOW_"));
            assert!(setting_key(action).starts_with("keyflow.keys."));
        }
    }
}
