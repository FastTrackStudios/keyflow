//! Keyflow text export.
//!
//! This exporter writes idiomatic `.kf` syntax for imported charts.

use keyflow_proto::chart::melody::Melody;
use keyflow_proto::chart::notations::{
    Dynamic, FiguredBass, Hairpin, HairpinKind, Placement, RepeatMark, StaffText,
};
use keyflow_proto::chart::types::{Measure, RhythmElement};
use keyflow_proto::key::ScaleMode;
use keyflow_proto::time::{MusicalDuration, MusicalPositionExt, TimeSignature};
use keyflow_proto::Note;
use keyflow_proto::{Chart, ChordRhythm, SectionType};

#[must_use]
pub fn chart_to_keyflow(chart: &Chart) -> String {
    let mut out = String::new();

    if let Some(title) = chart.metadata.title.as_deref() {
        out.push_str(title);
        // `Title - Artist` is the header form the parser recognizes. Without
        // the artist half, a title made of words that scan as chords ("American
        // Idiot") is read back as a bar line, and the chart grows a section.
        if let Some(artist) = chart.metadata.artist.as_deref() {
            if !artist.trim().is_empty() {
                out.push_str(" - ");
                out.push_str(artist.trim());
            }
        }
        out.push('\n');
    }

    let mut metadata = Vec::new();
    if let Some(tempo) = &chart.tempo {
        metadata.push(format!("8th={}bpm", tempo.bpm as u32));
    }
    if let Some(ts) = chart
        .initial_time_signature
        .as_ref()
        .or(chart.time_signature.as_ref())
    {
        metadata.push(format!("{}/{}", ts.numerator, ts.denominator));
    }
    if let Some(key) = &chart.initial_key {
        metadata.push(key_to_syntax(key));
    }
    if !metadata.is_empty() {
        out.push_str(&metadata.join(" "));
        out.push('\n');
    }

    let mut pending_cross_section_repeat_prefix: Option<Vec<Measure>> = None;
    for section in &chart.sections {
        out.push('\n');
        let measures = expand_measures_without_repeat_symbols(
            section.measures(),
            pending_cross_section_repeat_prefix.as_deref(),
        );
        pending_cross_section_repeat_prefix = cross_section_repeat_prefix(section.measures());
        let section_chord_length = common_chord_length(&measures);
        out.push_str(&section_header(section, measures.len()));
        out.push('\n');
        if let Some(rhythm) = &section_chord_length {
            out.push_str("\\ChordLength ");
            out.push_str(&chord_length_to_syntax(
                rhythm,
                measures
                    .first()
                    .map_or((4, 4), |measure| measure.time_signature),
            ));
            out.push('\n');
        }

        for (row_idx, row) in measures.chunks(4).enumerate() {
            if row_idx > 0 {
                out.push_str("    ");
            }

            // One barline between two measures, whatever decoration it
            // carries. `:|` already closes a bar and opens the next, and a
            // repeat that ends where the next begins is `:|:` — writing
            // `:| |:` puts an empty measure between them.
            let mut pending_close = false;
            let mut pending_passes: Option<usize> = None;
            for measure in row {
                let opens = matches!(measure.start_repeat, RepeatMark::Forward);
                match (pending_close, opens) {
                    (true, _) => {
                        out.push_str(":|");
                        if let Some(passes) = pending_passes.take() {
                            out.push('x');
                            out.push_str(&passes.to_string());
                        }
                        out.push_str(if opens { ": " } else { " " });
                    }
                    (false, true) => out.push_str("|: "),
                    (false, false) => out.push_str("| "),
                }
                out.push_str(&measure_to_keyflow(measure, section_chord_length.as_ref()));
                out.push(' ');
                pending_close = matches!(measure.end_repeat, RepeatMark::Backward);
                // Always written, even the `x2` a bare repeat implies: a
                // reader scanning for how many times should find the same
                // thing in the same place every time, rather than having to
                // notice an absence and know what it means.
                pending_passes = if pending_close {
                    Some(measure.repeat_count.max(2))
                } else {
                    None
                };
            }
            if pending_close {
                out.push_str(":|");
                if let Some(passes) = pending_passes {
                    out.push('x');
                    out.push_str(&passes.to_string());
                }
            } else {
                out.push('|');
            }
            out.push('\n');
        }
    }

    out
}

