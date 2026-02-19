//! Chart Commands
//!
//! Special commands that can be applied to chords, melodies, or rhythms
//! Commands can be specified with slash syntax (/fermata) or shorthand (->)

use facet::Facet;
use std::fmt;

/// Commands that can be applied to musical elements
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
#[repr(u8)]
pub enum Command {
    /// Fermata - hold the note/chord longer
    Fermata,

    /// Accent - emphasize the note/chord on the downbeat
    /// Syntax: '>C (accent after push marker = accent on beat 1)
    Accent,

    /// Accent on the pushed/anticipation beat
    /// Syntax: >'C (accent before push marker = accent on the push, e.g., beat 4.66)
    /// This renders the accent at the spillback position in the previous measure
    AccentOnPush,
}

impl Command {
    /// Parse a command from a slash notation (e.g., "/fermata", "/accent")
    pub fn parse_slash(text: &str) -> Option<Self> {
        let text = text.trim().trim_start_matches('/').trim().to_lowercase();

        match text.as_str() {
            "fermata" => Some(Command::Fermata),
            "accent" => Some(Command::Accent),
            _ => None,
        }
    }

    /// Get the symbol representation of this command
    pub fn symbol(&self) -> &'static str {
        match self {
            Command::Fermata => "𝄐",                        // Unicode fermata symbol
            Command::Accent | Command::AccentOnPush => ">", // Accent symbol
        }
    }

    /// Get the display name of this command
    pub fn name(&self) -> &'static str {
        match self {
            Command::Fermata => "fermata",
            Command::Accent => "accent",
            Command::AccentOnPush => "accent_on_push",
        }
    }

    /// Check if this is any type of accent command
    pub fn is_accent(&self) -> bool {
        matches!(self, Command::Accent | Command::AccentOnPush)
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.symbol())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fermata() {
        assert_eq!(Command::parse_slash("/fermata"), Some(Command::Fermata));
        assert_eq!(Command::parse_slash("/FERMATA"), Some(Command::Fermata));
        assert_eq!(Command::parse_slash("  /fermata  "), Some(Command::Fermata));
    }

    #[test]
    fn test_parse_accent() {
        assert_eq!(Command::parse_slash("/accent"), Some(Command::Accent));
        assert_eq!(Command::parse_slash("/ACCENT"), Some(Command::Accent));
    }

    #[test]
    fn test_parse_invalid() {
        assert_eq!(Command::parse_slash("/unknown"), None);
        assert_eq!(Command::parse_slash("not_a_command"), None);
    }

    #[test]
    fn test_symbols() {
        assert_eq!(Command::Fermata.symbol(), "𝄐");
        assert_eq!(Command::Accent.symbol(), ">");
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", Command::Fermata), "𝄐");
        assert_eq!(format!("{}", Command::Accent), ">");
    }
}
