//! What is selected on a chart, and how a click changes it: MuseScore's
//! selection (`engraving/dom/select.h`, `NotationInteraction::select`).
//!
//! - **A click** selects one element, and is the anchor of a range
//!   (`SelectType::SINGLE`).
//! - **Shift-click** selects everything from the anchor to the element
//!   clicked, in reading order (`SelectType::RANGE`).
//! - **Cmd-click** (Ctrl off a Mac) adds the element, or takes it off if
//!   it was selected; a range becomes a list of what it held
//!   (`SelectType::ADD`, `SelState::LIST`).
//! - **A box dragged** on the paper selects everything it touches (the
//!   lasso), or adds it with Shift or Cmd held.
//! - **A click on empty paper** selects nothing.
//!
//! Pure: it holds ids, and is told the reading order when it needs it.

/// What kind of selection it is, as MuseScore's `SelState`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    /// Nothing is selected.
    #[default]
    None,
    /// One element, or a range: everything from the anchor to one end.
    Range,
    /// Elements chosen one by one (Cmd-click, a lasso).
    List,
}

/// The selected elements, in the order they were chosen (a range's in
/// reading order), and the anchor a Shift-click extends from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection<Id> {
    items: Vec<Id>,
    anchor: Option<Id>,
    state: State,
}

impl<Id> Default for Selection<Id> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            anchor: None,
            state: State::None,
        }
    }
}

impl<Id: Clone + PartialEq> Selection<Id> {
    /// What is selected.
    #[must_use]
    pub fn items(&self) -> &[Id] {
        &self.items
    }

    #[must_use]
    pub fn contains(&self, id: &Id) -> bool {
        self.items.contains(id)
    }

    #[must_use]
    pub const fn state(&self) -> State {
        self.state
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Selects nothing.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// A click on `id`: it alone, the new anchor.
    pub fn click(&mut self, id: Id) {
        self.items = vec![id.clone()];
        self.anchor = Some(id);
        self.state = State::Range;
    }

    /// A Shift-click on `id`: everything from the anchor to it, in
    /// `order` (the chart's reading order). With no anchor, or one not
    /// in `order`, a click.
    pub fn extend(&mut self, id: Id, order: &[Id]) {
        let at = |x: &Id| order.iter().position(|o| o == x);
        let (Some(from), Some(to)) = (self.anchor.as_ref().and_then(at), at(&id)) else {
            self.click(id);
            return;
        };
        let (lo, hi) = (from.min(to), from.max(to));
        self.items = order[lo..=hi].to_vec();
        self.state = State::Range;
    }

    /// A Cmd-click on `id`: added, or taken off if it was selected. The
    /// anchor moves to an element added.
    pub fn toggle(&mut self, id: Id) {
        if let Some(i) = self.items.iter().position(|x| *x == id) {
            self.items.remove(i);
            if self.anchor.as_ref() == Some(&id) {
                self.anchor = self.items.last().cloned();
            }
        } else {
            self.items.push(id.clone());
            self.anchor = Some(id);
        }
        self.state = if self.items.is_empty() {
            State::None
        } else {
            State::List
        };
    }

    /// A box dragged over `ids`: they alone, or (`add`) with what was
    /// selected.
    pub fn lasso(&mut self, ids: impl IntoIterator<Item = Id>, add: bool) {
        if !add {
            self.items.clear();
        }
        for id in ids {
            if !self.items.contains(&id) {
                self.items.push(id);
            }
        }
        self.anchor = self.items.first().cloned();
        self.state = if self.items.is_empty() {
            State::None
        } else {
            State::List
        };
    }
}

/// How a press with these modifiers selects, as MuseScore's view does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    /// No modifier: the element alone.
    Single,
    /// Shift: a range to it.
    Range,
    /// Cmd (Ctrl off a Mac): it added or taken off.
    Toggle,
}

impl Gesture {
    /// The gesture of a press with `shift` and `command` (Cmd on a Mac,
    /// Ctrl elsewhere) held. Cmd wins when both are.
    #[must_use]
    pub const fn of(shift: bool, command: bool) -> Self {
        if command {
            Self::Toggle
        } else if shift {
            Self::Range
        } else {
            Self::Single
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORDER: [u32; 6] = [10, 20, 30, 40, 50, 60];

    #[test]
    fn a_click_selects_one_and_anchors_a_range() {
        let mut s = Selection::default();
        s.click(30);
        assert_eq!(s.items(), [30]);
        assert_eq!(s.state(), State::Range);
        s.extend(50, &ORDER);
        assert_eq!(s.items(), [30, 40, 50], "anchor to the click, in reading order");
        s.extend(10, &ORDER);
        assert_eq!(s.items(), [10, 20, 30], "back past the anchor");
    }

    #[test]
    fn shift_with_nothing_selected_is_a_click() {
        let mut s = Selection::default();
        s.extend(40, &ORDER);
        assert_eq!(s.items(), [40]);
    }

    #[test]
    fn cmd_click_adds_and_takes_off_and_a_range_becomes_a_list() {
        let mut s = Selection::default();
        s.click(20);
        s.extend(40, &ORDER);
        s.toggle(30);
        assert_eq!(s.items(), [20, 40], "taken out of the range");
        assert_eq!(s.state(), State::List);
        s.toggle(60);
        assert_eq!(s.items(), [20, 40, 60]);
        s.toggle(20);
        s.toggle(40);
        s.toggle(60);
        assert!(s.is_empty());
        assert_eq!(s.state(), State::None);
    }

    #[test]
    fn a_lasso_replaces_or_adds() {
        let mut s = Selection::default();
        s.click(10);
        s.lasso([30, 40], false);
        assert_eq!(s.items(), [30, 40]);
        s.lasso([40, 50], true);
        assert_eq!(s.items(), [30, 40, 50], "added, without doubles");
        s.lasso([], false);
        assert!(s.is_empty());
    }

    #[test]
    fn modifiers_pick_the_gesture() {
        assert_eq!(Gesture::of(false, false), Gesture::Single);
        assert_eq!(Gesture::of(true, false), Gesture::Range);
        assert_eq!(Gesture::of(false, true), Gesture::Toggle);
        assert_eq!(Gesture::of(true, true), Gesture::Toggle);
    }
}
