//! Where each beat sits in time and on the page: the beat positions the
//! cursor follows, the count-in's place before the downbeat, and what the
//! count-in snippet needs to know about the song's first chord.

use super::count_in_renderer::CountInBeatGeometry;
use super::{BeatPosition, slash_glyph_for_ticks};
use crate::engraver::layout::SegmentList;
use crate::engraver::layout::segment::SegmentType;

/// How many count-in bars the chart opens with: an explicit config
/// override, else the chart's own `CountIn` section length.
pub(super) fn count_in_measure_count(chart: &crate::chart::Chart, configured: u8) -> usize {
    use crate::sections::SectionType;
    // If the chart has a CountIn section, use its measure count.
    if configured > 0 {
        configured as usize
    } else {
        chart
            .sections
            .iter()
            .find(|s| matches!(s.section.section_type, SectionType::CountIn))
            .map(|s| s.measures().len())
            .unwrap_or(0)
    }
}

/// Where one laid-out bar sits: on the page, and in the song.
pub(super) struct BarOnTimeline {
    pub(super) page: u32,
    pub(super) system: usize,
    pub(super) measure: usize,
    pub(super) measure_x: f64,
    pub(super) measure_width: f64,
    pub(super) staff_y: f64,
    pub(super) staff_height: f64,
    pub(super) spatium: f64,
    /// Song time (seconds) and tick at the bar's downbeat.
    pub(super) time_start: f64,
    pub(super) tick_start: i64,
    pub(super) seconds_per_tick: f64,
    pub(super) time_signature: (u8, u8),
}

/// One [`BeatPosition`] per chord-rest segment of a laid-out bar, from the
/// actual segment data.
pub(super) fn bar_beat_positions<'a>(
    segments: &'a SegmentList,
    bar: &'a BarOnTimeline,
) -> impl Iterator<Item = BeatPosition> + 'a {
    segments
        .iter_type(SegmentType::CHORD_REST)
        .enumerate()
        .map(move |(beat_idx, segment)| {
            let time_start = bar.time_start + (segment.tick as f64 * bar.seconds_per_tick);
            let time_end =
                bar.time_start + ((segment.tick + segment.ticks) as f64 * bar.seconds_per_tick);
            let absolute_tick = bar.tick_start + segment.tick as i64;

            // Compute stem/flag info based on duration
            // Whole (1920) and half (960) are stemless in slash notation
            // Quarter (480) and shorter have stems
            let has_stem = segment.ticks < 960;
            let flag_count = match segment.ticks {
                t if t >= 480 => 0, // Quarter or longer: no flags
                t if t >= 240 => 1, // Eighth: 1 flag
                t if t >= 120 => 2, // 16th: 2 flags
                t if t >= 60 => 3,  // 32nd: 3 flags
                _ => 4,             // 64th: 4 flags
            };

            BeatPosition {
                page: bar.page,
                system: bar.system,
                measure: bar.measure,
                beat: beat_idx,
                tick: segment.tick,
                duration_ticks: segment.ticks,
                absolute_tick,
                x: bar.measure_x + segment.x,
                width: segment.width,
                measure_x0: bar.measure_x,
                measure_x1: bar.measure_x + bar.measure_width,
                staff_y: bar.staff_y,
                staff_height: bar.staff_height,
                time_start,
                time_end,
                glyph_codepoint: Some(slash_glyph_for_ticks(segment.ticks)),
                glyph_size: bar.spatium,
                glyph_y: bar.staff_y + bar.staff_height / 2.0, // Center on staff
                has_stem,
                stem_up: true, // Default to stem up
                flag_count,
                time_signature: bar.time_signature,
            }
        })
}

/// The count-in's span of the song: the bars before the downbeat, which
/// carry negative ticks so the real measures start at tick 0.
pub(super) struct CountInTiming {
    pub(super) count_in_ticks: i64,
    pub(super) ticks_per_measure: i64,
    pub(super) seconds_per_tick: f64,
    pub(super) time_signature: (u8, u8),
}

/// Create BeatPosition entries for count-in beats so the
/// cursor can highlight them during the count-in period.
pub(super) fn count_in_beat_positions<'a>(
    geos: &'a [CountInBeatGeometry],
    page: u32,
    timing: &'a CountInTiming,
) -> impl Iterator<Item = BeatPosition> + 'a {
    let time_signature = timing.time_signature;
    let ticks_per_beat = 1920i64 / time_signature.1 as i64;
    geos.iter().map(move |geo| {
        let beat_offset_in_measure = geo.beat_index as i64 * ticks_per_beat;
        let measure_offset = geo.measure_index as i64 * timing.ticks_per_measure;
        let absolute_tick = -timing.count_in_ticks + measure_offset + beat_offset_in_measure;
        BeatPosition {
            page,
            system: 0,
            measure: geo.measure_index,
            beat: geo.beat_index,
            tick: beat_offset_in_measure as i32,
            duration_ticks: ticks_per_beat as i32,
            absolute_tick,
            x: geo.x,
            width: geo.width,
            measure_x0: geo.x,
            measure_x1: geo.x + geo.width,
            staff_y: geo.staff_y,
            staff_height: geo.staff_height,
            time_start: absolute_tick as f64 * timing.seconds_per_tick,
            time_end: (absolute_tick + ticks_per_beat) as f64 * timing.seconds_per_tick,
            glyph_codepoint: Some(geo.glyph_codepoint),
            glyph_size: geo.glyph_size,
            glyph_y: geo.glyph_y,
            has_stem: false,
            stem_up: true,
            flag_count: 0,
            time_signature,
        }
    })
}

/// Post-process beat positions to make them contiguous (no gaps at barlines).
/// Each beat's width is extended to reach the next beat's x position,
/// ensuring smooth cursor movement across measure boundaries.
pub(super) fn close_beat_gaps(beat_positions: &mut [BeatPosition]) {
    for i in 0..beat_positions.len().saturating_sub(1) {
        let next_x = beat_positions[i + 1].x;
        let current_x = beat_positions[i].x;
        // Only extend if next beat is on same system (same y position)
        // and to the right (not a system break)
        if beat_positions[i + 1].staff_y == beat_positions[i].staff_y && next_x > current_x {
            beat_positions[i].width = next_x - current_x;
        }
    }
}

/// Detect whether the chart's *first* chord (skipping count-in / compact
/// sections) carries a push (anticipation) flag. Used to render the count-in
/// snippet's beat-4 indicator when the song starts on a pushed chord.
pub(super) fn chart_first_chord_is_pushed(chart: &crate::chart::Chart) -> bool {
    use crate::sections::SectionType;
    chart
        .sections
        .iter()
        .find(|s| {
            !s.section.section_type.is_compact()
                && !matches!(s.section.section_type, SectionType::End)
        })
        .and_then(|s| s.measures().iter().find(|m| !m.chords.is_empty()))
        .and_then(|m| m.chords.first())
        .and_then(|c| c.push_pull)
        .map(|(is_push, _)| is_push)
        .unwrap_or(false)
}