fn key_to_syntax(key: &keyflow_proto::Key) -> String {
    let root = key.root().name();
    let clean = root.trim_start_matches('#').trim_start_matches('b');
    let prefix = if root.contains('b') { 'b' } else { '#' };
    if key.mode == ScaleMode::aeolian() {
        format!("{prefix}{clean}m")
    } else {
        format!("{prefix}{clean}")
    }
}

fn section_header(section: &keyflow_proto::ChartSection, count: usize) -> String {
    match &section.section.section_type {
        SectionType::CountIn => format!("count {count}"),
        SectionType::Opening => format!("opening {count}"),
        SectionType::Intro => format!("intro {count}"),
        SectionType::Verse => format!("vs {count}"),
        SectionType::Chorus => format!("ch {count}"),
        SectionType::Bridge => format!("br {count}"),
        SectionType::Outro => format!("out {count}"),
        SectionType::Pre(inner) if matches!(inner.as_ref(), SectionType::Chorus) => {
            format!("pre {count}")
        }
        SectionType::Post(inner) if matches!(inner.as_ref(), SectionType::Chorus) => {
            format!("post {count}")
        }
        SectionType::Instrumental => format!("inst {count}"),
        SectionType::Solo => format!("solo {count}"),
        SectionType::Interlude => format!("interlude {count}"),
        SectionType::Vamp => format!("vamp {count}"),
        SectionType::Refrain => format!("refrain {count}"),
        SectionType::Turnaround => format!("turnaround {count}"),
        SectionType::Breakdown => format!("breakdown {count}"),
        SectionType::Hits => format!("hits {count}"),
        // A custom name goes in brackets. Bare, only single well-known words
        // parse back as a section header — `Guitar Solo 4` reads as chords,
        // and every bar under it silently joins the section above.
        SectionType::Custom(name) => format!("[{name}] {count}"),
        _ => format!("section {count}"),
    }
}

/// The measures a section writes out, and the repeat signs it keeps.
///
/// A repeat that opens and closes inside one section stays a repeat: `|: … :|`
/// says what the music does in four bars where writing the passes out takes
/// eight, and the reader can see the shape. The chart modes decide whether it
/// is *drawn* as a sign or spelled out — but only if the text still has it,
/// and folding cannot un-expand.
///
/// A repeat that opens in one section and closes in another is different. It
/// has no single line to sit on, so the pass it implies is written out where
/// it lands, which is what `repeat_prefix` carries in.
fn expand_measures_without_repeat_symbols(
    measures: &[Measure],
    repeat_prefix: Option<&[Measure]>,
) -> Vec<Measure> {
    // Nothing to write out: every repeat here opens and closes in this
    // section, so the signs can stay and say it in half the bars.
    if repeat_prefix.is_none() && repeats_are_self_contained(measures) {
        return measures.to_vec();
    }

    let mut expanded = Vec::new();
    let mut repeat_start = 0usize;

    for (idx, measure) in measures.iter().enumerate() {
        if matches!(measure.start_repeat, RepeatMark::Forward) {
            repeat_start = idx;
        }

        expanded.push(clear_repeat_symbols(measure));

        if matches!(measure.end_repeat, RepeatMark::Backward) {
            let repeat_end = first_ending_start(measures, repeat_start, idx).unwrap_or(idx + 1);
            if let Some(repeat_prefix) = repeat_prefix {
                expanded.extend(repeat_prefix.iter().map(repeat_pass_copy));
            }
            for repeated in &measures[repeat_start..repeat_end] {
                expanded.push(repeat_pass_copy(repeated));
            }
            repeat_start = idx + 1;
        }
    }

    expanded
}

/// Whether every repeat in this section closes inside it.
///
/// One that opens here and closes in the next section has no line to sit on,
/// so it is written out instead — see [`cross_section_repeat_prefix`].
fn repeats_are_self_contained(measures: &[Measure]) -> bool {
    let mut open = false;
    for measure in measures {
        if matches!(measure.start_repeat, RepeatMark::Forward) {
            open = true;
        }
        if matches!(measure.end_repeat, RepeatMark::Backward) {
            if !open {
                return false;
            }
            open = false;
        }
    }
    !open
}

fn cross_section_repeat_prefix(measures: &[Measure]) -> Option<Vec<Measure>> {
    let repeat_start = measures
        .iter()
        .position(|measure| matches!(measure.start_repeat, RepeatMark::Forward))?;
    let has_repeat_end = measures
        .iter()
        .skip(repeat_start)
        .any(|measure| matches!(measure.end_repeat, RepeatMark::Backward));
    (!has_repeat_end).then(|| measures[repeat_start..].to_vec())
}

