use super::*;

#[test]
fn test_duration_push_parsing_ambiguous_cases() {
    // Test '_44 - scale degree 4 pushed by quarter note
    let (modifier, remaining) = Chart::extract_leading_push_modifiers("'_44");
    assert!(modifier.duration.is_some());
    assert_eq!(modifier.duration.unwrap(), LilySyntax::Quarter);
    assert_eq!(remaining, "4");

    // Test '_84 - scale degree 4 pushed by eighth note
    let (modifier, remaining) = Chart::extract_leading_push_modifiers("'_84");
    assert!(modifier.duration.is_some());
    assert_eq!(modifier.duration.unwrap(), LilySyntax::Eighth);
    assert_eq!(remaining, "4");

    // Test '_164 - scale degree 4 pushed by sixteenth note
    let (modifier, remaining) = Chart::extract_leading_push_modifiers("'_164");
    assert!(modifier.duration.is_some());
    assert_eq!(modifier.duration.unwrap(), LilySyntax::Sixteenth);
    assert_eq!(remaining, "4");

    // Test '_324 - scale degree 4 pushed by thirty-second note
    let (modifier, remaining) = Chart::extract_leading_push_modifiers("'_324");
    assert!(modifier.duration.is_some());
    assert_eq!(modifier.duration.unwrap(), LilySyntax::ThirtySecond);
    assert_eq!(remaining, "4");

    // Test '_4C - C pushed by quarter note (non-ambiguous)
    let (modifier, remaining) = Chart::extract_leading_push_modifiers("'_4C");
    assert!(modifier.duration.is_some());
    assert_eq!(modifier.duration.unwrap(), LilySyntax::Quarter);
    assert_eq!(remaining, "C");

    // Test '_8tC - C pushed by triplet eighth
    let (modifier, remaining) = Chart::extract_leading_push_modifiers("'_8tC");
    assert!(modifier.duration.is_some());
    assert_eq!(modifier.duration.unwrap(), LilySyntax::Eighth);
    assert!(modifier.duration_triplet);
    assert_eq!(remaining, "C");

    // Test '_4.C - C pushed by dotted quarter
    let (modifier, remaining) = Chart::extract_leading_push_modifiers("'_4.C");
    assert!(modifier.duration.is_some());
    assert_eq!(modifier.duration.unwrap(), LilySyntax::Quarter);
    assert!(modifier.duration_dotted);
    assert_eq!(remaining, "C");
}

#[test]
fn test_duration_pull_parsing_ambiguous_cases() {
    // Test 4'_4 - scale degree 4 pulled by quarter note
    let (chord, modifier) = Chart::extract_trailing_pull_modifiers("4'_4");
    assert!(modifier.duration.is_some());
    assert_eq!(modifier.duration.unwrap(), LilySyntax::Quarter);
    assert_eq!(chord, "4");

    // Test C'_8 - C pulled by eighth note
    let (chord, modifier) = Chart::extract_trailing_pull_modifiers("C'_8");
    assert!(modifier.duration.is_some());
    assert_eq!(modifier.duration.unwrap(), LilySyntax::Eighth);
    assert_eq!(chord, "C");

    // Test C'_8t - C pulled by triplet eighth
    let (chord, modifier) = Chart::extract_trailing_pull_modifiers("C'_8t");
    assert!(modifier.duration.is_some());
    assert_eq!(modifier.duration.unwrap(), LilySyntax::Eighth);
    assert!(modifier.duration_triplet);
    assert_eq!(chord, "C");

    // Test C'_4. - C pulled by dotted quarter
    let (chord, modifier) = Chart::extract_trailing_pull_modifiers("C'_4.");
    assert!(modifier.duration.is_some());
    assert_eq!(modifier.duration.unwrap(), LilySyntax::Quarter);
    assert!(modifier.duration_dotted);
    assert_eq!(chord, "C");
}

