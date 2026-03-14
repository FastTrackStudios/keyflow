//! ChordPro format parser
//!
//! Parses ChordPro format: directives like `{title: ...}` and lyric lines with
//! inline chords like `[C]Verse [G]lyrics`.

use crate::chord::{ChordProChunk, ChordProDirective, ChordProDocument, ChordProLine, ChordProSection};

/// Parse ChordPro format text into a ChordProDocument
///
/// # Examples
///
/// ```ignore
/// {title: Amazing Grace}
/// {artist: Traditional}
///
/// [Verse]
/// [C]Amazing [F]grace, how [C]sweet the sound
/// [G]That saved a [C]wretch like me
/// ```
pub fn parse_chordpro(input: &str) -> Result<ChordProDocument, String> {
    let mut doc = ChordProDocument::new();
    let mut current_section: Option<ChordProSection> = None;

    for line in input.lines() {
        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            if let Some(mut section) = current_section.take() {
                section.lines.push(ChordProLine::Empty);
                current_section = Some(section);
            }
            continue;
        }

        // Parse directives: {name: value}
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            let content = &trimmed[1..trimmed.len() - 1];
            if let Some(colon_pos) = content.find(':') {
                let name = content[..colon_pos].trim().to_string();
                let value = content[colon_pos + 1..].trim().to_string();
                doc.directives.push(ChordProDirective::new(name, value));
            }
            continue;
        }

        // Parse section headers: [Section Name]
        if trimmed.starts_with('[') && trimmed.ends_with(']') && !trimmed.contains(']') {
            // Check if this looks like a section marker (single bracket pair at line start)
            let inner = &trimmed[1..trimmed.len() - 1];

            // If inner looks like a chord (single letter or chord symbol), skip it
            if is_chord_like(inner) {
                // This is a chord inline, not a section marker - process as lyric line
            } else {
                // This is a section marker
                if let Some(section) = current_section.take() {
                    doc.sections.push(section);
                }
                let mut new_section = ChordProSection::new();
                new_section.label = Some(inner.to_string());
                current_section = Some(new_section);
                continue;
            }
        }

        // Parse lyric lines with inline chords: [C]Text [G]more
        let line = parse_lyric_line(trimmed);
        if let Some(mut section) = current_section.take() {
            section.lines.push(line);
            current_section = Some(section);
        } else {
            // Create implicit section if none exists
            let mut section = ChordProSection::new();
            section.lines.push(line);
            current_section = Some(section);
        }
    }

    // Save final section
    if let Some(section) = current_section {
        doc.sections.push(section);
    }

    Ok(doc)
}

/// Parse a single lyric line with inline chords
/// Format: `[C]Text [G]more text [D]end`
fn parse_lyric_line(line: &str) -> ChordProLine {
    let mut chunks = Vec::new();
    let mut current_chord: Option<String> = None;
    let mut current_text = String::new();
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '[' => {
                // Start of chord bracket
                // First, save accumulated text if any
                if !current_text.is_empty() {
                    chunks.push(ChordProChunk {
                        chord: current_chord.take(),
                        text: current_text.clone(),
                    });
                    current_text.clear();
                }

                // Extract chord
                let mut chord = String::new();
                while let Some(&next_ch) = chars.peek() {
                    if next_ch == ']' {
                        chars.next(); // consume ]
                        break;
                    }
                    chord.push(chars.next().unwrap());
                }
                current_chord = Some(chord);
            }
            _ => {
                // Regular text
                current_text.push(ch);
            }
        }
    }

    // Save final chunk
    if !current_text.is_empty() || current_chord.is_some() {
        chunks.push(ChordProChunk {
            chord: current_chord,
            text: current_text,
        });
    }

    if chunks.is_empty() {
        ChordProLine::Empty
    } else {
        ChordProLine::Lyric(chunks)
    }
}

/// Check if a string looks like a chord symbol
fn is_chord_like(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let first_char = s.chars().next().unwrap();

    // Single letter chord like A, B, C, D, E, F, G
    if s.len() == 1 && matches!(first_char, 'A'..='G') {
        return true;
    }

    // Chord with modifiers: Cm, C7, Cmaj7, C#m, Cb7, etc.
    if matches!(first_char, 'A'..='G') {
        let rest = &s[1..];

        // Check for common chord patterns
        return rest.starts_with('m')
            || rest.starts_with('M')
            || rest.starts_with('7')
            || rest.starts_with('9')
            || rest.starts_with('#')
            || rest.starts_with('b')
            || rest.starts_with("maj")
            || rest.starts_with("min")
            || rest.starts_with("dim")
            || rest.starts_with("aug")
            || rest.starts_with("sus")
            || rest.starts_with('/');
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_chordpro() {
        let input = "{title: Test}\n[C]Verse";
        let doc = parse_chordpro(input).unwrap();

        assert_eq!(doc.directives.len(), 1);
        assert_eq!(doc.directives[0].name, "title");
    }

    #[test]
    fn test_parse_lyric_with_chords() {
        let line = parse_lyric_line("[C]Amazing [G]grace");

        match line {
            ChordProLine::Lyric(chunks) => {
                assert_eq!(chunks.len(), 2);
                assert_eq!(chunks[0].chord, Some("C".to_string()));
                assert_eq!(chunks[0].text, "Amazing ");
                assert_eq!(chunks[1].chord, Some("G".to_string()));
                assert_eq!(chunks[1].text, "grace");
            }
            _ => panic!("Expected Lyric line"),
        }
    }

    #[test]
    fn test_is_chord_like() {
        assert!(is_chord_like("C"));
        assert!(is_chord_like("Cm"));
        assert!(is_chord_like("C7"));
        assert!(is_chord_like("Cmaj7"));
        assert!(is_chord_like("G#m"));
        assert!(is_chord_like("Bb"));
        assert!(!is_chord_like("Verse"));
        assert!(!is_chord_like(""));
    }
}
