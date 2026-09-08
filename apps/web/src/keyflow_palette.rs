//! The command palette's catalog, for a buffer that holds a chart.
//!
//! The editor ships a markdown catalog — headings, lists, callouts, math,
//! wikilinks. That is right for a note and wrong for a chart.
//! `editor_view::palette::register_catalog` is the seam for saying so, and
//! this is what Keyflow says.
//!
//! # What belongs in it
//!
//! Keyflow's COMMANDS: the `/…` directive lines that configure a chart —
//! `/duration` for the default length of every chord, `/push` for how a
//! push divides, `/swing` for the feel — plus the two ways to name
//! something (`/alias`, `let`) and the chord shorthand.
//!
//! Not the chart language itself. A duration written on a chord (`C_8`) is
//! typed constantly and by hand; nobody needs a menu to reach it, and a
//! palette full of `_1 _2 _4 _8` is one you scroll past to find anything. A
//! directive is the opposite — rarely written, easy to misspell, and
//! invisible in the chart it governs. That is what a command palette is for.
//!
//! # Where the list comes from
//!
//! Not from here. The directives are described once in
//! `keyflow::chart::settings::DIRECTIVES`, next to the parser that reads
//! them, and this module turns that table into menu rows. Adding a setting
//! to the parser puts it in the menu with no second edit — which matters
//! more than tidiness, because an unknown `/setting` is a HARD ERROR from
//! `parse_setting_line`, not something quietly ignored. A menu that had
//! fallen behind the parser would hand someone a line that breaks their
//! chart.
//!
//! What stays here is the handful of things that are not settings: the two
//! naming forms that are not directives at all.

use editor::editor_view::palette::{CommandEntry, CommandKind};
use keyflow::chart::settings::DIRECTIVES;

/// Which group a directive belongs in.
///
/// `/duration`, `/push` and `/swing` set a value the whole chart inherits;
/// the rest are on/off. `/alias` names something, which is neither.
fn group_for(key: &str) -> &'static str {
    match key {
        "duration" | "push" | "swing" => "Defaults",
        "alias" => "Naming",
        _ => "Switches",
    }
}

/// Everything the palette offers while editing a chart.
///
/// The directives come from the parser's own table; the naming forms are
/// added here because they are not settings and the parser does not list
/// them among its own.
#[must_use]
pub fn commands() -> Vec<CommandEntry> {
    let mut out: Vec<CommandEntry> = DIRECTIVES
        .iter()
        .map(|spec| CommandEntry {
            label: spec.label,
            group: group_for(spec.key),
            // The line that lands in the document, under the label. It is a
            // better description than prose would be: it is the answer.
            desc: spec.example,
            kind: CommandKind::InsertBlockSnippet(spec.example, 0),
            // The shape of what gets inserted, not an abbreviation of the
            // name — at three characters that would only be a worse
            // spelling of the label sitting beside it.
            icon: "/",
        })
        .collect();

    // `/push` and `/swing` each take a feel that is worth one keystroke
    // rather than remembering the vocabulary. The table carries the
    // canonical example; these are the other value anyone actually writes.
    out.push(CommandEntry {
        label: "Push feel — triplet",
        group: "Defaults",
        desc: "/push triplet",
        kind: CommandKind::InsertBlockSnippet("/push triplet", 0),
        icon: "/",
    });
    out.push(CommandEntry {
        label: "Swing — triplet",
        group: "Defaults",
        desc: "/swing triplet",
        kind: CommandKind::InsertBlockSnippet("/swing triplet", 0),
        icon: "/",
    });

    // Not directives: the two ways to bind a name, and the chord shorthand.
    out.push(CommandEntry {
        label: "Let binding",
        group: "Naming",
        desc: "let name = C F G A",
        kind: CommandKind::InsertBlockSnippet("let name = C F G A", 0),
        icon: "let",
    });
    out.push(CommandEntry {
        label: "Chord shorthand",
        group: "Naming",
        desc: "Cm = Cm7b5",
        kind: CommandKind::InsertBlockSnippet("Cm = Cm7b5", 0),
        icon: "=",
    });

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyflow::text::chart::parse_chart;

    /// Every command inserts a line that parses.
    ///
    /// These are the failure-prone ones: an unrecognised `/setting` is a
    /// hard error out of `parse_setting_line`, not something quietly
    /// ignored, so a snippet with a typo in it breaks the chart under the
    /// caret rather than doing nothing.
    #[test]
    fn every_command_is_a_working_line() {
        for entry in commands() {
            let CommandKind::InsertBlockSnippet(text, _) = entry.kind else {
                panic!("{}: chart commands are block snippets", entry.label);
            };
            let src = format!("4/4 #C 120bpm\n{text}\n\nVS 4\nC F G A");
            assert!(
                parse_chart(&src).is_ok(),
                "{} inserts {text:?}, which does not parse:\n{src}",
                entry.label,
            );
        }
    }

    /// The menu is the parser's table, not a copy of it.
    ///
    /// This is the property the whole arrangement exists for: a setting
    /// added to `DIRECTIVES` shows up here without anyone editing this file.
    #[test]
    fn every_directive_reaches_the_menu() {
        let labels: Vec<_> = commands().iter().map(|c| c.label).collect();
        for spec in DIRECTIVES {
            assert!(
                labels.contains(&spec.label),
                "/{} is in the parser's table but not in the menu",
                spec.key,
            );
        }
        assert_eq!(
            commands().iter().filter(|c| c.icon == "/").count(),
            DIRECTIVES.len() + 2,
            "directive rows are the table plus the two extra feel values",
        );
    }

    /// Nothing from the markdown catalog, and nothing that is ordinary
    /// chart syntax rather than a command.
    #[test]
    fn the_catalog_is_commands_only() {
        let labels: Vec<_> = commands().iter().map(|c| c.label).collect();
        for not_a_command in [
            "Heading 1",
            "Bulleted list",
            "Wikilink",
            "Quarter note",
            "Verse",
            "Bar line",
        ] {
            assert!(
                !labels.contains(&not_a_command),
                "{not_a_command} is not a Keyflow command"
            );
        }
        assert!(labels.contains(&"Default duration"));
    }
}
