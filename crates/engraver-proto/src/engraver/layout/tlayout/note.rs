//! Note layout implementation.
//!
//! Handles layout of individual noteheads, including position on staff,
//! accidentals, dots, and ledger lines.

use kurbo::{Point, Rect};
use vello::peniko::Color;

use crate::engraver::layout::context::LayoutContext;
use crate::engraver::layout::shape::Shape;
use crate::engraver::scene::id::{ElementType, SemanticId};
use crate::engraver::scene::node::SceneNode;
use crate::engraver::scene::paint::PaintCommand;

use super::LayoutData;

/// SMuFL codepoints for noteheads.
pub mod glyphs {
    /// Whole note (semibreve)
    pub const NOTEHEAD_WHOLE: char = '\u{E0A2}';
    /// Half note (minim)
    pub const NOTEHEAD_HALF: char = '\u{E0A3}';
    /// Quarter note and shorter (crotchet)
    pub const NOTEHEAD_BLACK: char = '\u{E0A4}';
    /// Double whole note (breve)
    pub const NOTEHEAD_DOUBLE_WHOLE: char = '\u{E0A0}';

    // Slash noteheads (rhythmic notation)
    /// Slash notehead with vertical ends (filled, for quarter notes and shorter)
    pub const NOTEHEAD_SLASH_VERTICAL: char = '\u{E100}';
    /// Slash notehead with horizontal ends (filled, alternative)
    pub const NOTEHEAD_SLASH_HORIZONTAL: char = '\u{E101}';
    /// White slash whole note
    pub const NOTEHEAD_SLASH_WHITE_WHOLE: char = '\u{E102}';
    /// White slash half note
    pub const NOTEHEAD_SLASH_WHITE_HALF: char = '\u{E103}';
    /// White slash double whole note
    pub const NOTEHEAD_SLASH_WHITE_DOUBLE_WHOLE: char = '\u{E10A}';

    // Diamond noteheads (harmonics, also used for rhythmic notation half/whole)
    /// White diamond whole note
    pub const NOTEHEAD_DIAMOND_WHOLE: char = '\u{E0DB}';
    /// White diamond half note
    pub const NOTEHEAD_DIAMOND_HALF: char = '\u{E0DC}';
    /// Black diamond (filled, for quarter notes and shorter)
    pub const NOTEHEAD_DIAMOND_BLACK: char = '\u{E0DD}';
    /// Large diamond (wide variant)
    pub const NOTEHEAD_DIAMOND_WIDE: char = '\u{E0DE}';
    /// Slash diamond white (large white diamond for rhythmic notation)
    /// Base glyph - use ss08 stylistic set for oversized variant
    pub const NOTEHEAD_SLASH_DIAMOND_WHITE: char = '\u{E104}';

    // Accidentals
    /// Sharp
    pub const ACCIDENTAL_SHARP: char = '\u{E262}';
    /// Flat
    pub const ACCIDENTAL_FLAT: char = '\u{E260}';
    /// Natural
    pub const ACCIDENTAL_NATURAL: char = '\u{E261}';
    /// Double sharp
    pub const ACCIDENTAL_DOUBLE_SHARP: char = '\u{E263}';
    /// Double flat
    pub const ACCIDENTAL_DOUBLE_FLAT: char = '\u{E264}';

    // Augmentation dots
    /// Augmentation dot
    pub const AUGMENTATION_DOT: char = '\u{E1E7}';
}

/// Notehead type/style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NoteHeadType {
    /// Standard notehead (black, half, whole)
    #[default]
    Normal,
    /// Slash notehead for rhythmic notation (jazz charts, lead sheets)
    Slash,
    /// X notehead (percussion, ghost notes)
    X,
    /// Diamond notehead (harmonics)
    Diamond,
    /// Triangle notehead
    Triangle,
}

impl NoteHeadType {
    /// Get the SMuFL glyph for this notehead type and duration.
    #[must_use]
    pub const fn glyph(&self, duration: NoteDuration) -> char {
        match self {
            Self::Normal => duration.notehead_glyph(),
            Self::Slash => match duration {
                // Use large white diamond for half/whole in rhythmic notation
                // U+E104 noteheadSlashDiamondWhite (use ss08 for oversized variant)
                // TODO: Make this configurable (diamond vs white slash)
                NoteDuration::DoubleWhole | NoteDuration::Whole | NoteDuration::Half => {
                    glyphs::NOTEHEAD_SLASH_DIAMOND_WHITE
                }
                // Quarter and shorter use filled slash
                _ => glyphs::NOTEHEAD_SLASH_HORIZONTAL,
            },
            // Fallback to normal noteheads for unimplemented types
            Self::X | Self::Diamond | Self::Triangle => duration.notehead_glyph(),
        }
    }

