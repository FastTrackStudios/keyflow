//! Horizontal measure layout calculations.
//!
//! This module provides structures and algorithms for calculating the
//! horizontal layout of measures and beat positions within a system.
//! Based on MuseScore's measure spacing algorithm.

use crate::engraver::style::{MStyle, Sid};
use serde::{Deserialize, Serialize};

/// Computed layout for a single measure.
///
/// Contains the measure's position, width, and the positions of all beats
/// within the measure for precise element placement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasureLayout {
    /// Measure number (1-indexed)
    pub number: usize,
    /// X position of measure start (left barline) in points
    pub x: f32,
    /// Total measure width in points
    pub width: f32,
    /// Beat positions within measure (relative to measure start)
    pub beat_positions: Vec<BeatPosition>,
    /// Time signature for this measure (numerator, denominator)
    pub time_sig: (u8, u8),
    /// Whether this is the first measure in the system (has clef/key/time sig)
    pub is_first_in_system: bool,
}

impl MeasureLayout {
    /// Get the X position for a specific beat within this measure.
    ///
    /// Beat is 0-indexed (beat 0 is the downbeat).
    /// Returns None if the beat is out of range.
    #[must_use]
    pub fn beat_x(&self, beat: u32) -> Option<f32> {
        self.beat_positions
            .get(beat as usize)
            .map(|bp| self.x + bp.x)
    }

    /// Get the X position for a fractional beat (e.g., 1.5 for beat 2 halfway).
    ///
    /// Uses linear interpolation between beat positions.
    #[must_use]
    pub fn beat_x_fractional(&self, beat: f32) -> f32 {
        let beat_floor = beat.floor() as usize;
        let beat_frac = beat - beat.floor();

        if let Some(bp) = self.beat_positions.get(beat_floor) {
            let base_x = bp.x;
            let next_width = self
                .beat_positions
                .get(beat_floor + 1)
                .map(|next| next.x - bp.x)
                .unwrap_or(bp.width);

            self.x + base_x + (next_width * beat_frac)
        } else {
            // Beyond last beat, extrapolate
            self.x + self.width
        }
    }

    /// Get the content start X (after barline and bar-note distance).
    #[must_use]
    pub fn content_start_x(&self) -> f32 {
        self.beat_positions
            .first()
            .map(|bp| self.x + bp.x)
            .unwrap_or(self.x)
    }

    /// Get the content end X (before note-bar distance).
    #[must_use]
    pub fn content_end_x(&self) -> f32 {
        self.beat_positions
            .last()
            .map(|bp| self.x + bp.x + bp.width)
            .unwrap_or(self.x + self.width)
    }
}

/// Position of a beat within a measure.
///
/// Each beat has an X offset from the measure start and an allocated width.
/// Elements placed on this beat should be positioned at x, and the width
/// indicates how much horizontal space is available until the next beat.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BeatPosition {
    /// Beat number (0-indexed)
    pub beat: u32,
    /// X offset from measure start in points
    pub x: f32,
    /// Width allocated to this beat in points
    pub width: f32,
}

/// Configuration for measure layout calculation.
#[derive(Debug, Clone)]
pub struct MeasureLayoutConfig {
    /// Distance from barline to first beat (in points)
    pub bar_note_distance: f32,
    /// Distance from last beat to barline (in points)
    pub note_bar_distance: f32,
    /// Minimum measure width (in points)
    pub min_measure_width: f32,
    /// Extra width for first measure (clef, key sig, time sig)
    pub first_measure_extra: f32,
}

impl MeasureLayoutConfig {
    /// Create configuration from an MStyle.
    #[must_use]
    pub fn from_style(style: &MStyle) -> Self {
        let spatium = style.base_spatium();
        Self {
            bar_note_distance: style.spatium(Sid::BarNoteDistance) * spatium,
            note_bar_distance: style.spatium(Sid::NoteBarDistance) * spatium,
            min_measure_width: spatium * 10.0, // Reasonable default
            first_measure_extra: spatium * 8.0, // Space for clef + time sig
        }
    }
}

