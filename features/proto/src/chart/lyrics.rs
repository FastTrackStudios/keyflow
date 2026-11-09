//! Lyrics track support
//!
//! Represents lyrics for a section with timing and alignment information.
//! Lyrics are organized as syllables that can be aligned to beat positions
//! and have chords attached at various positions (before word, on syllable, etc.)

use facet::Facet;

/// Where a lyric line came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Facet, Default)]
#[repr(u8)]
pub enum LyricSourceFormat {
    /// Standard `[lyrics]` content in the keyflow block.
    #[default]
    Keyflow,
    /// ChordPro `[C]lyric` content from a `--- chordpro ---` block.
    ChordPro,
    /// Imported or generated lyric content whose original format is unknown.
    Unknown,
}

/// The granularity this lyric line is intended to sync at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Facet, Default)]
#[repr(u8)]
pub enum LyricSyncLevel {
    /// Whole-section lyric presence/cueing.
    Section = 0,
    /// Slide/page level lyric grouping, like presentation software.
    Slide = 1,
    /// Source lyric line grouping.
    Line = 2,
    /// Word-level lyric timing.
    Word = 3,
    /// Syllable-level lyric timing.
    #[default]
    Syllable = 4,
}

/// A derived lyric segment at a requested sync granularity.
#[derive(Debug, Clone, PartialEq, Facet)]
pub struct LyricSegment {
    /// Segment granularity.
    pub level: LyricSyncLevel,

    /// Display text for this segment.
    pub text: String,

    /// First syllable index covered by this segment.
    pub start_syllable: usize,

    /// Exclusive end syllable index covered by this segment.
    pub end_syllable: usize,

    /// Start measure inherited from the first covered syllable.
    pub measure_index: usize,

    /// Start beat inherited from the first covered syllable.
    pub beat: f32,
}

impl LyricSegment {
    fn new(
        level: LyricSyncLevel,
        text: String,
        start_syllable: usize,
        end_syllable: usize,
        first: &LyricSyllable,
    ) -> Self {
        Self {
            level,
            text,
            start_syllable,
            end_syllable,
            measure_index: first.measure_index,
            beat: first.beat,
        }
    }
}

/// A complete line of lyrics for a section
#[derive(Debug, Clone, PartialEq, Facet)]
pub struct LyricLine {
    /// Individual syllables with timing and chord attachment info
    pub syllables: Vec<LyricSyllable>,

    /// Source format used to create this line.
    pub source_format: LyricSourceFormat,

    /// Intended sync granularity for this line.
    pub sync_level: LyricSyncLevel,

    /// Human-readable label from the source, such as a ChordPro environment label.
    pub label: Option<String>,

    /// Optional singer/person assignment for multi-vocal arrangements.
    pub singer: Option<String>,

    /// Optional musical part assignment, such as lead, harmony, or response.
    pub part: Option<String>,

    /// True when every syllable's `measure_index` / `beat` is a real
    /// placement written in the source (a timed lyric lane, see
    /// [`LyricLine::parse_timed`]) rather than the zero default. Only timed
    /// lines can be engraved under the bars they are sung over.
    pub timed: bool,
}

impl LyricLine {
    /// Create a new lyric line from syllables
    pub fn new(syllables: Vec<LyricSyllable>) -> Self {
        Self {
            syllables,
            source_format: LyricSourceFormat::Keyflow,
            sync_level: LyricSyncLevel::Syllable,
            label: None,
            singer: None,
            part: None,
            timed: false,
        }
    }

    /// Does this lyric text use the timed-lane form — words placed with
    /// `@<bar>` / `@<bar>:<beat>` stamps?
    pub fn is_timed_text(text: &str) -> bool {
        text.split_whitespace()
            .any(|tok| parse_stamp(tok).is_some())
    }

