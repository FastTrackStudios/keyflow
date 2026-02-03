//! Layout-related model types for score rendering.
//!
//! This module contains types related to how music is laid out on the page,
//! including line/page breaks, system organization, and rehearsal marks.

use serde::{Deserialize, Serialize};

/// A rehearsal mark (section marker) in a score.
///
/// Rehearsal marks are used to:
/// - Label sections (Intro, Verse, Chorus, Bridge, etc.)
/// - Force line breaks at section boundaries
/// - Provide navigation points for musicians
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RehearsalMark {
    /// The text to display (e.g., "A", "Verse", "Chorus")
    pub text: String,
    /// Whether this mark forces a line break
    pub forces_break: bool,
    /// Style of the rehearsal mark
    pub style: RehearsalMarkStyle,
}

impl RehearsalMark {
    /// Create a new rehearsal mark with the given text.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            forces_break: true, // By default, section markers force breaks
            style: RehearsalMarkStyle::default(),
        }
    }

    /// Create a section marker (Intro, Verse, etc.)
    #[must_use]
    pub fn section(name: impl Into<String>) -> Self {
        Self::new(name)
    }

    /// Create a simple letter marker (A, B, C, etc.)
    #[must_use]
    pub fn letter(letter: char) -> Self {
        Self {
            text: letter.to_string(),
            forces_break: true,
            style: RehearsalMarkStyle::Boxed,
        }
    }

    /// Set whether this mark forces a line break.
    #[must_use]
    pub fn with_break(mut self, forces_break: bool) -> Self {
        self.forces_break = forces_break;
        self
    }

    /// Set the style of the rehearsal mark.
    #[must_use]
    pub fn with_style(mut self, style: RehearsalMarkStyle) -> Self {
        self.style = style;
        self
    }
}

/// Style options for rehearsal marks.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum RehearsalMarkStyle {
    /// No box, just text
    #[default]
    Plain,
    /// Text in a rectangular box
    Boxed,
    /// Text in a rounded rectangle (capsule)
    Capsule,
    /// Text in a circle (for single letters/numbers)
    Circle,
}

/// A layout break indicating where to start a new line, page, or section.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LayoutBreak {
    /// Force a line (system) break after this measure
    Line,
    /// Force a page break after this measure
    Page,
    /// Section break (implies line break, also affects spacing)
    Section,
    /// Prevent automatic breaks at this point
    NoBreak,
}

/// Policy for determining line breaks in a score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LineBreakPolicy {
    /// Automatic line breaks based on available width
    Auto,
    /// Fixed number of measures per line (unless a section starts)
    FixedMeasuresPerLine {
        /// Number of measures per line
        measures: u32,
        /// Whether to break at section markers regardless of measure count
        break_at_sections: bool,
    },
    /// Break only at explicit markers and section boundaries
    SectionBased,
    /// Break only at explicit markers
    ExplicitOnly,
}

impl Default for LineBreakPolicy {
    fn default() -> Self {
        Self::FixedMeasuresPerLine {
            measures: 4,
            break_at_sections: true,
        }
    }
}

impl LineBreakPolicy {
    /// Create a policy with 4 measures per line, breaking at sections.
    #[must_use]
    pub fn four_per_line() -> Self {
        Self::FixedMeasuresPerLine {
            measures: 4,
            break_at_sections: true,
        }
    }

    /// Create a policy with a custom number of measures per line.
    #[must_use]
    pub fn measures_per_line(count: u32) -> Self {
        Self::FixedMeasuresPerLine {
            measures: count,
            break_at_sections: true,
        }
    }

    /// Create an auto-layout policy (fill to width).
    #[must_use]
    pub fn auto() -> Self {
        Self::Auto
    }
}

/// Information about a system (line of music).
///
/// A system is a horizontal grouping of measures that spans one line
/// across the page, potentially including multiple staves.
#[derive(Debug, Clone, Default)]
pub struct SystemInfo {
    /// Index of the first measure in this system (0-based)
    pub start_measure: usize,
    /// Number of measures in this system
    pub measure_count: usize,
    /// Whether this is the first system of a section
    pub is_section_start: bool,
    /// The rehearsal mark at the start of this system, if any
    pub rehearsal_mark: Option<RehearsalMark>,
    /// Whether this system starts a new page
    pub starts_new_page: bool,
}