#[test]
fn test_accent_prefix_parsing() {
    use crate::text::sections::SectionType;
    use keyflow_proto::chart::commands::Command;

    // Test that >C parses as C with an accent command
    let input = r#"
Accent Test
120bpm 4/4 #C

VS
>C
"#;
    let chart = parse_chart(input).expect("Should parse");
    let measures: Vec<_> = chart
        .sections
        .iter()
        .filter(|s| {
            !s.section.section_type.is_compact() && s.section.section_type != SectionType::End
        })
        .flat_map(|s| s.measures())
        .collect();

    assert!(!measures.is_empty(), "Should have at least one measure");
    let chords: Vec<_> = measures[0]
        .chords
        .iter()
        .filter(|c| c.full_symbol != "s")
        .collect();
    assert!(
        !chords.is_empty(),
        "Should have at least one non-space chord"
    );

    let chord = &chords[0];
    assert_eq!(chord.full_symbol, "C", "Chord symbol should be C");
    assert!(
        chord.commands.iter().any(|c| matches!(c, Command::Accent)),
        "Chord should have accent command"
    );
}

#[test]
fn test_accent_on_push_and_downbeat_parsing() {
    use crate::text::sections::SectionType;
    use keyflow_proto::chart::commands::Command;

    // Test that >'C parses as AccentOnPush (accent on anticipation)
    // and '>C parses as Accent (accent on downbeat)
    // Based on real-world chart: Thriller - Dirty Loops
    let input = r#"
Thriller - Dirty Loops, Cory Wong
Transcribed By: Cody Wright
120bpm 4/4 #Ab
\push = triplet

COUNT 2

IN
r8t >Ab9_8t r8t r8t r8t >F9_8t r2 | s1

VS
>'F/C . Cm . 'F/C . Cm . 'F/C . Cm . 'F/C . Cm Cm9

CH
>Cm/Eb / 'Eb /// | 'Eb / 'F/C / 'Cm // | 'F/A //// | 'Fm9  ////
>Cm/Eb / 'Eb /// | 'Eb / 'F/C / 'Cm // | 'F/A | r8t >Ab9_8t r8t r8t >'F9_8t r8t r4 Fm/Ab_4 | s1

BR
'_4F7 | . |  Abmaj9 //// | // r8t Abmaj9_8t r8t Bb_8t r8t Cm7_8t | Cm7 | Ebmaj7/Bb | Am7b5 | Abmaj7 | G7sus4 | 'G7

VS
'F/C . Cm . 'F/C . Cm . 'F/C . Cm . 'F/C . Cm Cm9
"#;
    let chart = parse_chart(input).expect("Should parse");

    // Find VS section (first one after COUNT and IN)
    let verse_sections: Vec<_> = chart
        .sections
        .iter()
        .filter(|s| s.section.section_type == SectionType::Verse)
        .collect();
    assert!(!verse_sections.is_empty(), "Should have VS sections");

    // First VS measure should have >'F/C which is AccentOnPush
    let vs_measures = verse_sections[0].measures();
    let vs_chords: Vec<_> = vs_measures[0]
        .chords
        .iter()
        .filter(|c| c.full_symbol != "s")
        .collect();
    assert!(!vs_chords.is_empty(), "VS should have non-space chords");

    let vs_chord = vs_chords[0];
    assert_eq!(vs_chord.full_symbol, "F/C", "VS chord should be F/C");
    assert!(
        vs_chord
            .commands
            .iter()
            .any(|c| matches!(c, Command::AccentOnPush)),
        ">'F/C should have AccentOnPush command (accent on anticipation)"
    );
    assert!(
        vs_chord.push_pull.is_some(),
        "VS chord should have push_pull"
    );

    // Find CH section to test both accent types
    let chorus_sections: Vec<_> = chart
        .sections
        .iter()
        .filter(|s| s.section.section_type == SectionType::Chorus)
        .collect();
    assert!(!chorus_sections.is_empty(), "Should have CH sections");

    // CH measure 7 (second-to-last) has >'F9 which is AccentOnPush
    // Input: r8t >Ab9_8t r8t r8t >'F9_8t r8t r4 Fm/Ab_4
    let ch_measures = chorus_sections[0].measures();
    assert!(ch_measures.len() >= 8, "CH should have at least 8 measures");

    let ch7_chords: Vec<_> = ch_measures[7]
        .chords
        .iter()
        .filter(|c| c.full_symbol != "s" && c.full_symbol != "r")
        .collect();

    // Should have Ab9, F9, and Fm/Ab
    assert!(
        ch7_chords.len() >= 2,
        "CH measure 7 should have at least 2 chords"
    );

    // Ab9 has regular accent (>Ab9)
    let ab9_chord = ch7_chords.iter().find(|c| c.full_symbol == "Ab9");
    assert!(ab9_chord.is_some(), "Should have Ab9 chord");
    let ab9 = ab9_chord.unwrap();
    assert!(
        ab9.commands.iter().any(|c| matches!(c, Command::Accent)),
        ">Ab9 should have regular Accent (no push)"
    );

    // F9 has AccentOnPush (>'F9 with push)
    let f9_chord = ch7_chords.iter().find(|c| c.full_symbol == "F9");
    assert!(f9_chord.is_some(), "Should have F9 chord");
    let f9 = f9_chord.unwrap();
    assert!(
        f9.commands
            .iter()
            .any(|c| matches!(c, Command::AccentOnPush)),
        ">'F9 should have AccentOnPush (accent before push marker)"
    );
    assert!(f9.push_pull.is_some(), "F9 should have push_pull");
}