/// Calculate beat X positions within a single measure.
///
/// Uses MuseScore's proportional spacing algorithm:
/// - Beat positions are evenly distributed in the usable width
/// - Usable width = measure_width - bar_note_distance - note_bar_distance
///
/// # Arguments
/// * `measure_width` - Total measure width in points
/// * `time_sig` - Time signature (numerator, denominator)
/// * `bar_note_distance` - Space from barline to first beat
/// * `note_bar_distance` - Space from last beat to barline
///
/// # Returns
/// Vector of beat positions within the measure
#[must_use]
pub fn calculate_beat_positions(
    measure_width: f32,
    time_sig: (u8, u8),
    bar_note_distance: f32,
    note_bar_distance: f32,
) -> Vec<BeatPosition> {
    let beats_per_measure = time_sig.0 as f32;
    let usable_width = (measure_width - bar_note_distance - note_bar_distance).max(0.0);
    let beat_width = if beats_per_measure > 0.0 {
        usable_width / beats_per_measure
    } else {
        usable_width
    };

    (0..time_sig.0 as u32)
        .map(|beat| BeatPosition {
            beat,
            x: bar_note_distance + (beat as f32 * beat_width),
            width: beat_width,
        })
        .collect()
}

/// Information about a measure for layout calculation.
#[derive(Debug, Clone)]
pub struct MeasureInfo {
    /// Measure number (1-indexed)
    pub number: usize,
    /// Time signature for this measure
    pub time_sig: (u8, u8),
    /// Whether this measure starts a new section (may need extra space)
    pub is_section_start: bool,
}

/// Compute measure layouts for a complete system (line of music).
///
/// This distributes the available system width among all measures,
/// accounting for:
/// - First measure extra space (clef, key sig, time sig)
/// - Proportional spacing based on beat count
/// - Minimum measure widths
///
/// # Arguments
/// * `measures` - Information about each measure
/// * `system_width` - Total available width for the system in points
/// * `config` - Layout configuration
/// * `is_first_system` - Whether this is the first system (has extra elements)
///
/// # Returns
/// Vector of computed measure layouts
#[must_use]
pub fn compute_measure_layouts(
    measures: &[MeasureInfo],
    system_width: f32,
    config: &MeasureLayoutConfig,
    is_first_system: bool,
) -> Vec<MeasureLayout> {
    if measures.is_empty() {
        return Vec::new();
    }

    // Calculate total "weight" for width distribution
    // Each measure's weight is based on its beat count
    let total_beats: f32 = measures.iter().map(|m| m.time_sig.0 as f32).sum();

    // Account for first measure extra width
    let first_extra = if is_first_system {
        config.first_measure_extra
    } else {
        // Subsequent systems still need clef space
        config.first_measure_extra * 0.5
    };

    // Distributable width (after accounting for fixed elements)
    let distributable = system_width - first_extra;

    // Calculate width per beat
    let width_per_beat = if total_beats > 0.0 {
        distributable / total_beats
    } else {
        distributable / measures.len() as f32
    };

    let mut layouts = Vec::with_capacity(measures.len());
    let mut current_x = 0.0;

    for (i, measure) in measures.iter().enumerate() {
        let is_first = i == 0;

        // Calculate measure width based on beat count
        let base_width = measure.time_sig.0 as f32 * width_per_beat;
        let extra = if is_first { first_extra } else { 0.0 };
        let measure_width = (base_width + extra).max(config.min_measure_width);

        // Calculate beat positions within this measure
        let beat_positions = calculate_beat_positions(
            measure_width - extra, // Beat positions in the content area
            measure.time_sig,
            config.bar_note_distance,
            config.note_bar_distance,
        );

        // Adjust beat positions if first measure (shift by extra amount)
        let adjusted_beats: Vec<BeatPosition> = if is_first && extra > 0.0 {
            beat_positions
                .into_iter()
                .map(|mut bp| {
                    bp.x += extra;
                    bp
                })
                .collect()
        } else {
            beat_positions
        };

        layouts.push(MeasureLayout {
            number: measure.number,
            x: current_x,
            width: measure_width,
            beat_positions: adjusted_beats,
            time_sig: measure.time_sig,
            is_first_in_system: is_first,
        });

        current_x += measure_width;
    }

    layouts
}