fn first_ending_start(
    measures: &[Measure],
    repeat_start: usize,
    repeat_end: usize,
) -> Option<usize> {
    measures[repeat_start..=repeat_end]
        .iter()
        .position(|measure| {
            measure
                .volta_start
                .as_ref()
                .map(|volta| volta.numbers.contains(&1))
                .unwrap_or(false)
        })
        .map(|offset| repeat_start + offset)
}

/// A measure with its repeat decoration removed.
///
/// The bars are being written out, so a repeat sign over them would send the
/// reader round a second time.
fn clear_repeat_symbols(measure: &Measure) -> Measure {
    let mut measure = measure.clone();
    measure.start_repeat = RepeatMark::None;
    measure.end_repeat = RepeatMark::None;
    measure.volta_start = None;
    measure
}

/// A measure as it should appear on the *second* time through.
///
/// Everything [`clear_repeat_symbols`] drops, and anything written on the bar
/// as well. A cue is an instruction given once: re-emitting it on the repeat
/// pass printed `"The song starts on the & of 3"` over two different bars of
/// Highway to Hell's intro, which is one more time than the song starts.
fn repeat_pass_copy(measure: &Measure) -> Measure {
    let mut measure = clear_repeat_symbols(measure);
    measure.staff_text.clear();
    measure.text_cues.clear();
    measure.dynamics.clear();
    measure.classical_dynamics.clear();
    measure.hairpins.clear();
    measure
}

fn measure_to_keyflow(measure: &Measure, default_chord_length: Option<&ChordRhythm>) -> String {
    if !measure.melodies.is_empty() {
        let notation = measure_notation_to_keyflow(measure, default_chord_length);
        let melodies = measure
            .melodies
            .iter()
            .map(melody_to_syntax)
            .collect::<Vec<_>>()
            .join(" ");
        return format!("<< {notation} ; {melodies} >>");
    }

    measure_notation_to_keyflow(measure, default_chord_length)
}

fn measure_notation_to_keyflow(
    measure: &Measure,
    default_chord_length: Option<&ChordRhythm>,
) -> String {
    let mut parts = Vec::new();

    // The ending bracket this bar opens — `[1]`, `[2]`, `[1, 3]`. It has to
    // come before the chords, which is where the parser looks for it, and
    // before the simile shortcut below: a first ending that happens to repeat
    // the bar before it is still a first ending.
    if let Some(volta) = &measure.volta_start {
        if !volta.numbers.is_empty() {
            let numbers = volta
                .numbers
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            parts.push(format!("[{numbers}]"));
        }
    }

    for text in &measure.staff_text {
        parts.push(staff_text_to_syntax(text));
    }

    // A simile bar is written the way it was meant: `%`, not the chords it
    // stands for. Spelling them out is what the mark exists to avoid, and it
    // is what the reader would have to fold back up in their head.
    if measure.simile {
        parts.push("%".to_string());
        return parts.join(" ");
    }

    for dynamic in &measure.dynamics {
        parts.push(dynamic.to_string());
    }

    for dynamic in &measure.classical_dynamics {
        parts.push(classical_dynamic_to_syntax(dynamic));
    }

    for hairpin in &measure.hairpins {
        parts.push(hairpin_to_syntax(hairpin));
    }

    if measure.chords.is_empty() {
        parts.extend(
            measure
                .figured_bass
                .iter()
                .map(figured_bass_to_standalone_syntax),
        );

        if !measure.rhythm_elements.is_empty() {
            parts.extend(
                measure
                    .rhythm_elements
                    .iter()
                    .map(|element| rhythm_element_to_syntax(element, measure.time_signature)),
            );
        }

        if !parts.is_empty() {
            return parts.join(" ");
        }

        "r1".to_string()
    } else if measure.rhythm_elements.iter().any(RhythmElement::is_rest) {
        parts.extend(
            measure
                .rhythm_elements
                .iter()
                .map(|element| rhythm_element_to_syntax(element, measure.time_signature)),
        );
        parts.join(" ")
    } else {
        let mut beat = 1u8;
        let suppress_full_measure_slashes = measure.chords.len() > 1;
        let fallback_durations =
            even_chord_duration_suffixes(measure.chords.len(), measure.time_signature);
        parts.extend(measure.chords.iter().enumerate().map(|(idx, chord)| {
            let mut token = chord_symbol_to_syntax(&chord.full_symbol);
            if let Some(figured_bass) = measure.figured_bass.iter().find(|item| {
                item.beat == beat && matches!(item.placement, Placement::Above | Placement::Below)
            }) {
                token.push('"');
                token.push_str(&escape_quoted(&figured_bass_rows_to_text(figured_bass)));
                token.push('"');
            }
            if suppress_full_measure_slashes {
                if let Some(duration) = fallback_durations.get(idx) {
                    token.push('_');
                    token.push_str(duration);
                }
                return token;
            }
            match chord.rhythm {
                ChordRhythm::Slashes {
                    count,
                    dotted,
                    tied,
                } => {
                    let matches_default =
                        default_chord_length.is_some_and(|default| *default == chord.rhythm);
                    let full_measure_slashes = count >= measure.time_signature.0;
                    if full_measure_slashes {
                        // MusicXML imports often represent a full-measure slash
                        // pattern as eight slash glyphs in 4/4. Keyflow's
                        // idiom is the bare chord for a full measure.
                    } else if !matches_default {
                        token.push(' ');
                        token.push_str(&"/".repeat(count as usize));
                        if dotted {
                            token.push('.');
                        }
                        if tied {
                            token.push('~');
                        }
                    }
                }
                ChordRhythm::Default => token.push_str(" /"),
                ChordRhythm::Explicit(_) => {}
            }
            let time_signature = TimeSignature::new(
                u32::from(measure.time_signature.0),
                u32::from(measure.time_signature.1),
            );
            beat = beat.saturating_add(chord.duration.to_beats(time_signature).round() as u8);
            token
        }));

        parts.join(" ")
    }
}

