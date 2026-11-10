//! Chart directives and song metadata: `\repeat_chord_symbols`, its scope
//! inside a section, and the transcriber credit's place in the metadata.

use keyflow_proto::SongMetadata;
use keyflow_proto::chart::settings::{ChartSettings, RepeatChordSymbols};

/// Every spelling the directive accepts, including the old `auto` and the
/// booleans, lands on one of the three modes; anything else is an error
/// that names the valid values.
#[test]
fn repeat_chord_symbols_values() {
    for (value, mode) in [
        ("changes", RepeatChordSymbols::Changes),
        ("auto", RepeatChordSymbols::Changes),
        ("off", RepeatChordSymbols::Changes),
        ("clusters", RepeatChordSymbols::Clusters),
        ("Cluster", RepeatChordSymbols::Clusters),
        ("always", RepeatChordSymbols::Always),
        ("TRUE", RepeatChordSymbols::Always),
    ] {
        let mut settings = ChartSettings::new();
        settings
            .parse_setting_line(&format!("\\repeat_chord_symbols = {value}"))
            .unwrap_or_else(|e| panic!("{value}: {e}"));
        assert_eq!(settings.repeat_chord_symbols(), mode, "{value}");
        // What is stored reads back as the same mode.
        assert_eq!(RepeatChordSymbols::parse(mode.as_str()), Some(mode));
    }

    let err = ChartSettings::new()
        .parse_setting_line("\\repeat_chord_symbols = sometimes")
        .unwrap_err();
    assert!(err.contains("clusters"), "{err}");
}

/// A directive set inside a section reverts when the section ends.
#[test]
fn section_scoped_directive_reverts() {
    let mut settings = ChartSettings::new();
    settings
        .parse_setting_line("\\repeat_chord_symbols = clusters")
        .unwrap();
    let outer = settings.checkpoint();
    settings
        .parse_setting_line("\\repeat_chord_symbols = always")
        .unwrap();
    assert!(settings.repeat_chord_symbols_always());
    settings.restore(outer);
    assert_eq!(
        settings.repeat_chord_symbols(),
        RepeatChordSymbols::Clusters
    );
}

#[test]
fn defaults_for_a_new_chart() {
    let settings = ChartSettings::new();
    assert_eq!(settings.repeat_chord_symbols(), RepeatChordSymbols::Changes);
    assert!(settings.auto_rhythm_slashes());
    assert!(settings.push_alters_rhythm());
    assert!(!settings.chord_memory());
    assert!(!settings.smart_repeats());
    assert!(
        ChartSettings::new()
            .parse_setting_line("\\no_such_thing = 1")
            .is_err()
    );
}

/// The header line splits into title, subtitle and artist; the transcriber
/// is its own field and never borrows the subtitle.
#[test]
fn title_line_and_transcriber_credit() {
    let (title, artist, subtitle) = SongMetadata::parse_title_artist_subtitle(
        "Amazing Grace (My Chains Are Gone) - Chris Tomlin",
    );
    assert_eq!(title.as_deref(), Some("Amazing Grace"));
    assert_eq!(subtitle.as_deref(), Some("My Chains Are Gone"));
    assert_eq!(artist.as_deref(), Some("Chris Tomlin"));

    // A hyphenated title is not an artist split.
    let (title, artist, _) = SongMetadata::parse_title_artist_subtitle("Re-Run");
    assert_eq!(title.as_deref(), Some("Re-Run"));
    assert_eq!(artist, None);

    // No credit unless the chart gives one.
    assert_eq!(SongMetadata::new().transcriber, None);
}