#[test]
fn test_accent_not_in_chord_memory() {
    use crate::text::sections::SectionType;
    use keyflow_proto::chart::commands::Command;

    // With the new chord memory behavior:
    // - Basic chords (C, Cm) don't participate in memory at all
    // - Writing "C" gives "C" (basic major), NOT a recalled Cmaj7
    // This test verifies that accented chords don't bleed accents into basic chords
    let input = r#"
Accent Memory Test
120bpm 4/4 #C
\chord_memory = true

VS
>Cmaj7 | C D E F
"#;
    let chart = parse_chart(input).expect("Should parse");

    // Get all verse sections
    let verse_sections: Vec<_> = chart
        .sections
        .iter()
        .filter(|s| s.section.section_type == SectionType::Verse)
        .collect();

    assert!(
        !verse_sections.is_empty(),
        "Should have at least 1 verse section"
    );

    // VS measure 0: >Cmaj7 should have accent
    let vs_measures = verse_sections[0].measures();
    assert!(vs_measures.len() >= 2, "VS should have at least 2 measures");

    let m0_chords: Vec<_> = vs_measures[0]
        .chords
        .iter()
        .filter(|c| c.full_symbol != "s")
        .collect();
    assert!(!m0_chords.is_empty(), "M0 should have non-space chords");

    // First chord: >Cmaj7 should have accent
    let first_chord = m0_chords[0];
    assert_eq!(first_chord.full_symbol, "Cmaj7");
    assert!(
        first_chord
            .commands
            .iter()
            .any(|c| matches!(c, Command::Accent)),
        "First chord should have accent"
    );

    // VS measure 1: C (basic major) should recall Cmaj7 from major family memory
    // Basic chords CAN recall but don't store - split memory by chord family
    let m1_chords: Vec<_> = vs_measures[1]
        .chords
        .iter()
        .filter(|c| c.full_symbol != "s")
        .collect();
    assert!(!m1_chords.is_empty(), "M1 should have non-space chords");

    let second_chord = m1_chords[0];
    // Basic chords DO recall from their family's memory
    assert_eq!(
        second_chord.full_symbol, "Cmaj7",
        "Basic C should recall Cmaj7 from major family memory"
    );
    // But the accent is NOT recalled (it's a modifier on the chord instance, not stored in memory)
    assert!(
        !second_chord
            .commands
            .iter()
            .any(|c| matches!(c, Command::Accent)),
        "Recalled chord should NOT have accent from memory. Commands: {:?}",
        second_chord.commands
    );
}