/// Duration suffixes for a bar whose chords carry no usable rhythm of their
/// own — a split bar from an importer, where all we know is how many chords
/// share the measure.
///
/// The short counts are the house conventions (`2 2`, `4 4 2`, `4 4 4 4`).
/// Beyond that the bar is subdivided so the spans still *add up*: a bar of
/// eight chords in 4/4 is eight eighths, not eight quarters. Writing eight
/// quarters into one `| … |` is not a cosmetic problem — the parser reads it
/// back as two measures, so a section quietly doubles in length.
fn even_chord_duration_suffixes(chord_count: usize, time_signature: (u8, u8)) -> Vec<String> {
    let conventional: &[&str] = if time_signature == (6, 8) {
        match chord_count {
            0 | 1 => &[],
            2 => &["4.", "4."],
            3 => &["4", "4", "4"],
            4 => &["8.", "8.", "8.", "8."],
            _ => &[],
        }
    } else {
        match chord_count {
            0 | 1 => &[],
            2 => &["2", "2"],
            3 => &["4", "4", "2"],
            4 => &["4", "4", "4", "4"],
            _ => &[],
        }
    };
    if chord_count <= 4 {
        return conventional.iter().map(|s| (*s).to_string()).collect();
    }
    subdivide_measure(chord_count, time_signature)
}

/// Split a measure among `chord_count` chords on the coarsest note grid that
/// has a slot for each, handing any leftover slots to the last chords.
fn subdivide_measure(chord_count: usize, (numerator, denominator): (u8, u8)) -> Vec<String> {
    let beat = beat_ticks(denominator);
    let bar = beat * u32::from(numerator.max(1));

    let mut unit = beat;
    while bar / unit < chord_count as u32 && unit > 1 {
        unit /= 2;
    }
    let slots = bar / unit;
    let chords = chord_count as u32;
    if slots < chords {
        // Finer than a 32nd would not help. One unit each, and the bar runs
        // long — better an odd bar than a dropped chord.
        return vec![ticks_to_suffix(unit); chord_count];
    }

    let base = slots / chords;
    let remainder = slots % chords;
    (0..chords)
        .map(|i| {
            let extra = u32::from(i >= chords - remainder);
            ticks_to_suffix((base + extra) * unit)
        })
        .collect()
}

/// Ticks in one beat, where a quarter note is 48.
fn beat_ticks(denominator: u8) -> u32 {
    match denominator {
        1 => 192,
        2 => 96,
        4 => 48,
        8 => 24,
        16 => 12,
        32 => 6,
        _ => 48,
    }
}

