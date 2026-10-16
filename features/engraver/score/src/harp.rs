//! Harp pedal settings in staff text.
//!
//! Finale arrangers draw a pedal diagram in a pedal font and spell it out
//! underneath — `--‡--‡--‡----|--‡--------‡--‡--‡` over
//! `D--C--B---|--E--F#--G-A` — and both arrive as plain text: the diagram
//! as rows of dashes and daggers that mean nothing outside that font, the
//! names padded with dashes to sit under it. A harpist reads the names
//! alone, set the standard way: `D C B | E F♯ G A` (left foot, then right).

/// A line drawn in the pedal font, or carrying the pedal names.
fn is_pedal_line(line: &str) -> bool {
    line.contains('‡') || line.contains("--")
}

/// A pedal name: a string letter, maybe an accidental, and not the start
/// of a word ("C major" is a scale, not the C pedal).
fn pedal_names(line: &str) -> Vec<(usize, usize, String)> {
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let (start, c) = chars[i];
        let prev_alpha = i > 0 && chars[i - 1].1.is_alphanumeric();
        if ('A'..='G').contains(&c) && !prev_alpha {
            let mut name = c.to_string();
            let mut j = i + 1;
            if let Some(&(_, acc)) = chars.get(j)
                && matches!(acc, '#' | 'b' | '♯' | '♭' | '♮')
                && !chars.get(j + 1).is_some_and(|(_, n)| n.is_alphabetic())
            {
                name.push(match acc {
                    '#' => '♯',
                    'b' => '♭',
                    other => other,
                });
                j += 1;
            }
            if !chars.get(j).is_some_and(|(_, n)| n.is_alphabetic()) {
                let end = chars.get(j).map_or(line.len(), |(b, _)| *b);
                out.push((start, end, name));
                i = j;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Rewrite a staff text's pedal lines; other lines are left alone.
#[must_use]
pub fn clean_pedal_text(text: &str) -> String {
    if !text.lines().any(is_pedal_line) {
        return text.to_string();
    }
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        if !is_pedal_line(line) {
            out.push(line.to_string());
            continue;
        }
        let names = pedal_names(line);
        if names.len() < 7 {
            // A diagram row (the names are on their own line) goes; any
            // other text with dashes in it ("C major scale: -- 17 --")
            // stays as written.
            if !line.chars().all(|c| matches!(c, '‡' | '-' | '|' | ' ')) {
                out.push(line.to_string());
            }
            continue;
        }
        let (left, right): (Vec<&str>, Vec<&str>) = (
            names[..3].iter().map(|n| n.2.as_str()).collect(),
            names[3..7].iter().map(|n| n.2.as_str()).collect(),
        );
        let mut set = format!("{} | {}", left.join(" "), right.join(" "));
        // A remark after the names ("*change Bb to B#*") stays.
        let tail = line[names[6].1..].trim().trim_matches('-').trim();
        if tail.chars().any(char::is_alphabetic) {
            set.push_str("  ");
            set.push_str(tail);
        }
        out.push(set);
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::clean_pedal_text;

    #[test]
    fn diagram_and_names_become_names() {
        assert_eq!(
            clean_pedal_text(
                "G major scale:\n--‡--‡--‡----|--‡--------‡--‡--‡\nD--C--B---|--E--F#--G-A"
            ),
            "G major scale:\nD C B | E F♯ G A"
        );
    }

    #[test]
    fn diagram_and_names_on_one_line() {
        assert_eq!(
            clean_pedal_text("‡ ‡ -‡-‡-----|----‡--‡--‡-D-C-Bb---Eb-F--G--A"),
            "D C B♭ | E♭ F G A"
        );
    }

    #[test]
    fn trailing_remark_is_kept() {
        assert_eq!(
            clean_pedal_text("--‡-‡--------‡ D-C-B#--E#-F-G-A *change Bb to B#*"),
            "D C B♯ | E♯ F G A  *change Bb to B#*"
        );
    }

    #[test]
    fn dashes_in_ordinary_text_stay() {
        assert_eq!(
            clean_pedal_text("C major scale: -- 17 --"),
            "C major scale: -- 17 --"
        );
    }

    #[test]
    fn plain_text_untouched() {
        assert_eq!(clean_pedal_text("C major scale:"), "C major scale:");
        assert_eq!(clean_pedal_text("cresc. molto..."), "cresc. molto...");
    }
}
