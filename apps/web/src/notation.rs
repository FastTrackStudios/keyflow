//! Which notation the engraved chart is drawn in, independent of how the
//! source was typed.
//!
//! A chart is written once and read by different people. The person who
//! wrote `1 4 6 5` is thinking in degrees; the guitarist reading it at
//! rehearsal wants `E A C#m B`; the arranger wants `I IV vi V`. Those are
//! three views of one chart, not three charts, so this never touches the
//! source — it re-notates on the way to the engraver via
//! [`keyflow::transpose_source`], and the editor keeps showing exactly
//! what was typed.

use keyflow::NotationSystem;

use crate::prefs;

/// The choices offered in the editor, in the order they are shown.
pub const CHOICES: &[(Notation, &str, &str)] = &[
    (
        Notation::AsWritten,
        "As written",
        "Engrave each chord in the notation it was typed in",
    ),
    (
        Notation::Letters,
        "Letters",
        "Force letter names — E A C#m B",
    ),
    (
        Notation::Numbers,
        "Numbers",
        "Force Nashville numbers — 1 4 6m 5",
    ),
    (Notation::Roman, "Roman", "Force Roman numerals — I IV vi V"),
];

/// The editor's notation setting.
///
/// Distinct from [`NotationSystem`] because this is a *user preference*
/// with a stored spelling and a label, and that is a different thing from
/// the engine's rendering mode even though they map one to one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Notation {
    #[default]
    AsWritten,
    Letters,
    Numbers,
    Roman,
}

impl Notation {
    /// The engine mode this asks for.
    #[must_use]
    pub const fn system(self) -> NotationSystem {
        match self {
            Self::AsWritten => NotationSystem::AsWritten,
            Self::Letters => NotationSystem::Letters,
            Self::Numbers => NotationSystem::Nashville,
            Self::Roman => NotationSystem::Roman,
        }
    }

    /// The stored spelling. Written out rather than derived from the
    /// variant name so renaming a variant cannot silently invalidate
    /// everyone's saved preference.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::AsWritten => "as-written",
            Self::Letters => "letters",
            Self::Numbers => "numbers",
            Self::Roman => "roman",
        }
    }

    /// Parse a stored spelling. An unknown value — a preference written by
    /// a later version, say — reads as the default rather than an error.
    #[must_use]
    pub fn from_key(key: &str) -> Self {
        match key {
            "letters" => Self::Letters,
            "numbers" => Self::Numbers,
            "roman" => Self::Roman,
            _ => Self::AsWritten,
        }
    }

    /// The remembered choice, or `AsWritten`.
    #[must_use]
    pub fn remembered() -> Self {
        prefs::string(prefs::NOTATION).map_or(Self::AsWritten, |v| Self::from_key(&v))
    }

    /// Remember this choice for next time.
    pub fn remember(self) {
        prefs::set_string(prefs::NOTATION, self.key());
    }

    /// Re-notate `source` for the engraver. `AsWritten` returns it
    /// byte-for-byte, so the common case costs nothing.
    #[must_use]
    pub fn apply(self, source: &str) -> String {
        if self == Self::AsWritten {
            return source.to_string();
        }
        keyflow::transpose_source(
            source,
            &keyflow::ChartView {
                notation: self.system(),
                ..Default::default()
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IN_E: &str = "Song Title - Artist Name\n4/4 #E 120bpm\n\nVS 4\n1 4 6 5\n";

    fn chords(source: &str) -> String {
        source.lines().last().unwrap_or("").to_string()
    }

    #[test]
    fn every_choice_round_trips_through_storage() {
        for (n, _, _) in CHOICES {
            assert_eq!(Notation::from_key(n.key()), *n);
        }
    }

    #[test]
    fn an_unknown_stored_value_reads_as_the_default() {
        // A preference saved by a future version must not break this one.
        assert_eq!(Notation::from_key("solfege"), Notation::AsWritten);
        assert_eq!(Notation::from_key(""), Notation::AsWritten);
    }

    #[test]
    fn as_written_is_byte_for_byte() {
        assert_eq!(Notation::AsWritten.apply(IN_E), IN_E);
    }

    #[test]
    fn forcing_renders_the_same_chart_three_ways() {
        assert_eq!(chords(&Notation::Letters.apply(IN_E)), "E A C#m B");
        assert_eq!(chords(&Notation::Numbers.apply(IN_E)), "1 4 6m 5");
        assert_eq!(chords(&Notation::Roman.apply(IN_E)), "I IV vi V");
    }

    #[test]
    fn a_chart_written_in_letters_forces_just_as_well() {
        let letters = "Song Title - Artist Name\n4/4 #E 120bpm\n\nVS 4\nE A C#m B\n";
        assert_eq!(chords(&Notation::Numbers.apply(letters)), "1 4 6m 5");
        assert_eq!(chords(&Notation::Roman.apply(letters)), "I IV vi V");
        assert_eq!(chords(&Notation::Letters.apply(letters)), "E A C#m B");
    }

    #[test]
    fn the_header_survives_re_notation() {
        // Only chord lines are rewritten — the title, the metadata line and
        // the section marker come through untouched.
        let out = Notation::Numbers.apply(IN_E);
        assert!(out.starts_with("Song Title - Artist Name\n4/4 #E 120bpm\n"));
        assert!(out.contains("VS 4"));
    }
}