fn ticks_to_suffix(ticks: u32) -> String {
    const PLAIN: [(u32, &str); 6] = [
        (192, "1"),
        (96, "2"),
        (48, "4"),
        (24, "8"),
        (12, "16"),
        (6, "32"),
    ];
    for (base, name) in PLAIN {
        if ticks == base {
            return name.to_string();
        }
        if ticks == base + base / 2 {
            return format!("{name}.");
        }
    }
    PLAIN
        .iter()
        .find(|(base, _)| ticks >= *base)
        .map_or("32", |(_, name)| *name)
        .to_string()
}

fn chord_symbol_to_syntax(symbol: &str) -> String {
    if is_plain_major_symbol(symbol) {
        format!("!{symbol}")
    } else {
        symbol.to_string()
    }
}

fn is_plain_major_symbol(symbol: &str) -> bool {
    if symbol.contains('/') {
        return false;
    }
    let mut chars = symbol.chars();
    let Some(root) = chars.next() else {
        return false;
    };
    if !matches!(root, 'A'..='G') {
        return false;
    }
    match chars.next() {
        None => true,
        Some('#' | 'b') => chars.next().is_none(),
        Some(_) => false,
    }
}

fn rhythm_element_to_syntax(element: &RhythmElement, time_signature: (u8, u8)) -> String {
    match element {
        RhythmElement::Chord(chord) => chord_to_syntax(chord, None, time_signature),
        RhythmElement::Rest(rest) => {
            if !rest.original_token.is_empty() {
                rest.original_token.clone()
            } else {
                format!(
                    "r{}",
                    duration_to_lily_syntax(
                        rest.duration,
                        TimeSignature::new(
                            u32::from(time_signature.0),
                            u32::from(time_signature.1),
                        )
                    )
                )
            }
        }
        RhythmElement::Space(space) => {
            if !space.original_token.is_empty() {
                space.original_token.clone()
            } else {
                format!(
                    "s{}",
                    duration_to_lily_syntax(
                        space.duration,
                        TimeSignature::new(
                            u32::from(time_signature.0),
                            u32::from(time_signature.1),
                        )
                    )
                )
            }
        }
    }
}

fn chord_to_syntax(
    chord: &keyflow_proto::chart::types::ChordInstance,
    default_chord_length: Option<&ChordRhythm>,
    time_signature: (u8, u8),
) -> String {
    let mut token = chord_symbol_to_syntax(&chord.full_symbol);
    match chord.rhythm {
        ChordRhythm::Slashes {
            count,
            dotted,
            tied,
        } => {
            let matches_default =
                default_chord_length.is_some_and(|default| *default == chord.rhythm);
            if !matches_default && count != time_signature.0 {
                token.push(' ');
                token.push_str(&"/".repeat(count as usize));
                if dotted {
                    token.push('.');
                }
                if tied {
                    token.push('~');
                }
            }
        }
        ChordRhythm::Default => token.push_str(" /"),
        // An explicit note value is the only thing that says how long a
        // sub-beat chord lasts. Dropped, the chord reads as a whole bar and
        // the measure runs long.
        ChordRhythm::Explicit(duration) => token.push_str(&explicit_duration_suffix(&duration)),
    }
    token
}

/// `_8`, `_4.`, `_16~` — a chord's explicit note value in keyflow syntax.
fn explicit_duration_suffix(duration: &keyflow_proto::core::duration::NotationDuration) -> String {
    use keyflow_proto::core::duration::NoteValue;
    let value = match duration.note_value {
        NoteValue::Whole => "1",
        NoteValue::Half => "2",
        NoteValue::Quarter => "4",
        NoteValue::Eighth => "8",
        NoteValue::Sixteenth => "16",
        NoteValue::ThirtySecond => "32",
        NoteValue::SixtyFourth => "64",
    };
    let mut out = format!("_{value}");
    for _ in 0..duration.dots {
        out.push('.');
    }
    if duration.tied {
        out.push('~');
    }
    out
}

fn melody_to_syntax(melody: &Melody) -> String {
    melody.to_string().replace("m{", "m {")
}