#[test]
fn test_accent_preserved_in_section_template() {
    use crate::text::sections::SectionType;
    use keyflow_proto::chart::commands::Command;

    // Accents CAN be committed to section memory (templates).
    // If we define VS with >C, then recall VS, the accent should be preserved.
    let input = r#"
Accent Template Test
120bpm 4/4 #C

VS
>C D E F

VS
"#;
    let chart = parse_chart(input).expect("Should parse");
    let verse_sections: Vec<_> = chart
        .sections
        .iter()
        .filter(|s| s.section.section_type == SectionType::Verse)
        .collect();

    assert_eq!(verse_sections.len(), 2, "Should have 2 verse sections");

    // VS1 (template definition): >C should have accent
    let vs1_chords: Vec<_> = verse_sections[0].measures()[0]
        .chords
        .iter()
        .filter(|c| c.full_symbol != "s")
        .collect();
    assert!(!vs1_chords.is_empty());
    assert!(
        vs1_chords[0]
            .commands
            .iter()
            .any(|c| matches!(c, Command::Accent)),
        "VS1 first chord should have accent"
    );

    // VS2 (recalled from template): should also have accent on first chord
    let vs2_chords: Vec<_> = verse_sections[1].measures()[0]
        .chords
        .iter()
        .filter(|c| c.full_symbol != "s")
        .collect();
    assert!(!vs2_chords.is_empty());
    assert!(
        vs2_chords[0]
            .commands
            .iter()
            .any(|c| matches!(c, Command::Accent)),
        "VS2 recalled chord should have accent - templates preserve commands"
    );
}

#[test]
fn test_push_duration_then_accent() {
    use keyflow_proto::chart::commands::Command;

    // '_4>F7 = push with quarter duration, then accent (accent on downbeat)
    // >'_4F7 = accent before push (accent on pushed beat)
    let input = r#"
Test
120bpm 4/4

BR
'_4>F7 | >'_4G7 |
"#;
    let chart = parse_chart(input).expect("Should parse");

    // Find BR section
    let br_sections: Vec<_> = chart
        .sections
        .iter()
        .filter(|s| s.section.section_type == crate::text::sections::SectionType::Bridge)
        .collect();
    assert!(!br_sections.is_empty(), "Should have BR section");

    let measures = br_sections[0].measures();

    // Measure 0: '_4>F7 - accent AFTER push = regular Accent (downbeat)
    let m0_chords: Vec<_> = measures[0]
        .chords
        .iter()
        .filter(|c| c.full_symbol != "s" && c.full_symbol != "r")
        .collect();
    assert!(!m0_chords.is_empty(), "Measure 0 should have chords");
    let f7 = &m0_chords[0];
    assert_eq!(f7.full_symbol, "F7", "Should be F7");
    assert!(f7.push_pull.is_some(), "F7 should have push");
    assert!(
        f7.commands.iter().any(|c| matches!(c, Command::Accent)),
        "'_4>F7 should have Accent (accent after push = downbeat). Got: {:?}",
        f7.commands
    );
    assert!(
        !f7.commands
            .iter()
            .any(|c| matches!(c, Command::AccentOnPush)),
        "'_4>F7 should NOT have AccentOnPush. Got: {:?}",
        f7.commands
    );

    // Measure 1: >'_4G7 - accent BEFORE push = AccentOnPush (pushed beat)
    let m1_chords: Vec<_> = measures[1]
        .chords
        .iter()
        .filter(|c| c.full_symbol != "s" && c.full_symbol != "r")
        .collect();
    assert!(!m1_chords.is_empty(), "Measure 1 should have chords");
    let g7 = &m1_chords[0];
    assert_eq!(g7.full_symbol, "G7", "Should be G7");
    assert!(g7.push_pull.is_some(), "G7 should have push");
    assert!(
        g7.commands
            .iter()
            .any(|c| matches!(c, Command::AccentOnPush)),
        ">'_4G7 should have AccentOnPush (accent before push = pushed beat). Got: {:?}",
        g7.commands
    );
    assert!(
        !g7.commands.iter().any(|c| matches!(c, Command::Accent)),
        ">'_4G7 should NOT have regular Accent. Got: {:?}",
        g7.commands
    );
}
