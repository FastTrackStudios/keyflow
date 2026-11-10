use super::*;

#[test]
fn caret_inversion_resolves_to_a_real_inverted_chord() {
    let c_major = Key::parse("C").unwrap();
    let chord = |s: &str| {
        // One chord per line; resolve against C major to check the bass.
        let m = parse_line(s);
        m[0].chords[0].clone()
    };

    // `V^65` shows as V^65 but resolves to a real first-inversion dominant
    // 7th: a seventh chord with the 3rd (B in C) in the bass.
    let v65 = chord("V^65");
    assert_eq!(v65.full_symbol, "V^65");
    assert!(v65.parsed.family.is_some(), "^65 implies a seventh chord");
    let bass = v65.parsed.bass.as_ref().expect("inversion sets a bass");
    assert_eq!(bass.resolve(Some(&c_major)).unwrap().name, "B");

    // `V^6` is a first-inversion *triad* (no seventh), 3rd in the bass.
    let v6 = chord("V^6");
    assert_eq!(v6.full_symbol, "V^6");
    assert!(v6.parsed.family.is_none(), "^6 is a triad");
    assert_eq!(
        v6.parsed
            .bass
            .as_ref()
            .unwrap()
            .resolve(Some(&c_major))
            .unwrap()
            .name,
        "B"
    );

    // `V^43` → 7th, 5th (D) in the bass; `V^42` → 7th, 7th (F) in the bass.
    assert_eq!(
        chord("V^43")
            .parsed
            .bass
            .as_ref()
            .unwrap()
            .resolve(Some(&c_major))
            .unwrap()
            .name,
        "D"
    );
    assert_eq!(
        chord("V^42")
            .parsed
            .bass
            .as_ref()
            .unwrap()
            .resolve(Some(&c_major))
            .unwrap()
            .name,
        "F"
    );

    // A trailing duration is preserved on the chord.
    let with_dur = parse_line("V^65_2 V");
    assert_eq!(with_dur[0].chords[0].full_symbol, "V^65");
    assert_eq!(
        with_dur[0].chords[0]
            .duration
            .to_beats(TimeSignature::common_time()),
        2.0
    );

    // A suspension figure (`^4-3`) is not an inversion — it stays a figure.
    assert_eq!(parse_line("V^4-3")[0].figured_bass[0].rows[0].text, "4-3");

    // Without the caret, `V6` is still an ordinary added-6th chord.
    let plain = parse_line("V6");
    assert!(plain[0].figured_bass.is_empty());
    assert!(plain[0].chords[0].parsed.bass.as_ref().is_none());
    assert_eq!(plain[0].chords[0].full_symbol, "V6");
}

#[test]
fn caret_inversion_spells_chromatic_bass_and_ignores_chord_memory() {
    let c_major = Key::parse("C").unwrap();
    // A key is needed so the bass is spelled from the chord's real tones.
    let chart = parse_chart("T\n4/4 #C\n\nvs 4\nI III^6 V^65 V^6\n").unwrap();
    let m = chart.sections[0].measures();

    // `III` is a major chord (E–G♯–B); its 3rd is G♯, so `III^6` puts a
    // sharpened scale degree in the bass, not the diatonic G.
    let iii6 = m[1].chords[0].parsed.bass.as_ref().unwrap();
    assert_eq!(iii6.resolve(Some(&c_major)).unwrap().name, "G#");

    // `V^6` right after `V^65` stays a triad — the remembered seventh from
    // `V^65` does not carry into it.
    assert!(
        m[2].chords[0].parsed.family.is_some(),
        "V^65 is a seventh chord"
    );
    assert!(
        m[3].chords[0].parsed.family.is_none(),
        "V^6 must stay a triad despite the preceding V^65"
    );
    assert_eq!(
        m[3].chords[0]
            .parsed
            .bass
            .as_ref()
            .unwrap()
            .resolve(Some(&c_major))
            .unwrap()
            .name,
        "B"
    );
}

#[test]
fn chord_token_can_attach_figured_bass_rows() {
    let measures = parse_line("| A\"#4-3/2-1\" /// |");

    assert_eq!(measures.len(), 1);
    assert_eq!(measures[0].chords[0].full_symbol, "A");
    assert_eq!(measures[0].figured_bass.len(), 1);

    let figured_bass = &measures[0].figured_bass[0];
    assert_eq!(figured_bass.beat, 1);
    assert_eq!(figured_bass.placement, Placement::Above);
    assert_eq!(figured_bass.rows.len(), 2);
    assert_eq!(figured_bass.rows[0].accidental, "#");
    assert_eq!(figured_bass.rows[0].text, "4-3");
    assert_eq!(figured_bass.rows[1].accidental, "");
    assert_eq!(figured_bass.rows[1].text, "2-1");
}

