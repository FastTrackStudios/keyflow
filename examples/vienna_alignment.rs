//! Example: Chord-to-syllable alignment for Vienna
//!
//! Demonstrates how to:
//! 1. Parse a chart with chords and lyrics
//! 2. Align each chord to specific syllables based on timing
//! 3. Enable synced playback, lyric slides, and interactive charts

use keyflow_proto::{
    ChordSyllableAligner, LyricLine, LyricSyllable, SyllableParser, ChordInstance,
    AbsolutePosition, MusicalDuration
};

fn main() {
    println!("=== Vienna Chord-Syllable Alignment Example ===\n");

    // Step 1: Parse lyrics into syllables
    let parser = SyllableParser::new();
    let lyric_text = "[Gm]Slow down you [A#]crazy child";
    let lyric_line = parser.parse(lyric_text);

    println!("📝 Parsed {} syllables:", lyric_line.syllables.len());
    for (i, syl) in lyric_line.syllables.iter().enumerate() {
        println!(
            "  [{}] '{}' {} (word_initial: {})",
            i,
            syl.text,
            if syl.hyphen_after { "- " } else { "" },
            syl.word_initial
        );
    }

    // Step 2: Create chord instances with timing
    let chords = vec![
        ChordInstance {
            root: keyflow_proto::primitives::RootNotation::Note(
                keyflow_proto::primitives::MusicalNote::G,
            ),
            full_symbol: "Gm".to_string(),
            parsed: dummy_chord(),
            rhythm: dummy_rhythm(),
            original_token: "Gm".to_string(),
            duration: MusicalDuration::new(0, 2, 0), // 2 beats
            position: AbsolutePosition::new(0, 0),
            push_pull: None,
            commands: vec![],
            source_span: None,
        },
        ChordInstance {
            root: keyflow_proto::primitives::RootNotation::Note(
                keyflow_proto::primitives::MusicalNote::ASharp,
            ),
            full_symbol: "A#".to_string(),
            parsed: dummy_chord(),
            rhythm: dummy_rhythm(),
            original_token: "A#".to_string(),
            duration: MusicalDuration::new(0, 2, 0),
            position: AbsolutePosition::new(0, 2),
            push_pull: None,
            commands: vec![],
            source_span: None,
        },
    ];

    println!("\n🎵 Created {} chords:", chords.len());
    for (i, chord) in chords.iter().enumerate() {
        println!(
            "  [{}] {} at beat {} (duration: {} beats)",
            i,
            chord.full_symbol,
            chord.position.beat,
            chord.duration.beat
        );
    }

    // Step 3: Align chords to syllables
    println!("\n🔗 Aligning chords to syllables...");
    match ChordSyllableAligner::align(&chords, &lyric_line.syllables) {
        Ok(alignment) => {
            println!("✓ Alignment successful: {} mappings\n", alignment.mappings.len());

            // Display alignment
            for mapping in &alignment.mappings {
                let chord = &chords[mapping.chord_index];
                let syllable = &lyric_line.syllables[mapping.syllable_index];

                println!(
                    "  Syllable '{}' → Chord '{}' ({:?})",
                    syllable.text, chord.full_symbol, mapping.attachment
                );
                println!(
                    "    └─ Duration until next: {} beat(s)",
                    mapping.duration_until_next_chord.beat
                );
            }

            // Demonstrate use cases
            show_use_cases(&alignment, &chords, &lyric_line.syllables);
        }
        Err(e) => {
            println!("✗ Alignment failed: {}", e);
        }
    }
}

fn show_use_cases(
    alignment: &keyflow_proto::SectionAlignment,
    chords: &[ChordInstance],
    syllables: &[LyricSyllable],
) {
    println!("\n📱 Use Cases Enabled by Alignment:");

    // Use case 1: Synced lyrics
    println!("\n1️⃣  SYNCED LYRICS (Karaoke/Playback):");
    println!("   Each syllable knows its timing:");
    for (i, syl) in syllables.iter().enumerate() {
        let mapped_chords = alignment.chords_for_syllable(i);
        if !mapped_chords.is_empty() {
            let mapping = &mapped_chords[0];
            println!(
                "     Time {:.1}s: '{}' (chord: {})",
                (mapping.chord_position.beat as f32) * 0.5,
                syl.text,
                chords[mapping.chord_index].full_symbol
            );
        }
    }

    // Use case 2: Lyric slides
    println!("\n2️⃣  LYRIC SLIDES (Visual Progression):");
    println!("   Show syllable duration until next chord change:");
    for mapping in &alignment.mappings {
        let syllable = &syllables[mapping.syllable_index];
        println!(
            "     '{}' slides for {} beat(s)",
            syllable.text, mapping.duration_until_next_chord.beat
        );
    }

    // Use case 3: MIDI generation
    println!("\n3️⃣  MIDI GENERATION:");
    println!("   Generate note events timed to syllable boundaries:");
    println!("     Time 0.0s: Note C5 (from Gm chord)");
    println!("     Time 1.0s: Note A#4 (from A# chord, second syllable)");
    println!("     Time 2.0s: Note end");

    // Use case 4: Interactive charts
    println!("\n4️⃣  INTERACTIVE CHARTS:");
    println!("   Click syllable → highlight its chord:");
    println!("     Syllable 'Slow' → Highlight 'Gm'");
    println!("     Syllable 'crazy' → Highlight 'A#'");
}

// Dummy implementations for testing
fn dummy_chord() -> keyflow_proto::Chord {
    keyflow_proto::Chord::new(
        keyflow_proto::primitives::MusicalNote::C,
        keyflow_proto::chord::ChordQuality::Major,
    )
}

fn dummy_rhythm() -> keyflow_proto::ChordRhythm {
    keyflow_proto::chord::ChordRhythm::Slashes(4)
}
