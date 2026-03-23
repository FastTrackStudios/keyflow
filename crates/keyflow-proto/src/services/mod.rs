//! Services module - Service trait definitions for Keyflow

use facet::Facet;
use vox::service;

/// Service for chart operations
#[service]
pub trait ChartService {
    /// Get chart by ID
    async fn get_chart(&self, chart_id: String) -> Option<crate::Chart>;

    /// Parse chart from keyflow syntax
    async fn parse(&self, text: String) -> Result<crate::Chart, crate::ParseError>;
}

/// Service for parsing chart sources into a Chart.
///
/// This is the generic interface for text and MIDI sources.
#[service]
pub trait ChartParseService {
    async fn parse_text(&self, text: String) -> Result<crate::Chart, String>;
    async fn parse_midi(&self, bytes: Vec<u8>) -> Result<crate::Chart, String>;
}

/// Service for parsing operations
#[service]
pub trait ParserService {
    /// Parse chart text
    async fn parse_chart(&self, request: ParseRequest) -> Result<ParseResponse, crate::ParseError>;
}

/// Parse request
#[derive(Clone, Facet)]
pub struct ParseRequest {
    pub text: String,
}

/// Parse response
#[derive(Clone, Facet)]
pub struct ParseResponse {
    pub chart: crate::Chart,
}

/// Request for generating guide events during a count-in region.
#[derive(Clone, Facet)]
pub struct GuideRequest {
    /// Section start position in quarter notes.
    pub section_start_quarters: f64,
    /// Count-in start position in quarter notes.
    pub count_in_start_quarters: f64,
    /// Time signature for the section.
    pub time_signature: crate::TimeSignature,
    /// Tempo in BPM.
    pub tempo: f64,
    /// The section type being counted into.
    pub section_type: crate::SectionType,
    /// Optional section number (e.g., Verse 1, Verse 2).
    pub section_number: Option<u32>,
    /// Click track configuration.
    pub click_config: crate::ClickConfig,
    /// Count-in configuration.
    pub count_in_config: crate::CountInConfig,
    /// Section guide cue configuration.
    pub guide_config: crate::GuideConfig,
}

/// Service for generating click track, count-in, and section guide events.
#[service]
pub trait GuideService {
    /// Generate all guide events for a section's count-in region.
    ///
    /// Returns a list of `GuideEvent`s sorted by position.
    async fn generate_count_in_events(
        &self,
        request: GuideRequest,
    ) -> Vec<crate::GuideEvent>;
}