    /// Parse a **timed lyric lane**: words placed on bars and beats of the
    /// section they sit in.
    ///
    /// ```text
    /// @1:3 Joy @2 to @2:2.5 the @2:3 world
    /// @5 Let earth re-ceive her King
    /// ```
    ///
    /// `@<bar>` is a 1-based bar of the section, `@<bar>:<beat>` a 1-based
    /// beat in it (`2.5` is the "and" of 2). A word without its own stamp
    /// follows the previous word in the same place — the engraver flows it
    /// after its neighbour. Each source line ends a lyric line
    /// (`line_break_after`), and `re-ceive` splits into hyphenated
    /// syllables the way [`LyricLine::parse_simple`] does.
    ///
    /// The stored `measure_index` is 0-based and `beat` 0-based (beat 1 is
    /// `0.0`), matching [`LyricSyllable`].
    pub fn parse_timed(text: &str) -> Self {
        let mut syllables: Vec<LyricSyllable> = Vec::new();
        let (mut measure, mut beat) = (0usize, 0.0f32);
        for line in text.lines() {
            let before = syllables.len();
            for tok in line.split_whitespace() {
                if let Some((bar, b)) = parse_stamp(tok) {
                    measure = bar;
                    beat = b;
                    continue;
                }
                let parts: Vec<&str> = tok.split('-').filter(|p| !p.is_empty()).collect();
                let parts = if parts.is_empty() { vec![tok] } else { parts };
                let last = parts.len() - 1;
                // `re-` before a stamp: the word goes on at the next place.
                let carries_on = tok.len() > 1 && tok.ends_with('-');
                let continues = syllables.last().is_some_and(|s| s.hyphen_after);
                for (i, part) in parts.into_iter().enumerate() {
                    let mut syl = LyricSyllable::with_timing(part, measure, beat);
                    syl.word_initial = i == 0 && !continues;
                    syl.hyphen_after = i < last || carries_on;
                    syllables.push(syl);
                }
            }
            if syllables.len() > before
                && let Some(last) = syllables.last_mut()
            {
                last.line_break_after = true;
            }
        }
        let mut line = Self::new(syllables).with_sync_level(LyricSyncLevel::Word);
        line.timed = true;
        line
    }

