//! Services module - Service trait definitions for Keyflow

use facet::Facet;
use roam::service;

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