#[test]
fn chord_attached_quote_can_be_figured_bass() {
    let measures = parse_line("| A\"#4-3 2-1\" /// |");

    assert_eq!(measures.len(), 1);
    assert_eq!(measures[0].chords[0].full_symbol, "A");
    assert_eq!(measures[0].figured_bass.len(), 1);

    let figured_bass = &measures[0].figured_bass[0];
    assert_eq!(figured_bass.rows[0].accidental, "#");
    assert_eq!(figured_bass.rows[0].text, "4-3");
    assert_eq!(figured_bass.rows[1].accidental, "");
    assert_eq!(figured_bass.rows[1].text, "2-1");
}

#[test]
fn floating_slash_bass_inherits_root_and_displays_verbatim() {
    // `/D` after Bb is harmonically Bb/D but prints "/D".
    let measures = parse_line("Bb /D Eb F");
    assert_eq!(measures.len(), 4);
    let slash = &measures[1].chords[0];
    assert_eq!(slash.full_symbol, "Bb/D");
    assert_eq!(slash.display_override.as_deref(), Some("/D"));
    assert_eq!(
        slash.parsed.bass.as_ref().map(|b| format!("{b}")),
        Some("D".to_string())
    );
}

#[test]
fn floating_slash_bass_drops_prior_bass() {
    // `F/Eb` then `/D` inherits the root F, not the bass: F/D.
    let measures = parse_line("F/Eb /D");
    assert_eq!(measures[1].chords[0].full_symbol, "F/D");
    assert_eq!(
        measures[1].chords[0].display_override.as_deref(),
        Some("/D")
    );
}

#[test]
fn attached_suspension_figure_on_note_root() {
    // `Eb2` and `F4-3` are a chord plus a suspension figure; the digit is
    // not folded into the chord quality.
    let measures = parse_line("Eb2 F4-3");
    assert_eq!(measures.len(), 2);
    assert_eq!(measures[0].chords[0].full_symbol, "Eb");
    assert_eq!(measures[0].suspensions.len(), 1);
    assert_eq!(measures[0].suspensions[0].figure, "2");
    assert_eq!(measures[1].chords[0].full_symbol, "F");
    assert_eq!(measures[1].suspensions[0].figure, "4-3");
}

#[test]
fn attached_multi_part_and_chord_quality_figures() {
    // Multi-part hyphenated figure keeps every part (`E3-4-3`).
    let m = parse_line("E3-4-3");
    assert_eq!(m[0].chords[0].full_symbol, "E");
    assert_eq!(m[0].suspensions[0].figure, "3-4-3");

    // Real chord qualities are never mistaken for figures.
    for sym in ["C6", "Csus4", "Cm7", "Gm7b5", "C9"] {
        let m = parse_line(sym);
        assert_eq!(m[0].chords[0].full_symbol, sym, "{sym} should stay a chord");
        assert!(m[0].suspensions.is_empty(), "{sym} should have no figure");
    }
}

#[test]
fn bare_scale_degree_is_not_a_suspension_figure() {
    // A lone `4` is a scale-degree chord, never a floating figure.
    let measures = parse_line("4");
    assert_eq!(measures[0].chords[0].full_symbol, "4");
    assert!(measures[0].suspensions.is_empty());
}

#[test]
fn floating_hyphenated_figure_holds_chord_for_full_measure() {
    // `Bb // 4-3 //` is one measure: Bb sustains all four beats with the
    // 4-3 figure mid-bar, not two separate measures.
    let measures = parse_line("Bb // 4-3 // /D");
    assert_eq!(measures.len(), 2);
    assert_eq!(measures[0].chords.len(), 1);
    assert_eq!(measures[0].chords[0].full_symbol, "Bb");
    assert_eq!(
        measures[0].chords[0]
            .duration
            .to_beats(TimeSignature::common_time()),
        4.0
    );
    assert_eq!(measures[0].suspensions.len(), 1);
    assert_eq!(measures[0].suspensions[0].figure, "4-3");
    assert_eq!(measures[1].chords[0].full_symbol, "Bb/D");
}

#[test]
fn bare_chord_then_spaced_figure_opens_a_new_bar() {
    // `F 4-3 ///` opens a NEW bar restating F with the 4-3 figure. The `///`
    // gives that figure 3 beats, so the following chord lands on beat 4 of
    // the SAME bar (not a third bar): bar of F, then bar of [F4-3(3), Eb(1)].
    let measures = parse_line("F 4-3 /// Eb");
    assert_eq!(measures.len(), 2);
    let ts = TimeSignature::common_time();
    assert_eq!(measures[0].chords[0].full_symbol, "F");
    assert!(measures[0].suspensions.is_empty());
    assert_eq!(measures[0].chords[0].duration.to_beats(ts), 4.0);
    // Bar 2: F restated (3 beats, with 4-3) + Eb filling beat 4.
    assert_eq!(measures[1].chords[0].full_symbol, "F");
    assert_eq!(measures[1].chords[0].duration.to_beats(ts), 3.0);
    assert_eq!(measures[1].suspensions[0].figure, "4-3");
    assert_eq!(measures[1].chords[1].full_symbol, "Eb");
    assert_eq!(measures[1].chords[1].duration.to_beats(ts), 1.0);
}
