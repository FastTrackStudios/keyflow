//! Element-specific layout implementations.
//!
//! This module provides the `Layout` trait and element-specific layout
//! implementations, replacing MuseScore's 6,719-line TLayout.cpp with
//! modular, trait-based dispatch.

use kurbo::{Point, Rect};

use crate::engraver::error::Result;
use crate::engraver::layout::context::LayoutContext;
use crate::engraver::layout::shape::Shape;
use crate::engraver::model::ElementId;

/// Layout data produced for a music element.
///
/// Contains the computed position, bounding box, and collision shape
/// for a single element after layout.
#[derive(Debug, Clone)]
pub struct LayoutData {
    /// Position relative to parent element (in points)
    pub position: Point,
    /// Bounding box for hit testing (in points)
    pub bbox: Rect,
    /// Detailed shape for collision detection
    pub shape: Shape,
    /// Child element layouts (for hierarchical elements)
    pub children: Vec<(ElementId, LayoutData)>,
}

impl LayoutData {
    /// Create new layout data.
    #[must_use]
    pub fn new(position: Point, bbox: Rect, shape: Shape) -> Self {
        Self {
            position,
            bbox,
            shape,
            children: Vec::new(),
        }
    }

    /// Create layout data with children.
    #[must_use]
    pub fn with_children(
        position: Point,
        bbox: Rect,
        shape: Shape,
        children: Vec<(ElementId, LayoutData)>,
    ) -> Self {
        Self {
            position,
            bbox,
            shape,
            children,
        }
    }

    /// Add a child element's layout data.
    pub fn add_child(&mut self, id: ElementId, layout: LayoutData) {
        self.children.push((id, layout));
    }

    /// Translate this layout data by an offset.
    #[must_use]
    pub fn translate(&self, offset: Point) -> Self {
        Self {
            position: Point::new(self.position.x + offset.x, self.position.y + offset.y),
            bbox: self
                .bbox
                .with_origin(Point::new(self.bbox.x0 + offset.x, self.bbox.y0 + offset.y)),
            shape: self.shape.translate(offset),
            children: self.children.clone(),
        }
    }
}

/// Core layout trait for music elements.
///
/// All music notation elements implement this trait to provide their
/// layout logic. This replaces MuseScore's TLayout static factory class
/// with a more idiomatic Rust approach using trait dispatch.
///
/// # Error Handling
///
/// All methods return `Result` to allow proper error propagation instead
/// of panicking. Common errors include empty collections, missing fonts,
/// and invalid element indices.
///
/// # Implementation Strategy
///
/// - Implement directly for concrete types (Note, Harmony, Rest, etc.)
/// - Use enum dispatch via MusicElement for generic handling
/// - Each element type has a dedicated module (harmony.rs, chord.rs, etc.)
///
/// # Example
///
/// ```ignore
/// impl Layout for Harmony {
///     fn layout(&self, ctx: &LayoutContext) -> Result<LayoutData> {
///         // Chord symbol-specific layout logic
///         harmony::layout_harmony(self, ctx)
///     }
///
///     fn shape(&self, ctx: &LayoutContext) -> Result<Shape> {
///         harmony::harmony_shape(self, ctx)
///     }
/// }
/// ```
pub trait Layout {
    /// Compute layout for this element.
    ///
    /// Returns a `LayoutData` containing position, bounding box,
    /// and collision shape for the element.
    ///
    /// # Errors
    ///
    /// Returns an error if the element cannot be laid out (e.g., empty chord).
    fn layout(&self, ctx: &LayoutContext) -> Result<LayoutData>;

    /// Get bounding shape for collision detection.
    ///
    /// Returns a `Shape` representing the collision boundary
    /// of this element. Used for horizontal spacing and autoplace.
    ///
    /// # Errors
    ///
    /// Returns an error if the shape cannot be computed.
    fn shape(&self, ctx: &LayoutContext) -> Result<Shape>;

    /// Get natural width of this element (before stretching).
    ///
    /// Default implementation uses the shape's bounding box width.
    ///
    /// # Errors
    ///
    /// Returns an error if the width cannot be computed.
    fn natural_width(&self, ctx: &LayoutContext) -> Result<f64> {
        Ok(self.shape(ctx)?.bbox().width())
    }
}

// Element-specific layout modules
pub mod accidentals_layout;
pub mod articulation;
pub mod barline;
pub mod beam_layout;
pub mod brackets;
pub mod chord;
pub mod clef;
pub mod dynamics;
pub mod fermata;
pub mod harmony;
pub mod keysig;
pub mod lyrics;
pub mod measure;
pub mod note;
pub mod rehearsal_mark;
pub mod repeat_signs;
pub mod rest;
pub mod slur_tie;
pub mod system_dividers;
pub mod timesig;
pub mod tuplet;