fn duration_to_lily_syntax(duration: MusicalDuration, time_sig: TimeSignature) -> String {
    let beats = duration.to_beats(time_sig);
    for (value, base_beats) in [
        (1, f64::from(time_sig.denominator)),
        (2, f64::from(time_sig.denominator) / 2.0),
        (4, f64::from(time_sig.denominator) / 4.0),
        (8, f64::from(time_sig.denominator) / 8.0),
        (16, f64::from(time_sig.denominator) / 16.0),
        (32, f64::from(time_sig.denominator) / 32.0),
    ] {
        if (beats - base_beats).abs() < 0.001 {
            return value.to_string();
        }
        if (beats - base_beats * 1.5).abs() < 0.001 {
            return format!("{value}.");
        }
    }
    "1".to_string()
}

fn figured_bass_to_standalone_syntax(item: &FiguredBass) -> String {
    let prefix = match item.placement {
        Placement::Above => "^",
        Placement::Below => "",
    };
    format!(
        "{prefix}\"{}\"",
        escape_quoted(&figured_bass_rows_to_text(item))
    )
}

fn common_chord_length(measures: &[Measure]) -> Option<ChordRhythm> {
    let mut common: Option<ChordRhythm> = None;
    let mut chord_count = 0usize;

    for measure in measures {
        if !measure.melodies.is_empty() {
            return None;
        }
        for chord in &measure.chords {
            let ChordRhythm::Slashes { .. } = chord.rhythm else {
                return None;
            };
            chord_count += 1;
            if let Some(existing) = &common {
                if *existing != chord.rhythm {
                    return None;
                }
            } else {
                common = Some(chord.rhythm.clone());
            }
        }
    }

    (chord_count > 1)
        .then_some(common?)
        .filter(|_| chord_count > measures.len())
}

fn chord_length_to_syntax(rhythm: &ChordRhythm, time_signature: (u8, u8)) -> String {
    match rhythm {
        ChordRhythm::Slashes { count, dotted, .. } => {
            let slash_beats = f64::from(*count) * if *dotted { 1.5 } else { 1.0 };
            let dotted_quarter_beats = f64::from(time_signature.1) / 4.0 * 1.5;
            if (slash_beats - dotted_quarter_beats).abs() < 0.001 {
                return "4.".to_string();
            }
            let mut out = "/".repeat(*count as usize);
            if *dotted {
                out.push('.');
            }
            out
        }
        ChordRhythm::Default | ChordRhythm::Explicit(_) => "/".to_string(),
    }
}

fn figured_bass_rows_to_text(item: &FiguredBass) -> String {
    figured_bass_rows(item).join(" ")
}

fn figured_bass_rows(item: &FiguredBass) -> Vec<String> {
    let mut rows = Vec::new();
    for row in &item.rows {
        let mut parts = row.text.split_whitespace();
        if let Some(first) = parts.next() {
            let split_first = split_compacted_figured_row(first);
            rows.push(format!("{}{}", row.accidental, split_first[0]));
            rows.extend(split_first.into_iter().skip(1).map(str::to_string));
            rows.extend(parts.map(str::to_string));
        }
    }
    rows
}

fn split_compacted_figured_row(row: &str) -> Vec<&str> {
    let chars = row.chars().collect::<Vec<_>>();
    if chars.len() == 6
        && chars[0].is_ascii_digit()
        && chars[1] == '-'
        && chars[2].is_ascii_digit()
        && chars[3].is_ascii_digit()
        && chars[4] == '-'
        && chars[5].is_ascii_digit()
    {
        vec![&row[..3], &row[3..]]
    } else {
        vec![row]
    }
}

fn staff_text_to_syntax(text: &StaffText) -> String {
    let prefix = match text.placement {
        Placement::Above => "^",
        Placement::Below => "",
    };
    format!("{prefix}\"{}\"", escape_quoted(&text.text))
}

fn classical_dynamic_to_syntax(dynamic: &Dynamic) -> String {
    let mut text = format!("dyn {}", dynamic.level.as_str());
    if dynamic.beat != 1 {
        text.push('@');
        text.push_str(&dynamic.beat.to_string());
    }
    if dynamic.placement == Placement::Above {
        text.push_str(" above");
    }
    text
}

fn hairpin_to_syntax(hairpin: &Hairpin) -> String {
    let kind = match hairpin.kind {
        HairpinKind::Crescendo => "<",
        HairpinKind::Decrescendo => ">",
    };
    let mut text = format!(
        "hairpin {kind} {}..{}",
        hairpin.start_beat, hairpin.end_beat
    );
    if hairpin.placement == Placement::Above {
        text.push_str(" above");
    }
    text
}

fn escape_quoted(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