/// Stretch or compress measure layouts to exactly fit a target width.
///
/// This is used for justification - making the measures fill the entire
/// system width without gaps.
pub fn justify_measure_layouts(layouts: &mut [MeasureLayout], target_width: f32) {
    if layouts.is_empty() {
        return;
    }

    let current_width: f32 = layouts.iter().map(|m| m.width).sum();
    if current_width <= 0.0 {
        return;
    }

    let scale = target_width / current_width;

    let mut current_x = 0.0;
    for layout in layouts.iter_mut() {
        layout.x = current_x;
        layout.width *= scale;

        // Scale beat positions within the measure
        for bp in &mut layout.beat_positions {
            bp.x *= scale;
            bp.width *= scale;
        }

        current_x += layout.width;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_beat_positions_4_4() {
        let positions = calculate_beat_positions(100.0, (4, 4), 10.0, 5.0);

        assert_eq!(positions.len(), 4);
        assert!((positions[0].x - 10.0).abs() < 0.01); // First beat at bar_note_distance
        assert!((positions[0].width - 21.25).abs() < 0.01); // (100 - 10 - 5) / 4 = 21.25
    }

    #[test]
    fn test_beat_positions_3_4() {
        let positions = calculate_beat_positions(100.0, (3, 4), 10.0, 5.0);

        assert_eq!(positions.len(), 3);
        let expected_width = (100.0 - 10.0 - 5.0) / 3.0;
        assert!((positions[0].width - expected_width).abs() < 0.01);
    }

    #[test]
    fn test_measure_layout_beat_x() {
        let layout = MeasureLayout {
            number: 1,
            x: 50.0,
            width: 100.0,
            beat_positions: vec![
                BeatPosition {
                    beat: 0,
                    x: 10.0,
                    width: 20.0,
                },
                BeatPosition {
                    beat: 1,
                    x: 30.0,
                    width: 20.0,
                },
            ],
            time_sig: (4, 4),
            is_first_in_system: true,
        };

        assert_eq!(layout.beat_x(0), Some(60.0)); // 50 + 10
        assert_eq!(layout.beat_x(1), Some(80.0)); // 50 + 30
        assert_eq!(layout.beat_x(5), None); // Out of range
    }

    #[test]
    fn test_measure_layout_fractional_beat() {
        let layout = MeasureLayout {
            number: 1,
            x: 0.0,
            width: 100.0,
            beat_positions: vec![
                BeatPosition {
                    beat: 0,
                    x: 10.0,
                    width: 20.0,
                },
                BeatPosition {
                    beat: 1,
                    x: 30.0,
                    width: 20.0,
                },
            ],
            time_sig: (4, 4),
            is_first_in_system: false,
        };

        // Beat 0.5 should be halfway between beat 0 and beat 1
        let x = layout.beat_x_fractional(0.5);
        assert!((x - 20.0).abs() < 0.01); // 10 + (30-10)*0.5 = 20
    }

    #[test]
    fn test_compute_measure_layouts() {
        let config = MeasureLayoutConfig {
            bar_note_distance: 10.0,
            note_bar_distance: 5.0,
            min_measure_width: 50.0,
            first_measure_extra: 40.0,
        };

        let measures = vec![
            MeasureInfo {
                number: 1,
                time_sig: (4, 4),
                is_section_start: true,
            },
            MeasureInfo {
                number: 2,
                time_sig: (4, 4),
                is_section_start: false,
            },
        ];

        let layouts = compute_measure_layouts(&measures, 200.0, &config, true);

        assert_eq!(layouts.len(), 2);
        assert!(layouts[0].is_first_in_system);
        assert!(!layouts[1].is_first_in_system);

        // First measure should be wider due to extra space
        assert!(layouts[0].width > layouts[1].width);
    }

    #[test]
    fn test_justify_measure_layouts() {
        let mut layouts = vec![
            MeasureLayout {
                number: 1,
                x: 0.0,
                width: 100.0,
                beat_positions: vec![BeatPosition {
                    beat: 0,
                    x: 10.0,
                    width: 20.0,
                }],
                time_sig: (4, 4),
                is_first_in_system: true,
            },
            MeasureLayout {
                number: 2,
                x: 100.0,
                width: 100.0,
                beat_positions: vec![BeatPosition {
                    beat: 0,
                    x: 10.0,
                    width: 20.0,
                }],
                time_sig: (4, 4),
                is_first_in_system: false,
            },
        ];

        justify_measure_layouts(&mut layouts, 300.0);

        // Total width should now be 300
        let total: f32 = layouts.iter().map(|m| m.width).sum();
        assert!((total - 300.0).abs() < 0.01);

        // Each measure should be 150 (scaled from 100)
        assert!((layouts[0].width - 150.0).abs() < 0.01);
        assert!((layouts[1].width - 150.0).abs() < 0.01);
    }
}