impl SystemInfo {
    /// Create a new system info.
    #[must_use]
    pub fn new(start_measure: usize, measure_count: usize) -> Self {
        Self {
            start_measure,
            measure_count,
            is_section_start: false,
            rehearsal_mark: None,
            starts_new_page: false,
        }
    }

    /// Get the index of the last measure in this system.
    #[must_use]
    pub fn end_measure(&self) -> usize {
        self.start_measure + self.measure_count.saturating_sub(1)
    }
}

/// Result of laying out measures into systems.
#[derive(Debug, Clone, Default)]
pub struct SystemLayout {
    /// Information about each system
    pub systems: Vec<SystemInfo>,
    /// Total number of measures
    pub total_measures: usize,
}

/// Information about a page in the score.
#[derive(Debug, Clone, Default)]
pub struct PageInfo {
    /// Page number (1-indexed for display)
    pub page_number: usize,
    /// Index of the first system on this page
    pub first_system_index: usize,
    /// Number of systems on this page
    pub system_count: usize,
}

impl PageInfo {
    /// Create a new page info.
    #[must_use]
    pub fn new(page_number: usize, first_system_index: usize, system_count: usize) -> Self {
        Self {
            page_number,
            first_system_index,
            system_count,
        }
    }

    /// Get the index of the last system on this page.
    #[must_use]
    pub fn last_system_index(&self) -> usize {
        self.first_system_index + self.system_count.saturating_sub(1)
    }
}

/// Configuration for page layout calculations.
#[derive(Debug, Clone, Copy)]
pub struct PageLayoutConfig {
    /// Available content height in pixels (page height minus margins)
    pub content_height: f32,
    /// Height of a single system (5-line staff) in pixels
    pub system_height: f32,
    /// Vertical spacing between systems in pixels
    pub system_spacing: f32,
    /// Extra spacing before section starts in pixels
    pub section_extra_spacing: f32,
    /// Top padding before first system in pixels
    pub top_padding: f32,
    /// Height reserved for header on first page (0.0 if no header)
    pub first_page_header_height: f32,
    /// Height reserved for footer on each page (0.0 if no footer)
    pub footer_height: f32,

    // MuseScore-style vertical spread options
    /// Whether to spread systems to fill the page vertically
    pub enable_vertical_spread: bool,
    /// Minimum spread distance between systems (in pixels)
    pub min_system_spread: f32,
    /// Maximum spread distance between systems (in pixels)
    pub max_system_spread: f32,
    /// Whether to align first/last systems to margins
    pub align_to_margins: bool,
}

impl Default for PageLayoutConfig {
    fn default() -> Self {
        Self {
            content_height: 900.0,
            system_height: 40.0,
            system_spacing: 60.0,
            section_extra_spacing: 20.0,
            top_padding: 40.0,
            first_page_header_height: 0.0,
            footer_height: 0.0,
            // Spread defaults (disabled by default)
            enable_vertical_spread: false,
            min_system_spread: 40.0,
            max_system_spread: 160.0,
            align_to_margins: true,
        }
    }
}

/// Computed bounds for a single page's content area.
///
/// This struct provides the exact pixel coordinates where content can be rendered
/// on a page, accounting for margins, headers, and footers.
#[derive(Debug, Clone, Copy)]
pub struct PageContentBounds {
    /// Left edge of content area (after left margin)
    pub left: f32,
    /// Right edge of content area (before right margin)
    pub right: f32,
    /// Top edge where systems can start (after margin, header, and padding)
    pub top: f32,
    /// Bottom edge where content must stop (before footer and margin)
    pub bottom: f32,
}

impl PageContentBounds {
    /// Create bounds for a page.
    #[must_use]
    pub fn new(left: f32, right: f32, top: f32, bottom: f32) -> Self {
        Self {
            left,
            right,
            top,
            bottom,
        }
    }

    /// Get the content width.
    #[must_use]
    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    /// Get the content height.
    #[must_use]
    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    /// Check if a Y position is within the content bounds.
    #[must_use]
    pub fn y_in_bounds(&self, y: f32) -> bool {
        y >= self.top && y <= self.bottom
    }

    /// Check if a rectangle is fully within the content bounds.
    #[must_use]
    pub fn rect_in_bounds(&self, x: f32, y: f32, width: f32, height: f32) -> bool {
        x >= self.left && (x + width) <= self.right && y >= self.top && (y + height) <= self.bottom
    }

