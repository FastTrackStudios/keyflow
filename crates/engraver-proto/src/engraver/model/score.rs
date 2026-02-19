//! Score representation - the top-level container for music notation.

use super::element::TimeSignature;
use super::part::{Part, PartId};
use serde::{Deserialize, Serialize};

/// Score metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScoreMetadata {
    /// Title of the work
    pub title: Option<String>,
    /// Subtitle
    pub subtitle: Option<String>,
    /// Composer
    pub composer: Option<String>,
    /// Arranger
    pub arranger: Option<String>,
    /// Lyricist
    pub lyricist: Option<String>,
    /// Copyright notice
    pub copyright: Option<String>,
}

impl ScoreMetadata {
    /// Create new empty metadata.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the title.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the composer.
    #[must_use]
    pub fn with_composer(mut self, composer: impl Into<String>) -> Self {
        self.composer = Some(composer.into());
        self
    }
}

/// Layout settings for score rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutSettings {
    /// Staff size in millimeters (standard is 7.0mm)
    pub staff_size_mm: f64,
    /// Page width in millimeters
    pub page_width_mm: f64,
    /// Page height in millimeters
    pub page_height_mm: f64,
    /// Left margin in millimeters
    pub margin_left_mm: f64,
    /// Right margin in millimeters
    pub margin_right_mm: f64,
    /// Top margin in millimeters
    pub margin_top_mm: f64,
    /// Bottom margin in millimeters
    pub margin_bottom_mm: f64,
    /// Space between systems in staff spaces
    pub system_distance: f64,
    /// Space between staves within a system in staff spaces
    pub staff_distance: f64,
}

impl Default for LayoutSettings {
    fn default() -> Self {
        Self {
            staff_size_mm: 7.0,
            // US Letter paper (8.5" × 11")
            page_width_mm: 215.9,
            page_height_mm: 279.4,
            margin_left_mm: 15.0,
            margin_right_mm: 15.0,
            margin_top_mm: 20.0,
            margin_bottom_mm: 20.0,
            system_distance: 12.0,
            staff_distance: 6.0,
        }
    }
}

impl LayoutSettings {
    /// A4 paper size (210mm × 297mm).
    #[must_use]
    pub fn a4() -> Self {
        Self {
            page_width_mm: 210.0,
            page_height_mm: 297.0,
            ..Self::default()
        }
    }

    /// US Letter paper size (8.5" × 11") - same as default.
    #[must_use]
    pub fn letter() -> Self {
        Self::default()
    }

    /// Get the space unit (1/4 of staff size).
    #[must_use]
    pub fn space(&self) -> f64 {
        self.staff_size_mm / 4.0
    }

    /// Get the available width for music content.
    #[must_use]
    pub fn content_width(&self) -> f64 {
        self.page_width_mm - self.margin_left_mm - self.margin_right_mm
    }

    /// Get the available height for music content.
    #[must_use]
    pub fn content_height(&self) -> f64 {
        self.page_height_mm - self.margin_top_mm - self.margin_bottom_mm
    }
}

/// A complete musical score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Score {
    /// Score metadata (title, composer, etc.)
    pub metadata: ScoreMetadata,
    /// Parts in this score
    pub parts: Vec<Part>,
    /// Layout settings
    pub layout: LayoutSettings,
    /// Initial time signature
    pub time_signature: TimeSignature,
    /// Tempo in BPM (optional)
    pub tempo_bpm: Option<f64>,
}

impl Score {
    /// Create a new empty score.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a score with the given title.
    #[must_use]
    pub fn with_title(title: impl Into<String>) -> Self {
        Self {
            metadata: ScoreMetadata::new().with_title(title),
            ..Self::default()
        }
    }

    /// Add a part to this score.
    pub fn add_part(&mut self, part: Part) {
        self.parts.push(part);
    }

    /// Get a part by ID.
    #[must_use]
    pub fn get_part(&self, id: PartId) -> Option<&Part> {
        self.parts.iter().find(|p| p.id == id)
    }

    /// Get a mutable reference to a part by ID.
    pub fn get_part_mut(&mut self, id: PartId) -> Option<&mut Part> {
        self.parts.iter_mut().find(|p| p.id == id)
    }

    /// Get the total number of measures (from the longest part).
    #[must_use]
    pub fn measure_count(&self) -> usize {
        self.parts
            .iter()
            .map(|p| p.measures.len())
            .max()
            .unwrap_or(0)
    }

    /// Set the tempo.
    #[must_use]
    pub fn with_tempo(mut self, bpm: f64) -> Self {
        self.tempo_bpm = Some(bpm);
        self
    }

    /// Set the time signature.
    #[must_use]
    pub fn with_time_signature(mut self, time_sig: TimeSignature) -> Self {
        self.time_signature = time_sig;
        self
    }

    /// Set the layout settings.
    #[must_use]
    pub fn with_layout(mut self, layout: LayoutSettings) -> Self {
        self.layout = layout;
        self
    }
}

impl Default for Score {
    fn default() -> Self {
        Self {
            metadata: ScoreMetadata::default(),
            parts: Vec::new(),
            layout: LayoutSettings::default(),
            time_signature: TimeSignature::COMMON,
            tempo_bpm: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engraver::model::{
        Duration, DurationKind, Measure, MusicElement, Note, Octave, Pitch, PitchClass, Voice,
    };

    #[test]
    fn test_create_simple_score() {
        let mut score = Score::with_title("Test Score")
            .with_tempo(120.0)
            .with_time_signature(TimeSignature::COMMON);

        let mut part = Part::new(PartId(1), "Violin");

        let mut measure = Measure::new(1);
        let mut voice = Voice::new();
        voice.add(MusicElement::Note(Note::new(
            Pitch::new(PitchClass::C, Octave::MIDDLE),
            Duration::new(DurationKind::Quarter),
        )));
        measure.voices = vec![voice];
        part.add_measure(measure);

        score.add_part(part);

        assert_eq!(score.metadata.title, Some("Test Score".into()));
        assert_eq!(score.tempo_bpm, Some(120.0));
        assert_eq!(score.parts.len(), 1);
        assert_eq!(score.measure_count(), 1);
    }
}