    /// Write a timed line back in the form [`LyricLine::parse_timed`] reads:
    /// a stamp wherever a syllable's place differs from the one before it.
    pub fn to_timed_text(&self) -> String {
        let mut out = String::new();
        let mut last_place: Option<(usize, f32)> = None;
        let mut glue = false;
        for syl in &self.syllables {
            let place = (syl.measure_index, syl.beat);
            if last_place != Some(place) {
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push(' ');
                }
                out.push_str(&format_stamp(place.0, place.1));
                glue = false;
                last_place = Some(place);
            }
            if !glue && !out.is_empty() && !out.ends_with('\n') {
                out.push(' ');
            }
            out.push_str(&syl.text);
            glue = syl.hyphen_after;
            if glue {
                out.push('-');
            }
            if syl.line_break_after {
                out.push('\n');
                glue = false;
            }
        }
        out.trim_end().to_string()
    }

    /// Create an empty lyric line
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    /// Mark the source format for this lyric line.
    pub fn with_source_format(mut self, source_format: LyricSourceFormat) -> Self {
        self.source_format = source_format;
        self
    }

    /// Mark the intended sync level for this lyric line.
    pub fn with_sync_level(mut self, sync_level: LyricSyncLevel) -> Self {
        self.sync_level = sync_level;
        self
    }

    /// Set a human-readable source label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the singer/person assignment.
    pub fn with_singer(mut self, singer: impl Into<String>) -> Self {
        self.singer = Some(singer.into());
        self
    }

    /// Set the musical part assignment.
    pub fn with_part(mut self, part: impl Into<String>) -> Self {
        self.part = Some(part.into());
        self
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
                        line_break_after: false,
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
                    line_break_after: false,
                });
            }
        }

        Self::new(syllables)
    }

    /// Check if any syllables have chord attachments
    pub fn has_chords(&self) -> bool {
        self.syllables.iter().any(|s| s.chord.is_some())
    }

    /// Serialize with {Chord}syllable format (for round-tripping)
    pub fn to_chord_text(&self) -> String {
        let mut result = String::new();
        for (i, syl) in self.syllables.iter().enumerate() {
            if let Some(chord) = &syl.chord {
                result.push('{');
                result.push_str(chord);
                result.push('}');
            }
            result.push_str(&syl.text);
            if syl.hyphen_after {
                result.push('-');
            } else if i < self.syllables.len() - 1 {
                result.push(' ');
            }
        }
        result
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

    /// Derive lyric segments for a coarser sync level.
    ///
    /// The source of truth remains the most detailed available syllable list:
    /// - syllables are direct one-syllable segments;
    /// - words are reconstructed from `word_initial` and `hyphen_after`;
    /// - source lines are reconstructed from `line_break_after`;
    /// - slides are grouped from words using simple presentation heuristics;
    /// - section is a single segment spanning the full line.
    pub fn derive_segments(&self, level: LyricSyncLevel) -> Vec<LyricSegment> {
        match level {
            LyricSyncLevel::Syllable => self.derive_syllable_segments(),
            LyricSyncLevel::Word => self.derive_word_segments(),
            LyricSyncLevel::Line => self.derive_line_segments(),
            LyricSyncLevel::Slide => self.derive_slide_segments(),
            LyricSyncLevel::Section => self.derive_section_segments(),
        }
    }

    /// Derive one segment per syllable.
    pub fn derive_syllable_segments(&self) -> Vec<LyricSegment> {
        self.syllables
            .iter()
            .enumerate()
            .map(|(idx, syl)| {
                LyricSegment::new(
                    LyricSyncLevel::Syllable,
                    syl.text.clone(),
                    idx,
                    idx + 1,
                    syl,
                )
            })
            .collect()
    }

    /// Derive words from syllables.
    pub fn derive_word_segments(&self) -> Vec<LyricSegment> {
        if self.syllables.iter().any(|s| s.word_initial) {
            return self.derive_word_segments_from_initials();
        }

        self.derive_word_segments_from_hyphens()
    }

    fn derive_word_segments_from_initials(&self) -> Vec<LyricSegment> {
        let mut out = Vec::new();
        let mut start: Option<usize> = None;

        for idx in 0..self.syllables.len() {
            let syl = &self.syllables[idx];
            if syl.word_initial || start.is_none() {
                if let Some(s) = start {
                    push_word_segment(&mut out, &self.syllables, s, idx);
                }
                start = Some(idx);
            }
        }

        if let Some(s) = start {
            push_word_segment(&mut out, &self.syllables, s, self.syllables.len());
        }

        out
    }

    fn derive_word_segments_from_hyphens(&self) -> Vec<LyricSegment> {
        let mut out = Vec::new();
        let mut start: Option<usize> = None;

        for idx in 0..self.syllables.len() {
            let syl = &self.syllables[idx];
            if start.is_none() || (syl.word_initial && idx != start.unwrap()) {
                if let Some(s) = start {
                    push_word_segment(&mut out, &self.syllables, s, idx);
                }
                start = Some(idx);
            }

            if !syl.hyphen_after
                && let Some(s) = start.take()
            {
                push_word_segment(&mut out, &self.syllables, s, idx + 1);
            }
        }

        if let Some(s) = start {
            push_word_segment(&mut out, &self.syllables, s, self.syllables.len());
        }

        out
    }

    /// Derive slide-level groups from words.
    ///
    /// Heuristics are intentionally conservative and deterministic:
    /// - break after strong punctuation (`.`, `!`, `?`, `;`, `:`);
    /// - otherwise keep slides to about eight words;
    /// - if a comma appears after at least four words, it can end a slide.
    pub fn derive_slide_segments(&self) -> Vec<LyricSegment> {
        let words = self.derive_word_segments();
        if words.is_empty() {
            return Vec::new();
        }

        let mut out = Vec::new();
        let mut slide_start = 0usize;
        let mut words_in_slide = 0usize;

        for (idx, word) in words.iter().enumerate() {
            words_in_slide += 1;
            let punctuation_break = ends_with_strong_break(&word.text);
            let comma_break = words_in_slide >= 4 && word.text.ends_with(',');
            let length_break = words_in_slide >= 8;
            let is_last = idx + 1 == words.len();

            if punctuation_break || comma_break || length_break || is_last {
                push_slide_segment(&mut out, &words, slide_start, idx + 1);
                slide_start = idx + 1;
                words_in_slide = 0;
            }
        }

        out
    }

    /// Derive source lyric lines from syllable boundaries.
    pub fn derive_line_segments(&self) -> Vec<LyricSegment> {
        let mut out = Vec::new();
        let mut start = 0usize;

        for (idx, syl) in self.syllables.iter().enumerate() {
            let is_last = idx + 1 == self.syllables.len();
            if syl.line_break_after || is_last {
                push_line_segment(&mut out, &self.syllables, start, idx + 1);
                start = idx + 1;
            }
        }

        out
    }

    /// Derive one section-level segment.
    pub fn derive_section_segments(&self) -> Vec<LyricSegment> {
        let Some(first) = self.syllables.first() else {
            return Vec::new();
        };

        vec![LyricSegment::new(
            LyricSyncLevel::Section,
            self.full_text(),
            0,
            self.syllables.len(),
            first,
        )]
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

    /// Whether this syllable ends a source lyric line.
    pub line_break_after: bool,
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
            line_break_after: false,
        }
    }

    /// Create a syllable with timing information
    pub fn with_timing(text: impl Into<String>, measure_index: usize, beat: f32) -> Self {
        Self {
            text: text.into(),
            hyphen_after: false,
            chord: None,
            chord_attachment: None,
            measure_index,
            beat,
            word_initial: false,
            line_break_after: false,
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

    /// Mark this syllable as ending a source lyric line.
    pub fn line_break_after(mut self) -> Self {
        self.line_break_after = true;
        self
    }
}

fn push_word_segment(
    out: &mut Vec<LyricSegment>,
    syllables: &[LyricSyllable],
    start: usize,
    end: usize,
) {
    if start >= end || start >= syllables.len() {
        return;
    }

    let mut text = String::new();
    for (offset, syl) in syllables[start..end].iter().enumerate() {
        let previous_had_hyphen = offset > 0 && syllables[start + offset - 1].hyphen_after;
        let previous_was_word_join = offset > 0 && !syl.word_initial;
        if offset > 0 && !previous_had_hyphen && !previous_was_word_join {
            text.push(' ');
        }
        text.push_str(&syl.text);
        if syl.hyphen_after {
            text.push('-');
        }
    }

    out.push(LyricSegment::new(
        LyricSyncLevel::Word,
        text,
        start,
        end,
        &syllables[start],
    ));
}

fn push_slide_segment(
    out: &mut Vec<LyricSegment>,
    words: &[LyricSegment],
    start_word: usize,
    end_word: usize,
) {
    if start_word >= end_word || start_word >= words.len() {
        return;
    }

    let first = &words[start_word];
    let last = &words[end_word - 1];
    out.push(LyricSegment {
        level: LyricSyncLevel::Slide,
        text: words[start_word..end_word]
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        start_syllable: first.start_syllable,
        end_syllable: last.end_syllable,
        measure_index: first.measure_index,
        beat: first.beat,
    });
}

fn push_line_segment(
    out: &mut Vec<LyricSegment>,
    syllables: &[LyricSyllable],
    start: usize,
    end: usize,
) {
    if start >= end || start >= syllables.len() {
        return;
    }

    let words = segments_for_syllable_range(syllables, start, end, LyricSyncLevel::Word);
    out.push(LyricSegment::new(
        LyricSyncLevel::Line,
        words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        start,
        end,
        &syllables[start],
    ));
}

fn segments_for_syllable_range(
    syllables: &[LyricSyllable],
    start: usize,
    end: usize,
    level: LyricSyncLevel,
) -> Vec<LyricSegment> {
    let mut line = LyricLine::new(syllables[start..end].to_vec());
    for syl in &mut line.syllables {
        syl.line_break_after = false;
    }
    let mut segments = match level {
        LyricSyncLevel::Syllable => line.derive_syllable_segments(),
        LyricSyncLevel::Word => line.derive_word_segments(),
        LyricSyncLevel::Line => line.derive_line_segments(),
        LyricSyncLevel::Slide => line.derive_slide_segments(),
        LyricSyncLevel::Section => line.derive_section_segments(),
    };
    for segment in &mut segments {
        segment.start_syllable += start;
        segment.end_syllable += start;
    }
    segments
}

fn ends_with_strong_break(text: &str) -> bool {
    text.ends_with('.')
        || text.ends_with('!')
        || text.ends_with('?')
        || text.ends_with(';')
        || text.ends_with(':')
}

/// `@3` → bar index 2, beat 0.0; `@3:2.5` → bar index 2, beat 1.5.
/// Anything else (a word, `@vox`, `@0`, a beat below 1) is not a stamp.
fn parse_stamp(tok: &str) -> Option<(usize, f32)> {
    let body = tok.strip_prefix('@')?;
    let (bar, beat) = match body.split_once(':') {
        Some((bar, beat)) => (bar, Some(beat)),
        None => (body, None),
    };
    if bar.is_empty() || !bar.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let bar: usize = bar.parse().ok()?;
    let beat: f32 = match beat {
        Some(b) if !b.is_empty() && b.chars().all(|c| c.is_ascii_digit() || c == '.') => {
            b.parse().ok()?
        }
        Some(_) => return None,
        None => 1.0,
    };
    (bar >= 1 && beat >= 1.0).then(|| (bar - 1, beat - 1.0))
}

/// The inverse of [`parse_stamp`]: beat 1 is left off, and a beat is
/// written with as few decimals as it needs.
fn format_stamp(measure_index: usize, beat: f32) -> String {
    let bar = measure_index + 1;
    if beat.abs() < 1e-4 {
        return format!("@{bar}");
    }
    let beat = ((beat + 1.0) * 1000.0).round() / 1000.0;
    let mut b = format!("{beat:.3}");
    while b.ends_with('0') {
        b.pop();
    }
    if b.ends_with('.') {
        b.pop();
    }
    format!("@{bar}:{b}")
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
        let line = LyricLine::new(vec![
            LyricSyllable::new("Twinkle"),
            LyricSyllable::new("twinkle").hyphenated(),
            LyricSyllable::new("little"),
        ]);
        // Note: we'll get "Twinkle-twinkle little" since syllables are joined
        let text = line.full_text();
        assert!(text.contains("Twinkle") && text.contains("twinkle"));
    }

    #[test]
    fn derives_words_from_syllables() {
        let line = LyricLine::parse_simple("A-ma-zing grace");
        let words = line.derive_word_segments();

        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "A-ma-zing");
        assert_eq!(words[0].start_syllable, 0);
        assert_eq!(words[0].end_syllable, 3);
        assert_eq!(words[1].text, "grace");
        assert_eq!(words[1].start_syllable, 3);
        assert_eq!(words[1].end_syllable, 4);
    }

    #[test]
    fn derives_slides_from_words_with_punctuation() {
        let line = LyricLine::parse_simple("Amazing grace, how sweet the sound. That saved me.");
        let slides = line.derive_slide_segments();

        assert_eq!(slides.len(), 2);
        assert_eq!(slides[0].text, "Amazing grace, how sweet the sound.");
        assert_eq!(slides[0].start_syllable, 0);
        assert_eq!(slides[1].text, "That saved me.");
        assert_eq!(slides[1].end_syllable, line.syllables.len());
    }

    #[test]
    fn derives_section_from_any_detailed_line() {
        let line = LyricLine::parse_simple("A-ma-zing grace");
        let sections = line.derive_segments(LyricSyncLevel::Section);

        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].text, "A-ma-zing grace");
        assert_eq!(sections[0].start_syllable, 0);
        assert_eq!(sections[0].end_syllable, line.syllables.len());
    }

    #[test]
    fn derives_lines_from_source_boundaries() {
        let mut line =
            LyricLine::parse_simple("Holy there is no one like You There is none beside You");
        line.syllables[6].line_break_after = true;

        let lines = line.derive_segments(LyricSyncLevel::Line);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "Holy there is no one like You");
        assert_eq!(lines[0].start_syllable, 0);
        assert_eq!(lines[0].end_syllable, 7);
        assert_eq!(lines[1].text, "There is none beside You");
        assert_eq!(lines[1].start_syllable, 7);
    }

    #[test]
    fn derives_syllable_segments_directly() {
        let line = LyricLine::parse_simple("A-ma");
        let syllables = line.derive_segments(LyricSyncLevel::Syllable);

        assert_eq!(syllables.len(), 2);
        assert_eq!(syllables[0].text, "A");
        assert_eq!(syllables[1].text, "ma");
    }

    #[test]
    fn timed_lane_places_words_on_bars_and_beats() {
        let line = LyricLine::parse_timed("@1:3 Sing @2 a new @2:2.5 song\n@4 hel-lo there");
        assert!(line.timed);
        let at: Vec<_> = line
            .syllables
            .iter()
            .map(|s| (s.text.as_str(), s.measure_index, s.beat))
            .collect();
        assert_eq!(
            at,
            vec![
                ("Sing", 0, 2.0),
                ("a", 1, 0.0),
                ("new", 1, 0.0), // no stamp of its own: shares the place before it
                ("song", 1, 1.5),
                ("hel", 3, 0.0),
                ("lo", 3, 0.0),
                ("there", 3, 0.0),
            ]
        );
        assert!(line.syllables[3].line_break_after);
        assert!(line.syllables[4].hyphen_after);
        assert!(!line.syllables[5].word_initial);
    }

    #[test]
    fn only_numeric_stamps_count_as_timing() {
        assert!(LyricLine::is_timed_text("@3 word"));
        assert!(LyricLine::is_timed_text("x @3:2.5 word"));
        assert!(!LyricLine::is_timed_text("@vox word"));
        assert!(!LyricLine::is_timed_text("@0 word"));
        assert!(!LyricLine::is_timed_text("email me@3 now"));
    }

    #[test]
    fn timed_lane_round_trips_through_text() {
        let src = "@1:3 Sing @2 a new @2:2.5 song\n@4 hel-lo @4:4 there-\n@5 fore";
        let line = LyricLine::parse_timed(src);
        let again = LyricLine::parse_timed(&line.to_timed_text());
        assert_eq!(line, again);
        assert_eq!(line.to_timed_text(), src);
    }
}