    /// Clamp a Y position to within the content bounds.
    #[must_use]
    pub fn clamp_y(&self, y: f32) -> f32 {
        y.clamp(self.top, self.bottom)
    }
}

/// Result of laying out systems across pages.
#[derive(Debug, Clone, Default)]
pub struct PageLayout {
    /// Information about each page
    pub pages: Vec<PageInfo>,
    /// The system layout this was computed from
    pub system_layout: SystemLayout,
}

impl PageLayout {
    /// Create a new page layout from a system layout.
    #[must_use]
    pub fn new(system_layout: SystemLayout) -> Self {
        Self {
            pages: Vec::new(),
            system_layout,
        }
    }

    /// Get the number of pages.
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Get information about a specific page.
    #[must_use]
    pub fn get_page(&self, index: usize) -> Option<&PageInfo> {
        self.pages.get(index)
    }

    /// Find which page a system belongs to.
    #[must_use]
    pub fn page_for_system(&self, system_index: usize) -> Option<usize> {
        self.pages.iter().position(|page| {
            system_index >= page.first_system_index && system_index <= page.last_system_index()
        })
    }

    /// Get systems for a specific page.
    #[must_use]
    pub fn systems_on_page(&self, page_index: usize) -> Option<&[SystemInfo]> {
        let page = self.pages.get(page_index)?;
        let start = page.first_system_index;
        let end = start + page.system_count;
        self.system_layout.systems.get(start..end)
    }
}

impl SystemLayout {
    /// Create a new empty system layout.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a system layout with the given number of measures per line.
    ///
    /// This is a simple algorithm that doesn't consider section breaks.
    /// For section-aware layout, use `from_score_with_policy`.
    #[must_use]
    pub fn fixed_measures_per_line(total_measures: usize, measures_per_line: usize) -> Self {
        let mut systems = Vec::new();
        let mut current = 0;

        while current < total_measures {
            let count = (total_measures - current).min(measures_per_line);
            systems.push(SystemInfo::new(current, count));
            current += count;
        }

        Self {
            systems,
            total_measures,
        }
    }

    /// Get the number of systems.
    #[must_use]
    pub fn system_count(&self) -> usize {
        self.systems.len()
    }

    /// Get information about a specific system.
    #[must_use]
    pub fn get_system(&self, index: usize) -> Option<&SystemInfo> {
        self.systems.get(index)
    }

    /// Find which system a measure belongs to.
    #[must_use]
    pub fn system_for_measure(&self, measure_index: usize) -> Option<usize> {
        self.systems.iter().position(|sys| {
            measure_index >= sys.start_measure && measure_index <= sys.end_measure()
        })
    }
}

/// Compute system layout from measures with rehearsal marks.
///
/// # Arguments
/// * `total_measures` - Total number of measures in the score
/// * `section_starts` - Indices of measures that start new sections (have rehearsal marks)
/// * `policy` - The line break policy to use
///
/// # Returns
/// A `SystemLayout` describing how measures are grouped into systems
pub fn compute_system_layout(
    total_measures: usize,
    section_starts: &[usize],
    policy: &LineBreakPolicy,
) -> SystemLayout {
    if total_measures == 0 {
        return SystemLayout::default();
    }

    match policy {
        LineBreakPolicy::Auto => {
            // For now, auto just uses 4 per line
            // In the future, this would compute based on width
            SystemLayout::fixed_measures_per_line(total_measures, 4)
        }
        LineBreakPolicy::FixedMeasuresPerLine {
            measures,
            break_at_sections,
        } => compute_fixed_with_sections(
            total_measures,
            section_starts,
            *measures as usize,
            *break_at_sections,
        ),
        LineBreakPolicy::SectionBased => {
            compute_section_based_layout(total_measures, section_starts)
        }
        LineBreakPolicy::ExplicitOnly => {
            // One big system (in practice, explicit breaks would be handled separately)
            SystemLayout {
                systems: vec![SystemInfo::new(0, total_measures)],
                total_measures,
            }
        }
    }
}

