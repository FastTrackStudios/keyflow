//! Interaction layer for editing and selection.
//!
//! This module handles user interaction with the score:
//! - Hit testing
//! - Selection management
//! - Editing operations
//! - Undo/redo

use crate::engraver::scene::GraphicalObjectId;
use kurbo::Point;

/// Selection state.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    /// Currently selected objects
    pub selected: Vec<GraphicalObjectId>,
}

impl Selection {
    /// Create a new empty selection.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if anything is selected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    /// Clear the selection.
    pub fn clear(&mut self) {
        self.selected.clear();
    }

    /// Select a single object.
    pub fn select(&mut self, id: GraphicalObjectId) {
        self.selected.clear();
        self.selected.push(id);
    }

    /// Add to selection (for multi-select).
    pub fn add(&mut self, id: GraphicalObjectId) {
        if !self.selected.contains(&id) {
            self.selected.push(id);
        }
    }

    /// Toggle selection.
    pub fn toggle(&mut self, id: GraphicalObjectId) {
        if let Some(pos) = self.selected.iter().position(|&x| x == id) {
            self.selected.remove(pos);
        } else {
            self.selected.push(id);
        }
    }
}

/// Cursor position in the score.
#[derive(Debug, Clone)]
pub struct Cursor {
    /// Current position in score coordinates
    pub position: Point,
    /// Whether the cursor is visible
    pub visible: bool,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            position: Point::ZERO,
            visible: true,
        }
    }
}

/// Editing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditMode {
    /// Normal selection mode
    #[default]
    Select,
    /// Note entry mode
    NoteEntry,
    /// Rest entry mode
    RestEntry,
}

/// Interaction state for the editor.
#[derive(Debug, Default)]
pub struct InteractionState {
    /// Current selection
    pub selection: Selection,
    /// Cursor position
    pub cursor: Cursor,
    /// Current edit mode
    pub mode: EditMode,
}

impl InteractionState {
    /// Create a new interaction state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
