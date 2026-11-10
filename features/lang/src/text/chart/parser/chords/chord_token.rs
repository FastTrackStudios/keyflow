//! A single chord token into a `ChordInstance`.

use crate::text::chart::parser::ChartParser;
use crate::text::chart::types::ChordInstance;
use crate::text::chord::{ChordQuality, ChordRhythm, NotationSystem};
use crate::text::parsing::{Lexer, TextSpan};
use crate::text::primitives::RootNotation;
use crate::text::sections::SectionType;
use crate::text::time::{AbsolutePosition, TimeSignature};

impl<'a> ChartParser<'a> {
    /// Parse a single chord token
    ///
    /// # Arguments
    /// * `token` - The chord token string (e.g., "Cmaj7", "Am7/G")
    /// * `section_type` - Current section type for chord memory
    /// * `time_sig` - Current time signature for duration calculation
    /// * `source_span` - Optional source text span for linking back to input
    /// * `system` - Notation-system hint for resolving an ambiguous `b<digit>`
    pub(in crate::text::chart::parser) fn parse_chord_token(
        &mut self,
        token: &str,
        section_type: &SectionType,
        time_sig: TimeSignature,
        source_span: Option<TextSpan>,
        system: NotationSystem,
    ) -> Result<ChordInstance, String> {
        use crate::text::chord::Chord;
        use keyflow_proto::chart::commands::Command;

        // Check for accent prefix (>) BEFORE push - indicates accent on the pushed beat
        // Supports: >'C (accent on the anticipation beat 4.66)
        let (accent_before_push, token_after_leading_accent) = if token.starts_with('>') {
            (true, token.strip_prefix('>').unwrap_or(token))
        } else {
            (false, token)
        };

        // Check for staccato prefix (.) AFTER accent, BEFORE push
        // Supports: .C (staccato chord), >.'C (accented staccato push), .'C (staccato push)
        let (is_staccato, token_after_staccato) = if token_after_leading_accent.starts_with('.') {
            (
                true,
                token_after_leading_accent
                    .strip_prefix('.')
                    .unwrap_or(token_after_leading_accent),
            )
        } else {
            (false, token_after_leading_accent)
        };

        // Check for one-time override (prefix !)
        let (is_override, token_clean) = if token_after_staccato.starts_with('!') {
            (
                true,
                token_after_staccato
                    .strip_prefix('!')
                    .unwrap_or(token_after_staccato),
            )
        } else {
            (false, token_after_staccato)
        };

        // Check for push notation (leading apostrophes with optional triplet/tuplet: 'C, ''Em, 'tC, ':5C)
        let (push_modifier, token_after_push) = Self::extract_leading_push_modifiers(token_clean);

        // Check for accent AFTER push prefix - indicates accent on the downbeat
        // Supports: '>C (accent on beat 1, the downbeat)
        let (accent_after_push, token_after_post_accent) = if token_after_push.starts_with('>') {
            (
                true,
                token_after_push
                    .strip_prefix('>')
                    .unwrap_or(token_after_push),
            )
        } else {
            (false, token_after_push)
        };

        // Check for accent shorthand (->) - legacy syntax, also supported (treated as regular accent)
        let (has_accent_inline, token_no_accent) = if token_after_post_accent.contains("->") {
            (true, token_after_post_accent.replace("->", ""))
        } else {
            (false, token_after_post_accent.to_string())
        };
        let token_after_post_accent = token_no_accent.as_str();

        // Check for pull notation (trailing apostrophes with optional triplet/tuplet: C', Em'', C't, C':5)
        let (token_after_pull, pull_modifier) =
            Self::extract_trailing_pull_modifiers(token_after_post_accent);

        // Check for slash chord (e.g., "1/3", "Cmaj7/E", "g/b")
        // But NOT rhythm slashes (e.g., "g//", "C///")
        let (chord_part, bass_part) = Self::split_slash_chord(&token_after_pull);

        // Normalize case for chord parsing - capitalize first letter if it's a
        // note name. This allows "cmaj7" to be parsed as "Cmaj7". A leading
        // ASCII `b` is a flat, though: keep it lowercase when it precedes a
        // Roman numeral (`bIII`) or a digit (`b7` = ♭7), instead of turning
        // it into note B.
        //
        // This does not depend on the surrounding notation system. It used
        // to, and `C b3 F G` engraved as `C B F G` — a letters-majority
        // chart resolved to `Letter`, and `b3` was normalized into the note
        // B, silently. Lowercase `b` is not a note letter anywhere in this
        // parser (`classify_token_system` lists `a c d e f g` and pointedly
        // omits it), so `b` before a digit has only one reading. A B chord
        // is written `B`.
        let second = chord_part.as_bytes().get(1).copied();
        let next_is_roman = matches!(second, Some(b'I' | b'V' | b'i' | b'v'));
        let next_is_digit = second.is_some_and(|c| c.is_ascii_digit());
        let keep_lowercase_flat = chord_part.starts_with('b') && (next_is_roman || next_is_digit);
        let normalized_token = if keep_lowercase_flat {
            chord_part.to_string()
        } else {
            Self::normalize_chord_case(&chord_part)
        };

        // Parse the chord using the Chord parser
        let mut lexer = Lexer::new(normalized_token.clone());
        let tokens = lexer.tokenize();

        // `b3` is a flatted degree even in a chart that is otherwise all
        // letters, so the token is parsed as a degree regardless of the
        // scope's resolved system. Keeping the lowercase `b` is not enough
        // on its own — the chord parser re-reads it as note B when handed
        // `Letter`.
        let token_system = if keep_lowercase_flat && next_is_digit {
            NotationSystem::Degree
        } else {
            system
        };

        let mut chord = Chord::parse_with_system(&tokens, token_system)
            .map_err(|e| format!("Failed to parse chord '{}': {:?}", chord_part, e))?;

        // The `/target` of a secondary chord, kept so the rendered symbol
        // still reads `V/V` after the target is folded into the root.
        let mut applied_suffix: Option<String> = None;

        // Add bass note if this is a slash chord
        if let Some(bass_str) = bass_part {
            let bass_normalized = Self::normalize_chord_case(bass_str);
            let bass_notation = RootNotation::from_string(&bass_normalized)
                .ok_or_else(|| format!("Invalid bass note: {}", bass_str))?;
            // A Roman numeral over a Roman numeral is a secondary chord
            // (`V/V` = "five of five"), not a slash bass — the same rule
            // `Chord::parse` applies. This path split the slash itself
            // before ever reaching that logic, so `V/V` arrived here as a
            // `V` triad with a `V` in the bass and resolved to `G/V`.
            if chord.root.is_roman() && bass_notation.is_roman() {
                applied_suffix = Some(bass_notation.to_string());
                chord.root = chord.root.clone().with_applied_target(bass_notation);
            } else {
                chord.bass = Some(bass_notation);
            }
        }

        // Extract the root from the ORIGINAL token (before normalization, but after apostrophes)
        // This preserves the original casing/format (lowercase note, scale degree, Roman numeral)
        // For scale degrees with quality (e.g., "2maj"), we need to extract just the number part
        // But we pass the full chord_part to process_chord so it can detect explicit quality
        let root_from_token = Self::extract_root_from_token(&chord_part);
        let chord_part_without_duration = chord_part
            .split_once('_')
            .map_or(chord_part.as_str(), |(base, _)| base);

        // For slash chords with just a root (no explicit quality), don't recall from memory.
        // Writing "F/C" means "F major over C bass", not "whatever F I used before with C bass".
        let is_slash_chord_with_just_root =
            chord.bass.is_some() && chord_part_without_duration.len() <= root_from_token.len() + 1; // +1 for possible accidental

        // Use ChordMemory to process this chord and get the appropriate full symbol
        // Pass chord_part (which includes quality like "2maj") so it can detect explicit quality
        let current_key = self.current_key.clone();
        // Off unless the chart asks for it (`\chord_memory = true`): a chord
        // is what is written. Read here, so a setting anywhere above applies.
        let memory_on = self.settings.chord_memory();
        self.chord_memory.set_enabled(memory_on);
        let mut full_symbol = if is_slash_chord_with_just_root {
            // Slash chord with just root - use the normalized symbol, don't recall from memory
            chord.normalized.clone()
        } else {
            self.chord_memory.process_chord(
                &root_from_token,
                &chord_part, // Use chord_part (after apostrophes stripped) - includes "2maj" not just "2"
                &chord.normalized,
                section_type,
                is_override,
                current_key.as_ref(),
            )
        };

        // Append bass note to full_symbol for slash chords (e.g., "F" + "/C" -> "F/C")
        if let Some(bass) = &chord.bass {
            full_symbol = format!("{}/{}", full_symbol, bass);
        } else if let Some(target) = &applied_suffix {
            full_symbol = format!("{}/{}", full_symbol, target);
        }

        // Give a bare number-system chord its key-implied (diatonic) quality:
        // `2` in C major is ii (minor), `7` is vii° (diminished). The terse
        // display (full_symbol) is unchanged — only the chord's actual quality
        // and intervals are set, so a seventh/extension stacks correctly
        // (`2:7` → ii m7). Skipped under a `!` literal override, when an
        // explicit quality is written (`2m`, `2M`, `2maj`, `2dim`…), for Roman
        // numerals (case already carries quality), and chromatic degrees.
        let infer_diatonic = !is_override
            && chord.quality == ChordQuality::Major
            && !Self::token_specifies_major(&chord_part, &root_from_token);
        if infer_diatonic {
            if let Some((key, degree)) =
                current_key.as_ref().zip(chord.root.diatonic_scale_degree())
            {
                if let Some(dq) = key.diatonic_quality(degree) {
                    if dq != ChordQuality::Major {
                        chord.set_triad_quality(dq);
                    }
                }
            }
        }

        // Get rhythm and duration (push/pull will be applied later)
        let rhythm = chord.duration.clone().unwrap_or(ChordRhythm::Default);
        let duration = rhythm.to_duration(time_sig);

        // Store push/pull info for later adjustment
        // Use settings.push_mode as the default base, but explicit 't' or ':N' modifiers override it
        let default_push_mode = self.settings.push_mode;
        let push_pull_info = if push_modifier.is_present() {
            push_modifier
                .to_amount(default_push_mode)
                .map(|amt| (true, amt))
        } else if pull_modifier.is_present() {
            pull_modifier
                .to_amount(default_push_mode)
                .map(|amt| (false, amt))
        } else {
            None
        };

        // Create a RootNotation from the original token to preserve casing
        // (e.g., "vi" stays "vi", not "Vi")
        // Fall back to the parsed chord's root if we can't parse the original
        let root_notation =
            RootNotation::from_string(&root_from_token).unwrap_or_else(|| chord.root.clone());

        // Store original token without auto-assigned duration suffix
        // Auto-duration adds _1, _2, _4, _8, _16 to tokens, but we want to preserve
        // whether the USER explicitly specified a duration
        let original_token = Self::strip_auto_duration_suffix(token);

        // Create chord instance
        let mut instance = ChordInstance::new(
            root_notation,
            full_symbol,
            chord,
            rhythm,
            original_token,
            duration,
            AbsolutePosition::at_beginning(), // Will be calculated in post-processing
        )
        .with_push_pull(push_pull_info);

        // Add source span for click-to-highlight and editing
        if let Some(span) = source_span {
            instance = instance.with_source_span(span);
        }

        // Add accent command if present
        // The order of > and ' determines which beat gets the accent:
        // - >'C = accent before push = AccentOnPush (accent on the anticipation beat)
        // - '>C = accent after push = Accent (accent on the downbeat)
        let has_push = push_modifier.is_present();
        if accent_before_push && has_push {
            // Accent comes before push marker (>'C) - accent goes on the pushed/anticipation beat
            instance = instance.add_command(Command::AccentOnPush);
        } else if accent_before_push || accent_after_push || has_accent_inline {
            // Regular accent - on the downbeat (or no push involved)
            instance = instance.add_command(Command::Accent);
        }

        // Add staccato command if present (.C, >.'C, .'C)
        if is_staccato {
            instance = instance.add_command(Command::Staccato);
        }

        Ok(instance)
    }
}