/// Compute layout with fixed measures per line, breaking at sections.
fn compute_fixed_with_sections(
    total_measures: usize,
    section_starts: &[usize],
    measures_per_line: usize,
    break_at_sections: bool,
) -> SystemLayout {
    let mut systems: Vec<SystemInfo> = Vec::new();
    let mut current = 0;
    let mut measures_in_current_line = 0;

    while current < total_measures {
        // Check if we're at a section start
        let at_section_start = section_starts.contains(&current);

        // Determine if we need to start a new system
        let need_new_system = systems.is_empty()
            || (break_at_sections && at_section_start && measures_in_current_line > 0)
            || measures_in_current_line >= measures_per_line;

        if need_new_system && !systems.is_empty() {
            // Finalize the previous system with its measure count
            if let Some(last) = systems.last_mut() {
                last.measure_count = measures_in_current_line;
            }
            measures_in_current_line = 0;
        }

        if systems.is_empty() || need_new_system {
            let mut sys = SystemInfo::new(current, 0);
            sys.is_section_start = at_section_start;
            systems.push(sys);
        }

        current += 1;
        measures_in_current_line += 1;
    }

    // Finalize the last system
    if let Some(last) = systems.last_mut() {
        last.measure_count = measures_in_current_line;
    }

    SystemLayout {
        systems,
        total_measures,
    }
}

/// Compute layout based purely on section boundaries.
fn compute_section_based_layout(total_measures: usize, section_starts: &[usize]) -> SystemLayout {
    if section_starts.is_empty() {
        return SystemLayout {
            systems: vec![SystemInfo::new(0, total_measures)],
            total_measures,
        };
    }

    let mut systems = Vec::new();
    let mut sorted_starts: Vec<usize> = section_starts.to_vec();
    sorted_starts.sort_unstable();

    // Add 0 if not present (start of score)
    if sorted_starts.first() != Some(&0) {
        sorted_starts.insert(0, 0);
    }

    for (i, &start) in sorted_starts.iter().enumerate() {
        let end = sorted_starts.get(i + 1).copied().unwrap_or(total_measures);
        let mut sys = SystemInfo::new(start, end - start);
        sys.is_section_start = i > 0 || section_starts.contains(&0);
        systems.push(sys);
    }

    SystemLayout {
        systems,
        total_measures,
    }
}

/// Compute page layout from a system layout.
///
/// This function takes a system layout and computes which systems should go
/// on each page based on the available content height and system sizes.
/// It follows the MuseScore approach: accumulate systems onto a page until
/// the next system wouldn't fit, then start a new page.
///
/// The first page accounts for header height, and all pages account for footer height.
///
/// # Arguments
/// * `system_layout` - The system layout to paginate
/// * `config` - Configuration for page dimensions and spacing
///
/// # Returns
/// A `PageLayout` describing how systems are distributed across pages
pub fn compute_page_layout(system_layout: SystemLayout, config: &PageLayoutConfig) -> PageLayout {
    if system_layout.systems.is_empty() {
        return PageLayout {
            pages: vec![],
            system_layout,
        };
    }

    let mut pages: Vec<PageInfo> = Vec::new();
    let mut current_page_start = 0;
    let mut systems_on_current_page = 0;

    // Calculate effective content height for first page (accounting for header)
    let first_page_available =
        config.content_height - config.first_page_header_height - config.footer_height;

    // Calculate effective content height for subsequent pages (no header)
    let other_pages_available = config.content_height - config.footer_height;

    // Start position accounting for header on first page
    let mut current_y = config.top_padding;
    let mut current_page_available = first_page_available;

    for (sys_idx, sys_info) in system_layout.systems.iter().enumerate() {
        // Calculate the height this system will require
        let extra_spacing =
            if sys_info.is_section_start && sys_idx > 0 && systems_on_current_page > 0 {
                config.section_extra_spacing
            } else {
                0.0
            };

        let spacing = if systems_on_current_page > 0 {
            config.system_spacing
        } else {
            0.0
        };

        let system_total_height = extra_spacing + spacing + config.system_height;

        // Check if this system fits on the current page
        let would_exceed = current_y + system_total_height > current_page_available;

        if would_exceed && systems_on_current_page > 0 {
            // Finalize the current page
            pages.push(PageInfo::new(
                pages.len() + 1,
                current_page_start,
                systems_on_current_page,
            ));

            // Start a new page (no header on subsequent pages)
            current_page_start = sys_idx;
            current_y = config.top_padding + config.system_height;
            current_page_available = other_pages_available;
            systems_on_current_page = 1;
        } else {
            // Add system to current page
            current_y += system_total_height;
            systems_on_current_page += 1;
        }
    }

    // Finalize the last page
    if systems_on_current_page > 0 {
        pages.push(PageInfo::new(
            pages.len() + 1,
            current_page_start,
            systems_on_current_page,
        ));
    }

    PageLayout {
        pages,
        system_layout,
    }
}

