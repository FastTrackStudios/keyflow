//! Which notation system a line is written in (letters vs degrees), used
//! to read the ambiguous `b<digit>` root.

use crate::text::chart::parser::ChartParser;
use crate::text::chord::NotationSystem;

impl<'a> ChartParser<'a> {
    /// Classify a chord token's root notation system, for resolving the
    /// ambiguous `b<digit>` root. `Some(true)` = degree-based (Nashville
    /// number or Roman numeral), `Some(false)` = letter name, `None` = the
    /// token doesn't decide (ambiguous `b`+digit, rests, annotations).
    pub(super) fn classify_token_system(token: &str) -> Option<bool> {
        let t = token.trim_start_matches(['>', '.', '\'', '(']);
        let mut chars = t.chars();
        let c0 = chars.next()?;
        let c1 = t.chars().nth(1);
        let is_degree_digit = |c: char| ('1'..='7').contains(&c);
        let is_roman = |c: char| matches!(c, 'I' | 'V' | 'i' | 'v');
        match c0 {
            c if is_degree_digit(c) => Some(true),            // 1..7
            'I' | 'V' | 'i' | 'v' => Some(true),              // Roman numeral
            'A'..='G' => Some(false),                         // note letter (upper)
            'a' | 'c' | 'd' | 'e' | 'f' | 'g' => Some(false), // note letter (lower)
            '#' => match c1 {
                Some(c) if is_degree_digit(c) || is_roman(c) => Some(true),
                _ => None,
            },
            'b' => match c1 {
                // Symmetric with `#`: `bIII` and `b3` are both flats, and
                // neither has a reading as a note letter — lowercase `b` is
                // absent from the note-letter arms above for exactly this
                // reason.
                Some(c) if is_degree_digit(c) || is_roman(c) => Some(true),
                _ => None,
            },
            _ => None,
        }
    }

    /// Decide a scope's notation system by majority vote of its decisive
    /// tokens. `None` when there's a tie or no decisive tokens (defer).
    fn classify_scope<'t>(tokens: impl Iterator<Item = &'t str>) -> Option<bool> {
        let (mut letter, mut degree) = (0u32, 0u32);
        for t in tokens {
            match Self::classify_token_system(t) {
                Some(true) => degree += 1,
                Some(false) => letter += 1,
                None => {}
            }
        }
        match degree.cmp(&letter) {
            std::cmp::Ordering::Greater => Some(true),
            std::cmp::Ordering::Less => Some(false),
            std::cmp::Ordering::Equal => None,
        }
    }

    /// Whether the token explicitly forces a major triad, overriding the key's
    /// diatonic quality: `2M`, `2maj`, `2Major`, `2:maj`, `2△`, `2^`. Lowercase
    /// `m`/`dim`/`aug`/`sus` already parse to a non-major quality, so they're
    /// handled by the quality check — this only catches the major markers.
    pub(super) fn token_specifies_major(chord_part: &str, root: &str) -> bool {
        let rest = chord_part.strip_prefix(root).unwrap_or(chord_part);
        let rest = rest.strip_prefix(':').unwrap_or(rest);
        rest.starts_with('M')
            || rest.starts_with("maj")
            || rest.starts_with("Maj")
            || rest.starts_with('△')
            || rest.starts_with('^')
    }

    /// A bare leading flat degree: `b` followed by a digit 1-7 (`b3`, `b7`).
    pub(super) fn is_leading_flat_degree(token: &str) -> bool {
        let b = token.as_bytes();
        b.first() == Some(&b'b') && matches!(b.get(1), Some(d) if (b'1'..=b'7').contains(d))
    }

    /// A degree with its numeric quality run together: `17` (= `1:7`), `46`,
    /// `b17` — an optional accidental, a degree digit 1-7, then more digits.
    /// The `:` separator (`1:7`) is the readable form; this is the terse one.
    pub(super) fn is_merged_degree(token: &str) -> bool {
        let b = token.as_bytes();
        let i = usize::from(matches!(b.first(), Some(b'#' | b'b')));
        matches!(b.get(i), Some(d) if (b'1'..=b'7').contains(d))
            && b.get(i + 1).is_some_and(u8::is_ascii_digit)
    }

    /// Insert the implicit `:` into a merged degree (`17` -> `1:7`), so the
    /// digit after the degree is parsed as the quality, not eaten as a figure.
    /// Tokens that aren't a merged degree are returned unchanged.
    pub(super) fn split_merged_degree(token: &str) -> String {
        if Self::is_merged_degree(token) {
            let b = token.as_bytes();
            let i = usize::from(matches!(b.first(), Some(b'#' | b'b')));
            let (head, tail) = token.split_at(i + 1);
            format!("{head}:{tail}")
        } else {
            token.to_string()
        }
    }

    /// Resolve the notation system for an ambiguous `b<digit>` chord: the
    /// current line wins, then the chart's accumulated system (from earlier
    /// lines), else `Auto` (which reads `b7` as the note B).
    pub(super) fn resolve_notation_system(&self, line_tokens: &[&str]) -> NotationSystem {
        let to_system = |is_degree: bool| {
            if is_degree {
                NotationSystem::Degree
            } else {
                NotationSystem::Letter
            }
        };
        // Line scope wins.
        if let Some(d) = Self::classify_scope(line_tokens.iter().copied()) {
            return to_system(d);
        }
        // Chart scope: tally from earlier chord lines.
        match self.chart_degree_votes.cmp(&self.chart_letter_votes) {
            std::cmp::Ordering::Greater => NotationSystem::Degree,
            std::cmp::Ordering::Less => NotationSystem::Letter,
            std::cmp::Ordering::Equal => NotationSystem::Auto,
        }
    }

    /// Fold a chord line's decisive tokens into the chart-wide system tally, so
    /// later lines can fall back to it when their own scope is ambiguous.
    pub(super) fn record_line_system_votes(&mut self, line_tokens: &[&str]) {
        for t in line_tokens {
            match Self::classify_token_system(t) {
                Some(true) => self.chart_degree_votes += 1,
                Some(false) => self.chart_letter_votes += 1,
                None => {}
            }
        }
    }
}