    /// Get the width of this notehead type in spatiums.
    #[must_use]
    pub const fn width(&self) -> f64 {
        match self {
            Self::Normal => 1.18,
            Self::Slash => 1.5, // Slash noteheads are wider
            Self::X => 1.2,
            Self::Diamond => 1.3,
            Self::Triangle => 1.2,
        }
    }
}

/// Accidental type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accidental {
    None,
    Sharp,
    Flat,
    Natural,
    DoubleSharp,
    DoubleFlat,
}

impl Accidental {
    /// Get the SMuFL glyph for this accidental.
    #[must_use]
    pub const fn glyph(&self) -> Option<char> {
        match self {
            Self::None => None,
            Self::Sharp => Some(glyphs::ACCIDENTAL_SHARP),
            Self::Flat => Some(glyphs::ACCIDENTAL_FLAT),
            Self::Natural => Some(glyphs::ACCIDENTAL_NATURAL),
            Self::DoubleSharp => Some(glyphs::ACCIDENTAL_DOUBLE_SHARP),
            Self::DoubleFlat => Some(glyphs::ACCIDENTAL_DOUBLE_FLAT),
        }
    }

    /// Get the width of this accidental in spatiums.
    #[must_use]
    pub const fn width(&self) -> f64 {
        match self {
            Self::None => 0.0,
            Self::Sharp => 1.2,
            Self::Flat => 0.9,
            Self::Natural => 0.7,
            Self::DoubleSharp => 1.0,
            Self::DoubleFlat => 1.4,
        }
    }
}

/// Duration type for notehead selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteDuration {
    DoubleWhole,
    Whole,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
    SixtyFourth,
}

impl NoteDuration {
    /// Get the SMuFL glyph for this duration's notehead.
    #[must_use]
    pub const fn notehead_glyph(&self) -> char {
        match self {
            Self::DoubleWhole => glyphs::NOTEHEAD_DOUBLE_WHOLE,
            Self::Whole => glyphs::NOTEHEAD_WHOLE,
            Self::Half => glyphs::NOTEHEAD_HALF,
            _ => glyphs::NOTEHEAD_BLACK,
        }
    }

    /// Check if this duration requires a stem.
    #[must_use]
    pub const fn has_stem(&self) -> bool {
        !matches!(self, Self::DoubleWhole | Self::Whole)
    }

    /// Get the number of flags (for eighth notes and shorter).
    #[must_use]
    pub const fn flag_count(&self) -> u8 {
        match self {
            Self::Eighth => 1,
            Self::Sixteenth => 2,
            Self::ThirtySecond => 3,
            Self::SixtyFourth => 4,
            _ => 0,
        }
    }
}

/// Note layout parameters.
#[derive(Debug, Clone)]
pub struct NoteParams {
    /// Unique identifier for this note
    pub id: u64,
    /// Duration type
    pub duration: NoteDuration,
    /// Notehead type/style
    pub head_type: NoteHeadType,
    /// Staff line position (0 = middle line, positive = up)
    pub line: i32,
    /// Accidental to display
    pub accidental: Accidental,
    /// Number of augmentation dots
    pub dots: u8,
    /// Whether this note is part of a chord with offset noteheads
    pub offset_x: f64,
    /// Whether to draw ledger lines
    pub ledger_lines: bool,
}

impl Default for NoteParams {
    fn default() -> Self {
        Self {
            id: 0,
            duration: NoteDuration::Quarter,
            head_type: NoteHeadType::Normal,
            line: 0,
            accidental: Accidental::None,
            dots: 0,
            offset_x: 0.0,
            ledger_lines: true,
        }
    }
}