/// Compute page layout from a system layout, also marking which systems start new pages.
///
/// This is similar to `compute_page_layout` but also updates the `starts_new_page` flag
/// on each SystemInfo.
///
/// # Arguments
/// * `system_layout` - The system layout to paginate (will be modified)
/// * `config` - Configuration for page dimensions and spacing
///
/// # Returns
/// A `PageLayout` describing how systems are distributed across pages
pub fn compute_page_layout_mut(
    mut system_layout: SystemLayout,
    config: &PageLayoutConfig,
) -> PageLayout {
    if system_layout.systems.is_empty() {
        return PageLayout {
            pages: vec![],
            system_layout,
        };
    }

    let mut pages: Vec<PageInfo> = Vec::new();
    let mut current_page_start = 0;
    let mut systems_on_current_page = 0;

    // Calculate effective content height for first page (accounting for header)
    let first_page_available =
        config.content_height - config.first_page_header_height - config.footer_height;

    // Calculate effective content height for subsequent pages (no header)
    let other_pages_available = config.content_height - config.footer_height;

    // Start position accounting for header on first page
    let mut current_y = config.top_padding;
    let mut current_page_available = first_page_available;

    for sys_idx in 0..system_layout.systems.len() {
        let sys_info = &system_layout.systems[sys_idx];

        // Calculate the height this system will require
        let extra_spacing =
            if sys_info.is_section_start && sys_idx > 0 && systems_on_current_page > 0 {
                config.section_extra_spacing
            } else {
                0.0
            };

        let spacing = if systems_on_current_page > 0 {
            config.system_spacing
        } else {
            0.0
        };

        let system_total_height = extra_spacing + spacing + config.system_height;

        // Check if this system fits on the current page
        let would_exceed = current_y + system_total_height > current_page_available;

        if would_exceed && systems_on_current_page > 0 {
            // Finalize the current page
            pages.push(PageInfo::new(
                pages.len() + 1,
                current_page_start,
                systems_on_current_page,
            ));

            // Mark this system as starting a new page
            system_layout.systems[sys_idx].starts_new_page = true;

            // Start a new page (no header on subsequent pages)
            current_page_start = sys_idx;
            current_y = config.top_padding + config.system_height;
            current_page_available = other_pages_available;
            systems_on_current_page = 1;
        } else {
            // Add system to current page
            current_y += system_total_height;
            systems_on_current_page += 1;
        }
    }

    // Finalize the last page
    if systems_on_current_page > 0 {
        pages.push(PageInfo::new(
            pages.len() + 1,
            current_page_start,
            systems_on_current_page,
        ));
    }

    PageLayout {
        pages,
        system_layout,
    }
}

/// Y position for a system on a page.
#[derive(Debug, Clone, Copy)]
pub struct SystemYPosition {
    /// Index of the system
    pub system_index: usize,
    /// Y position of the top of the system (staff top line)
    pub y: f32,
    /// Height of the system
    pub height: f32,
}

