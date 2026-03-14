//! Lyrics track support
//!
//! Represents lyrics for a section with timing and alignment information.
//! Lyrics are organized as syllables that can be aligned to beat positions
//! and have chords attached at various positions (before word, on syllable, etc.)

use facet::Facet;

/// A complete line of lyrics for a section
#[derive(Debug, Clone, PartialEq, Facet)]
pub struct LyricLine {
    /// Individual syllables with timing and chord attachment info
    pub syllables: Vec<LyricSyllable>,
}

impl LyricLine {
    /// Create a new lyric line from syllables
    pub fn new(syllables: Vec<LyricSyllable>) -> Self {
        Self { syllables }
    }

    /// Create an empty lyric line
    pub fn empty() -> Self {
        Self {
            syllables: Vec::new(),
        }
    }

    /// Parse a simple lyric line (whitespace-separated syllables)
    ///
    /// Format: "word1 word2 word3"
    /// Hyphens at word boundary indicate melisma: "twin-kle" → "twin", "kle" (hyphenated)
    ///
    /// # Examples
    /// - "Twinkle twinkle little star" → 4 syllables (no hyphens)
    /// - "A-ma-zing grace how sweet" → 5 units, first 3 hyphenated internally
    pub fn parse_simple(text: &str) -> Self {
        let mut syllables = Vec::new();

        for word in text.split_whitespace() {
            if word.is_empty() {
                continue;
            }

            // Check if this word contains internal hyphens (melisma)
            if word.contains('-') {
                let parts: Vec<&str> = word.split('-').collect();
                for (i, part) in parts.iter().enumerate() {
                    if part.is_empty() {
                        continue;
                    }
                    // All but the last part have hyphens after them
                    let hyphen_after = i < parts.len() - 1;
                    syllables.push(LyricSyllable {
                        text: part.to_string(),
                        hyphen_after,
                        chord: None,
                        chord_attachment: None,
                        measure_index: 0,
                        beat: 0.0,
                        word_initial: i == 0,
                    });
                }
            } else {
                // Simple word (no internal hyphens)
                syllables.push(LyricSyllable {
                    text: word.to_string(),
                    hyphen_after: false,
                    chord: None,
                    chord_attachment: None,
                    measure_index: 0,
                    beat: 0.0,
                    word_initial: true,
                });
            }
        }

        Self { syllables }
    }

    /// Get the full lyric text (with hyphens for melisma)
    pub fn full_text(&self) -> String {
        let mut result = String::new();
        for (i, syl) in self.syllables.iter().enumerate() {
            result.push_str(&syl.text);
            if syl.hyphen_after {
                result.push('-');
            } else if i < self.syllables.len() - 1 {
                result.push(' ');
            }
        }
        result
    }
}

impl Default for LyricLine {
    fn default() -> Self {
        Self::empty()
    }
}

/// How a chord attaches to a syllable
#[derive(Debug, Clone, Copy, PartialEq, Eq, Facet)]
#[repr(u8)]
pub enum ChordAttachment {
    /// Chord appears before the entire word (above first syllable)
    BeforeWord,

    /// Chord appears at the beginning of this syllable
    AtSyllableStart,

    /// Chord appears at the end of this syllable
    AtSyllableEnd,

    /// Chord appears between this syllable and the next
    BetweenSyllables,

    /// Chord appears after the entire word
    AfterWord,
}

/// A single syllable within a lyric line
#[derive(Debug, Clone, PartialEq, Facet)]
pub struct LyricSyllable {
    /// The text of this syllable
    pub text: String,

    /// Whether this syllable is followed by a hyphen (for melisma/multi-note syllables)
    /// e.g., "twin-" in "twinkle" when spread across multiple notes
    pub hyphen_after: bool,

    /// Optional chord attached to this syllable
    pub chord: Option<String>,

    /// How the chord attaches to this syllable
    pub chord_attachment: Option<ChordAttachment>,

    /// Which measure this syllable aligns to (default: 0)
    pub measure_index: usize,

    /// Beat position within the measure (0-based, 0.0 = first beat)
    /// This allows fine-grained timing even within a measure
    pub beat: f32,

    /// Whether this is the first syllable of a word (for layout purposes)
    pub word_initial: bool,
}

impl LyricSyllable {
    /// Create a new syllable with default timing
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            hyphen_after: false,
            chord: None,
            chord_attachment: None,
            measure_index: 0,
            beat: 0.0,
            word_initial: false,
        }
    }

    /// Create a syllable with timing information
    pub fn with_timing(
        text: impl Into<String>,
        measure_index: usize,
        beat: f32,
    ) -> Self {
        Self {
            text: text.into(),
            hyphen_after: false,
            chord: None,
            chord_attachment: None,
            measure_index,
            beat,
            word_initial: false,
        }
    }

    /// Mark this syllable as hyphenated (melisma)
    pub fn hyphenated(mut self) -> Self {
        self.hyphen_after = true;
        self
    }

    /// Attach a chord to this syllable
    pub fn with_chord(mut self, chord: impl Into<String>, attachment: ChordAttachment) -> Self {
        self.chord = Some(chord.into());
        self.chord_attachment = Some(attachment);
        self
    }

    /// Mark this as the first syllable of a word
    pub fn word_initial(mut self) -> Self {
        self.word_initial = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_lyrics() {
        let line = LyricLine::parse_simple("Twinkle twinkle little star");
        assert_eq!(line.syllables.len(), 4);
        assert_eq!(line.syllables[0].text, "Twinkle");
        assert!(!line.syllables[0].hyphen_after);
    }

    #[test]
    fn test_parse_hyphenated_lyrics() {
        let line = LyricLine::parse_simple("A-ma-zing grace");
        assert_eq!(line.syllables.len(), 4); // A, ma, zing, grace
        assert_eq!(line.syllables[0].text, "A");
        assert!(line.syllables[0].hyphen_after);
        assert_eq!(line.syllables[1].text, "ma");
        assert!(line.syllables[1].hyphen_after);
        assert_eq!(line.syllables[2].text, "zing");
        assert!(!line.syllables[2].hyphen_after);
    }

    #[test]
    fn test_lyric_line_full_text() {
        let mut line = LyricLine::new(vec![
            LyricSyllable::new("Twinkle"),
            LyricSyllable::new("twinkle").hyphenated(),
            LyricSyllable::new("little"),
        ]);
        // Note: we'll get "Twinkle-twinkle little" since syllables are joined
        let text = line.full_text();
        assert!(text.contains("Twinkle") && text.contains("twinkle"));
    }
}