// Re-exports for convenient access
pub use accidentals_layout::{
    layout_accidentals, AccidentalInfo, AccidentalLayoutConfig, AccidentalPlacement,
};
pub use articulation::{
    layout_articulation, layout_articulations, ArticulationAlign, ArticulationAnchor,
    ArticulationConfig, ArticulationContext, ArticulationInput, ArticulationLayout,
    ArticulationType,
};
pub use barline::{layout_barline, BarlineParams, BarlineType};
pub use beam_layout::{layout_beam, BeamLayout, BeamLayoutConfig, BeamNote};
pub use brackets::{
    brace_magnification, create_brace_path, create_square_bracket_path, layout_bracket,
    layout_brackets, select_brace_glyph, smufl as bracket_smufl, total_brackets_width,
    BracketConfig, BracketInput, BracketLayout, BracketType,
};
pub use chord::{layout_chord, ChordNote, ChordParams, StemDirection};
pub use clef::{layout_clef, ClefOctave, ClefParams, ClefType};
pub use dynamics::{layout_dynamic, DynamicType, DynamicsAlign, DynamicsParams, DynamicsPlacement};
pub use fermata::{
    layout_fermata, layout_fermatas, FermataConfig, FermataInput, FermataLayout, FermataPlacement,
    FermataType,
};
pub use harmony::{
    layout_harmony, musejazz, parse_chord, smufl, ChordNotation, HarmonyLayoutData, HarmonyParams,
    HarmonyStyle, SymbolSet,
};
pub use keysig::{layout_keysig, ClefContext, KeySigParams, KeySigType};
pub use lyrics::{
    layout_lyrics, layout_lyrics_dash, layout_melisma, LyricsParams, LyricsPlacement, SyllabicType,
};
pub use measure::{layout_measure, layout_system, MeasureParams};
pub use note::{layout_note, note_shape, Accidental, NoteDuration, NoteHeadType, NoteParams};
pub use rehearsal_mark::{
    layout_margin_label, layout_rehearsal_mark, layout_section_label, themes as rehearsal_themes,
    MarginLabelParams, RehearsalMarkLayoutData, RehearsalMarkParams, RehearsalMarkStyle,
};
pub use repeat_signs::{
    layout_jump, layout_jumps, layout_marker, layout_markers, smufl as repeat_smufl, JumpInput,
    JumpLayout, JumpType, MarkerInput, MarkerLayout, MarkerType, RepeatPlacement, RepeatSignConfig,
};
pub use rest::{layout_multi_measure_rest, layout_rest, RestDuration, RestParams};
pub use slur_tie::{
    layout_slur, layout_slur_with_obstacles, layout_tie, layout_tie_with_obstacles, ObstacleType,
    SlurControlPoints, SlurDirection, SlurEndpoint, SlurObstacle, SlurStyle, SlurTieConfig,
    SlurTieLayout,
};
pub use system_dividers::{
    layout_system_divider, layout_system_dividers, smufl as divider_smufl, system_divider_width,
    DividerSide, SystemDividerConfig, SystemDividerInput, SystemDividerLayout, SystemDividerStyle,
};
pub use timesig::{layout_timesig, TimeSigParams, TimeSigType};
pub use tuplet::{
    layout_tuplet, TupletBracketType, TupletConfig, TupletLayout, TupletNote, TupletNumberType,
    TupletRatio,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_data_new() {
        let pos = Point::new(10.0, 20.0);
        let bbox = Rect::new(10.0, 20.0, 50.0, 40.0);
        let shape = Shape::from_rect(bbox);

        let data = LayoutData::new(pos, bbox, shape);

        assert_eq!(data.position, pos);
        assert_eq!(data.bbox, bbox);
        assert_eq!(data.children.len(), 0);
    }

    #[test]
    fn test_layout_data_translate() {
        let pos = Point::new(10.0, 20.0);
        let bbox = Rect::new(10.0, 20.0, 50.0, 40.0);
        let shape = Shape::from_rect(bbox);
        let data = LayoutData::new(pos, bbox, shape);

        let offset = Point::new(5.0, 3.0);
        let translated = data.translate(offset);

        assert_eq!(translated.position, Point::new(15.0, 23.0));
        assert_eq!(translated.bbox.x0, 15.0);
        assert_eq!(translated.bbox.y0, 23.0);
    }
}