/// Distribute systems to fill a page vertically (MuseScore's spreadPage algorithm).
///
/// This function takes the systems on a single page and calculates their Y positions
/// such that they are evenly distributed to fill the available content area.
///
/// # Arguments
/// * `system_indices` - Indices of systems on this page
/// * `content_top` - Top of content area (Y coordinate)
/// * `content_bottom` - Bottom of content area (Y coordinate)
/// * `system_height` - Height of each system
/// * `config` - Layout configuration with spread settings
///
/// # Returns
/// Vector of Y positions for each system
pub fn spread_systems_on_page(
    system_indices: &[usize],
    content_top: f32,
    content_bottom: f32,
    system_height: f32,
    config: &PageLayoutConfig,
) -> Vec<SystemYPosition> {
    if system_indices.is_empty() {
        return Vec::new();
    }

    let system_count = system_indices.len();
    let available_height = content_bottom - content_top;
    let total_system_height = system_height * system_count as f32;

    // If spread is disabled, use fixed spacing
    if !config.enable_vertical_spread {
        return system_indices
            .iter()
            .enumerate()
            .map(|(i, &sys_idx)| {
                let y = content_top
                    + config.top_padding
                    + (i as f32 * (system_height + config.system_spacing));
                SystemYPosition {
                    system_index: sys_idx,
                    y,
                    height: system_height,
                }
            })
            .collect();
    }

    // Only one system - center it or align to top
    if system_count == 1 {
        let y = if config.align_to_margins {
            content_top + config.top_padding
        } else {
            content_top + (available_height - system_height) / 2.0
        };
        return vec![SystemYPosition {
            system_index: system_indices[0],
            y,
            height: system_height,
        }];
    }

    // Calculate gaps between systems
    let gaps = system_count - 1;
    let available_for_gaps = available_height - total_system_height - config.top_padding;

    // Calculate spread amount per gap, respecting min/max
    let spread_per_gap = if gaps > 0 {
        (available_for_gaps / gaps as f32).clamp(config.min_system_spread, config.max_system_spread)
    } else {
        config.system_spacing
    };

    // If we can't achieve good spread, fall back to fixed spacing
    let actual_total = config.top_padding + total_system_height + (spread_per_gap * gaps as f32);
    let start_y = if config.align_to_margins {
        content_top + config.top_padding
    } else {
        // Center the content block
        content_top + (available_height - actual_total) / 2.0 + config.top_padding
    };

    // Calculate Y position for each system
    let mut current_y = start_y;
    system_indices
        .iter()
        .map(|&sys_idx| {
            let pos = SystemYPosition {
                system_index: sys_idx,
                y: current_y,
                height: system_height,
            };
            current_y += system_height + spread_per_gap;
            pos
        })
        .collect()
}