/// Layout a single note.
///
/// # Returns
/// Tuple of (LayoutData, SceneNode) containing position/shape and visual representation.
#[must_use]
pub fn layout_note(params: &NoteParams, ctx: &LayoutContext) -> (LayoutData, SceneNode) {
    let spatium = ctx.spatium();
    let staff_line_distance = spatium; // Distance between staff lines

    // Calculate Y position from staff line
    // Line 0 = middle line (B4 in treble clef)
    // Positive lines go up, negative go down
    let y = -params.line as f64 * staff_line_distance / 2.0;

    // Start X at 0, adjusted for accidentals
    let mut x = 0.0;

    let mut commands = Vec::new();
    let mut total_width = 0.0;

    // Draw accidental if present
    if let Some(acc_glyph) = params.accidental.glyph() {
        let acc_width = params.accidental.width() * spatium;
        let acc_x = x;

        commands.push(PaintCommand::glyph(
            acc_glyph,
            Point::new(acc_x, y),
            spatium,
            Color::BLACK,
        ));

        x += acc_width + spatium * 0.15; // Small gap after accidental
        total_width += acc_width + spatium * 0.15;
    }

    // Draw notehead
    let notehead_x = x + params.offset_x;
    let notehead_glyph = params.head_type.glyph(params.duration);
    let notehead_width = spatium * params.head_type.width();

    commands.push(PaintCommand::glyph(
        notehead_glyph,
        Point::new(notehead_x, y),
        spatium,
        Color::BLACK,
    ));

    total_width += notehead_width;

    // Draw ledger lines if needed
    if params.ledger_lines {
        let ledger_commands = draw_ledger_lines(params.line, notehead_x, notehead_width, spatium);
        commands.extend(ledger_commands);
    }

    // Draw augmentation dots
    // MuseScore uses dotNoteDistance = 0.5 spatiums (default)
    // And dotDotDistance = 0.5 spatiums between multiple dots
    const DOT_NOTE_DISTANCE: f64 = 0.5; // spatiums
    const DOT_DOT_DISTANCE: f64 = 0.5; // spatiums between adjacent dots
    const DOT_GLYPH_WIDTH: f64 = 0.35; // approximate width of the dot glyph

    if params.dots > 0 {
        let dot_x = notehead_x + notehead_width + spatium * DOT_NOTE_DISTANCE;
        let dot_y = if params.line % 2 == 0 {
            y - staff_line_distance / 4.0 // Move dot up if on a line
        } else {
            y
        };

        for i in 0..params.dots {
            commands.push(PaintCommand::glyph(
                glyphs::AUGMENTATION_DOT,
                Point::new(dot_x + i as f64 * spatium * DOT_DOT_DISTANCE, dot_y),
                spatium,
                Color::BLACK,
            ));
        }

        // Width calculation:
        // - DOT_NOTE_DISTANCE: gap from notehead to first dot
        // - (params.dots - 1) * DOT_DOT_DISTANCE: gaps between adjacent dots
        // - DOT_GLYPH_WIDTH: width of the last dot's glyph
        let dots_spacing = if params.dots > 1 {
            (params.dots - 1) as f64 * DOT_DOT_DISTANCE
        } else {
            0.0
        };
        total_width += spatium * (DOT_NOTE_DISTANCE + dots_spacing + DOT_GLYPH_WIDTH);
    }

    // Calculate bounding box (relative to note position)
    let half_height = spatium * 0.5;
    let bbox = Rect::new(0.0, -half_height, total_width, half_height);

    // Create shape for collision detection (in world coordinates)
    let world_bbox = Rect::new(0.0, y - half_height, total_width, y + half_height);
    let shape = Shape::from_rect(world_bbox);

    // Create layout data with proper position
    let layout = LayoutData::new(Point::new(0.0, y), bbox, shape);

    // Create scene node with semantic ID
    let semantic_id = SemanticId::new(ElementType::Note, params.id);
    let node =
        SceneNode::leaf(semantic_id, commands).with_metadata("pitch-line", params.line.to_string());

    (layout, node)
}

/// Draw ledger lines for notes outside the staff.
fn draw_ledger_lines(
    line: i32,
    notehead_x: f64,
    notehead_width: f64,
    spatium: f64,
) -> Vec<PaintCommand> {
    let mut commands = Vec::new();
    let line_extension = spatium * 0.4; // How far ledger line extends past notehead
    let line_width = spatium * 0.16;
    let staff_line_distance = spatium;

    // Ledger lines above staff (line > 5, i.e., above top line)
    if line > 5 {
        let mut l = 6;
        while l <= line {
            if l % 2 == 0 {
                // Only draw on even lines (actual lines, not spaces)
                let ledger_y = -l as f64 * staff_line_distance / 2.0;
                commands.push(PaintCommand::line(
                    Point::new(notehead_x - line_extension, ledger_y),
                    Point::new(notehead_x + notehead_width + line_extension, ledger_y),
                    Color::BLACK,
                    line_width,
                ));
            }
            l += 1;
        }
    }

    // Ledger lines below staff (line < -5, i.e., below bottom line)
    if line < -5 {
        let mut l = -6;
        while l >= line {
            if l % 2 == 0 {
                let ledger_y = -l as f64 * staff_line_distance / 2.0;
                commands.push(PaintCommand::line(
                    Point::new(notehead_x - line_extension, ledger_y),
                    Point::new(notehead_x + notehead_width + line_extension, ledger_y),
                    Color::BLACK,
                    line_width,
                ));
            }
            l -= 1;
        }
    }

    commands
}