/// Calculate Y positions for all systems across all pages.
///
/// This convenience function combines page layout with vertical spreading
/// to produce final Y coordinates for every system in the score.
///
/// # Arguments
/// * `page_layout` - The page layout computed from `compute_page_layout`
/// * `config` - Layout configuration
/// * `content_bounds_per_page` - Function to get content bounds for each page
///
/// # Returns
/// Vector of Y positions for all systems
pub fn compute_all_system_y_positions<F>(
    page_layout: &PageLayout,
    config: &PageLayoutConfig,
    content_bounds_per_page: F,
) -> Vec<SystemYPosition>
where
    F: Fn(usize) -> PageContentBounds,
{
    let mut all_positions = Vec::new();

    for (page_idx, page) in page_layout.pages.iter().enumerate() {
        let bounds = content_bounds_per_page(page_idx);

        // Get system indices for this page
        let system_indices: Vec<usize> =
            (page.first_system_index..page.first_system_index + page.system_count).collect();

        let positions = spread_systems_on_page(
            &system_indices,
            bounds.top,
            bounds.bottom,
            config.system_height,
            config,
        );

        all_positions.extend(positions);
    }

    all_positions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_measures_per_line() {
        let layout = SystemLayout::fixed_measures_per_line(16, 4);
        assert_eq!(layout.system_count(), 4);
        assert_eq!(layout.systems[0].start_measure, 0);
        assert_eq!(layout.systems[0].measure_count, 4);
        assert_eq!(layout.systems[3].start_measure, 12);
        assert_eq!(layout.systems[3].measure_count, 4);
    }

    #[test]
    fn test_fixed_with_sections() {
        let section_starts = vec![0, 8]; // Intro at 0, Verse at 8
        let layout = compute_system_layout(
            16,
            &section_starts,
            &LineBreakPolicy::FixedMeasuresPerLine {
                measures: 4,
                break_at_sections: true,
            },
        );

        // Should break at measure 8 (section start) even though we're at 4 measures
        // Expected: [0-3], [4-7], [8-11], [12-15]
        assert_eq!(layout.system_count(), 4);
    }

    #[test]
    fn test_section_based_layout() {
        let section_starts = vec![0, 4, 12];
        let layout = compute_system_layout(16, &section_starts, &LineBreakPolicy::SectionBased);

        // Should have 3 systems: [0-3], [4-11], [12-15]
        assert_eq!(layout.system_count(), 3);
        assert_eq!(layout.systems[0].measure_count, 4);
        assert_eq!(layout.systems[1].measure_count, 8);
        assert_eq!(layout.systems[2].measure_count, 4);
    }

    #[test]
    fn test_page_layout_single_page() {
        // Create a system layout with 4 systems
        let system_layout = SystemLayout::fixed_measures_per_line(16, 4);

        // Config that fits all systems on one page
        let config = PageLayoutConfig {
            content_height: 500.0,
            system_height: 40.0,
            system_spacing: 60.0,
            section_extra_spacing: 20.0,
            top_padding: 40.0,
            ..Default::default()
        };

        let page_layout = compute_page_layout(system_layout, &config);

        assert_eq!(page_layout.page_count(), 1);
        assert_eq!(page_layout.pages[0].system_count, 4);
    }

    #[test]
    fn test_page_layout_multiple_pages() {
        // Create a system layout with 8 systems
        let system_layout = SystemLayout::fixed_measures_per_line(32, 4);

        // Config that only fits 2 systems per page
        // First system: top_padding (40) + system_height (40) = 80
        // Each additional: system_spacing (60) + system_height (40) = 100
        // 1 system: 80
        // 2 systems: 80 + 100 = 180
        // 3 systems: 180 + 100 = 280 > 250 (doesn't fit)
        let config = PageLayoutConfig {
            content_height: 250.0,
            system_height: 40.0,
            system_spacing: 60.0,
            section_extra_spacing: 20.0,
            top_padding: 40.0,
            ..Default::default()
        };

        let page_layout = compute_page_layout(system_layout, &config);

        // 8 systems, 2 per page = 4 pages
        assert_eq!(page_layout.page_count(), 4);
        assert_eq!(page_layout.pages[0].system_count, 2);
        assert_eq!(page_layout.pages[1].system_count, 2);
        assert_eq!(page_layout.pages[2].system_count, 2);
        assert_eq!(page_layout.pages[3].system_count, 2);
    }

    #[test]
    fn test_page_layout_with_sections() {
        // Create a system layout with sections
        let section_starts = vec![0, 8];
        let system_layout = compute_system_layout(
            32,
            &section_starts,
            &LineBreakPolicy::FixedMeasuresPerLine {
                measures: 4,
                break_at_sections: true,
            },
        );

        // Config with section extra spacing
        let config = PageLayoutConfig {
            content_height: 400.0,
            system_height: 40.0,
            system_spacing: 60.0,
            section_extra_spacing: 30.0, // Extra spacing for sections
            top_padding: 40.0,
            ..Default::default()
        };

        let page_layout = compute_page_layout(system_layout, &config);

        // Should create multiple pages
        assert!(page_layout.page_count() >= 2);
    }

    #[test]
    fn test_page_for_system() {
        let system_layout = SystemLayout::fixed_measures_per_line(32, 4);

        // Config with 2 systems per page
        let config = PageLayoutConfig {
            content_height: 250.0,
            system_height: 40.0,
            system_spacing: 60.0,
            section_extra_spacing: 20.0,
            top_padding: 40.0,
            ..Default::default()
        };

        let page_layout = compute_page_layout(system_layout, &config);

        // First 2 systems on page 0
        assert_eq!(page_layout.page_for_system(0), Some(0));
        assert_eq!(page_layout.page_for_system(1), Some(0));

        // Systems 2-3 on page 1
        assert_eq!(page_layout.page_for_system(2), Some(1));
        assert_eq!(page_layout.page_for_system(3), Some(1));

        // Systems 4-5 on page 2
        assert_eq!(page_layout.page_for_system(4), Some(2));
        assert_eq!(page_layout.page_for_system(5), Some(2));

        // Systems 6-7 on page 3
        assert_eq!(page_layout.page_for_system(6), Some(3));
        assert_eq!(page_layout.page_for_system(7), Some(3));
    }

    #[test]
    fn test_systems_on_page() {
        let system_layout = SystemLayout::fixed_measures_per_line(16, 4);

        let config = PageLayoutConfig {
            content_height: 300.0,
            system_height: 40.0,
            system_spacing: 60.0,
            section_extra_spacing: 20.0,
            top_padding: 40.0,
            ..Default::default()
        };

        let page_layout = compute_page_layout(system_layout, &config);

        // Get systems on first page
        let systems_page_0 = page_layout.systems_on_page(0).unwrap();
        assert!(!systems_page_0.is_empty());

        // Each system should have 4 measures
        for sys in systems_page_0 {
            assert_eq!(sys.measure_count, 4);
        }
    }
}