/// Calculate the shape for a note (for collision detection).
#[must_use]
pub fn note_shape(params: &NoteParams, ctx: &LayoutContext) -> Shape {
    let spatium = ctx.spatium();
    let y = -params.line as f64 * spatium / 2.0;
    let half_height = spatium * 0.5;

    let mut width = spatium * params.head_type.width(); // Notehead width based on type
    if params.accidental != Accidental::None {
        width += params.accidental.width() * spatium + spatium * 0.15;
    }
    if params.dots > 0 {
        // Match MuseScore's dotNoteDistance (0.5) + dotDotDistance (0.5)
        width += spatium * 0.5 + params.dots as f64 * spatium * 0.5;
    }

    Shape::from_rect(Rect::new(0.0, y - half_height, width, y + half_height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engraver::layout::context::LayoutConfiguration;
    use crate::engraver::style::MStyle;

    fn test_ctx() -> LayoutContext<'static> {
        let config = LayoutConfiguration::default();
        let style = Box::leak(Box::new(MStyle::default()));
        LayoutContext::new_for_test(config, style)
    }

    #[test]
    fn test_layout_simple_note() {
        let ctx = test_ctx();
        let params = NoteParams {
            id: 1,
            duration: NoteDuration::Quarter,
            line: 0,
            ..Default::default()
        };

        let (layout, node) = layout_note(&params, &ctx);

        assert!(!layout.bbox.is_zero_area());
        assert!(node.id.is_some());
        assert!(!node.commands.is_empty());
    }

    #[test]
    fn test_layout_note_with_accidental() {
        let ctx = test_ctx();
        let params = NoteParams {
            id: 2,
            duration: NoteDuration::Quarter,
            line: 0,
            accidental: Accidental::Sharp,
            ..Default::default()
        };

        let (layout, node) = layout_note(&params, &ctx);

        // Should have at least 2 commands (accidental + notehead)
        assert!(node.commands.len() >= 2);
        // Bounding box should be wider with accidental
        assert!(layout.bbox.width() > ctx.spatium());
    }

    #[test]
    fn test_layout_note_with_dots() {
        let ctx = test_ctx();
        let params = NoteParams {
            id: 3,
            duration: NoteDuration::Half,
            line: 2,
            dots: 2,
            ..Default::default()
        };

        let (layout, node) = layout_note(&params, &ctx);

        // Should have notehead + 2 dots = 3 commands minimum
        assert!(node.commands.len() >= 3);
    }

    #[test]
    fn test_notehead_glyphs() {
        assert_eq!(NoteDuration::Whole.notehead_glyph(), glyphs::NOTEHEAD_WHOLE);
        assert_eq!(NoteDuration::Half.notehead_glyph(), glyphs::NOTEHEAD_HALF);
        assert_eq!(
            NoteDuration::Quarter.notehead_glyph(),
            glyphs::NOTEHEAD_BLACK
        );
        assert_eq!(
            NoteDuration::Eighth.notehead_glyph(),
            glyphs::NOTEHEAD_BLACK
        );
    }

    #[test]
    fn test_stem_required() {
        assert!(!NoteDuration::Whole.has_stem());
        assert!(!NoteDuration::DoubleWhole.has_stem());
        assert!(NoteDuration::Half.has_stem());
        assert!(NoteDuration::Quarter.has_stem());
    }

    #[test]
    fn test_flag_count() {
        assert_eq!(NoteDuration::Quarter.flag_count(), 0);
        assert_eq!(NoteDuration::Eighth.flag_count(), 1);
        assert_eq!(NoteDuration::Sixteenth.flag_count(), 2);
        assert_eq!(NoteDuration::SixtyFourth.flag_count(), 4);
    }
}
