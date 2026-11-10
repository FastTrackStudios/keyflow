//! System breaking: which bars share a line, and how wide a bar must be
//! before it can share one.

use super::{ChartLayoutEngine, symbol_spacing, width_dist};
use crate::engraver::layout::text_metrics::TextFontMetrics;
use crate::engraver::layout::tlayout::HarmonyStyle;
use crate::time::MusicalPositionExt as _;

impl ChartLayoutEngine {
    /// Group measures into systems based on maximum measures per system.
    ///
    /// Always uses count-based grouping to maintain consistent layout.
    /// Rhythm compression handles fitting content within the allocated width.
    /// Break the section's measures into systems.
    ///
    /// `max_measures_per_system` is a cap, not a target: a system also stops
    /// when the next bar's chord symbols would not fit beside the ones already
    /// on the line. Without that, raising the cap — which is exactly what the
    /// folded and compact modes do — packs bars in until the symbols run into
    /// each other, and a bar of six chords at eight bars a line prints as one
    /// smear.
    pub(super) fn group_measures_into_systems(
        &self,
        measures: &[crate::chart::types::Measure],
        content_width: f64,
        measured_min: &[f64],
    ) -> Vec<Vec<usize>> {
        let mut systems = Vec::new();
        let mut current_system: Vec<usize> = Vec::new();
        let mut current_width = 0.0f64;

        let harmony_style = self.measured_harmony_style();
        // A bar asks for the narrowest it can honestly be drawn: room for its
        // symbols on their beats (the measuring pass's minimum), its notes,
        // and at least its slashes. Not a comfortable width — the bars-per-line
        // cap is the phrase grouping a chart is read by, and a line keeps all
        // of it unless the symbols physically cannot fit. The spring
        // distribution then shares out whatever room is left.
        //
        // Symbols are measured at the smallest a line may draw them
        // (`width_dist::MIN_SYSTEM_SYMBOL_SCALE`): a full line of chord
        // clusters draws its symbols smaller rather than lose a bar.
        let symbol_scale = width_dist::MIN_SYSTEM_SYMBOL_SCALE;
        let widths: Vec<f64> = measures
            .iter()
            .enumerate()
            .map(|(idx, m)| {
                (self
                    .chord_symbol_width_floor(m, &harmony_style)
                    .max(measured_min.get(idx).copied().unwrap_or(0.0))
                    * symbol_scale)
                    .max(self.melody_width_needed(m))
            })
            .collect();
        // A system always holds at least one measure, however wide it is —
        // better one cramped bar than an empty line and the same cramped bar.
        let usable = content_width.max(1.0);

        // A section of written notes keeps one grid: the same number of bars
        // on every line, as many as every line can hold. Greedy packing would
        // set one bar here and two there, and a chart is read by counting
        // bars. The grid is checked line by line, not against the widest bar
        // alone: a dense hook bar and the short turnaround after it share a
        // line wherever the pair fits, even though two hook bars would not.
        if measures
            .iter()
            .any(|m| self.melody_width_needed(m) > 0.0 || is_dense_rhythm(m))
            && !measures.iter().any(|m| m.volta_start.is_some())
        {
            let needed: Vec<f64> = widths
                .iter()
                .zip(measured_min.iter().chain(std::iter::repeat(&0.0)))
                .map(|(w, m)| w.max(*m).max(1.0))
                .collect();
            let per_line =
                uniform_bars_per_line(&needed, usable, self.config.max_measures_per_system.max(1));
            return (0..measures.len())
                .collect::<Vec<_>>()
                .chunks(per_line)
                .map(<[usize]>::to_vec)
                .collect();
        }

        // A section one bar past the cap goes on one line, when asked. Two
        // lines for five bars costs a system of height to print a single bar,
        // and that bar is what a reader loses the chart's shape to.
        if self.config.fit_whole_section_on_one_system
            && measures.len() == self.config.max_measures_per_system + 1
            && !measures.iter().any(starts_long_volta)
        {
            let squeezed: f64 = measures
                .iter()
                .map(|measure| self.chord_symbol_width_floor(measure, &harmony_style))
                .sum();
            if squeezed <= usable {
                return vec![(0..measures.len()).collect()];
            }
        }

        for (idx, measure) in measures.iter().enumerate() {
            if starts_long_volta(measure) && !current_system.is_empty() {
                systems.push(std::mem::take(&mut current_system));
                current_width = 0.0;
            }

            // An ending stays with the phrase it ends. A four-bar line whose
            // next bar is a second ending runs to five rather than stranding
            // `[2]` on a line of its own — which is what a chart with a first
            // and second ending looks like everywhere. It overrides the width
            // check as well as the cap, since one more bar on an otherwise
            // full line is exactly the case, and it is bounded so a run of
            // endings cannot walk off the page.
            let keeps_an_ending_with_its_phrase = measure.volta_start.is_some()
                && !starts_long_volta(measure)
                && !current_system.is_empty()
                && current_system.len() < self.config.max_measures_per_system + VOLTA_SYSTEM_SLACK;

            let wanted = widths[idx];
            if !keeps_an_ending_with_its_phrase
                && !current_system.is_empty()
                && current_width + wanted > usable
            {
                systems.push(std::mem::take(&mut current_system));
                current_width = 0.0;
            }

            current_system.push(idx);
            current_width += wanted;

            let next_keeps_its_ending = measures
                .get(idx + 1)
                .is_some_and(|m| m.volta_start.is_some() && !starts_long_volta(m));
            let slack = if next_keeps_its_ending {
                VOLTA_SYSTEM_SLACK
            } else {
                0
            };
            if current_system.len() >= self.config.max_measures_per_system + slack {
                systems.push(std::mem::take(&mut current_system));
                current_width = 0.0;
            }
        }

        if !current_system.is_empty() {
            systems.push(current_system);
        }

        rescue_orphan_bar(&mut systems, &widths, usable, measures);
        systems
    }

    /// The width a bar of written notes needs at this staff size: each note
    /// a share by its value (a quarter under three spaces, an eighth a little
    /// over two), plus room for the barline padding. Zero for
    /// a bar with no notes, so slash bars still pack by their symbols.
    pub(super) fn melody_width_needed(&self, measure: &crate::chart::types::Measure) -> f64 {
        let sp = self.config.spatium;
        // A written chord rhythm (`G#m_8 G#m_8 r8 …`) needs room the same
        // way: each attack, rest or space a share by its value.
        if measure.melodies.iter().all(|m| m.notes.is_empty()) && is_dense_rhythm(measure) {
            let ts = crate::time::TimeSignature::new(
                measure.time_signature.0.into(),
                measure.time_signature.1.into(),
            );
            let shares: f64 = measure
                .chords
                .iter()
                .map(|c| match c.duration.to_beats(ts) {
                    b if b >= 4.0 => 4.0,
                    b if b >= 2.0 => 3.3,
                    b if b >= 1.0 => 2.8,
                    b if b >= 0.5 => 2.2,
                    b if b >= 0.25 => 1.8,
                    _ => 1.5,
                })
                .sum();
            return shares * sp + 2.0 * sp;
        }
        let notes: f64 = measure
            .melodies
            .iter()
            .flat_map(|m| &m.notes)
            .filter(|n| !n.is_space())
            .map(|n| {
                // Measured against engraved charts: Sibelius sets four bars
                // of eight eighths across a 540pt line at a 5pt space.
                let share = match n.duration {
                    1 => 4.0,
                    2 => 3.3,
                    4 => 2.8,
                    8 => 2.2,
                    16 => 1.8,
                    _ => 1.5,
                };
                if n.dotted { share + 0.5 } else { share }
            })
            .sum::<f64>()
            * sp;
        if notes > 0.0 { notes + 2.0 * sp } else { 0.0 }
    }

    /// The narrowest this measure can honestly be drawn.
    ///
    /// Room for the symbols laid end to end, and a bar with none still gets
    /// its slashes — never the comfortable `min_measure_width`, which at the
    /// master-rhythm preset is a quarter of the page and would decide line
    /// breaks by looks rather than by what fits.
    pub(super) fn chord_symbol_width_floor(
        &self,
        measure: &crate::chart::types::Measure,
        harmony_style: &HarmonyStyle,
    ) -> f64 {
        // Four slashes and the space they need to read as four.
        self.printed_symbols_width(measure, harmony_style)
            .max(self.config.spatium * 8.0)
    }

    /// The bar's printed symbols laid end to end at their rendered widths,
    /// each with the clearance the renderer keeps between two symbols.
    pub(super) fn printed_symbols_width(
        &self,
        measure: &crate::chart::types::Measure,
        harmony_style: &HarmonyStyle,
    ) -> f64 {
        let spacing = symbol_spacing::SymbolSpacing::new(
            self.config.spatium,
            self.config.min_chord_symbol_gap,
        );
        symbol_spacing::printed_symbols(measure, harmony_style, measure.time_signature, true)
            .iter()
            .map(|s| s.extent.left + s.extent.right + spacing.clearance)
            .sum()
    }

    /// The configured harmony style with the engine's font metrics, so a
    /// symbol measures as wide as it will print.
    pub(super) fn measured_harmony_style(&self) -> HarmonyStyle {
        self.measured_fonts().1
    }

    /// The engine's text font metrics, and the configured harmony style
    /// measuring with them.
    pub(super) fn measured_fonts(&self) -> (TextFontMetrics, HarmonyStyle) {
        let text_metrics = TextFontMetrics::new(self.text_font_data.clone());
        // Use text font for symbols when symbol_font_family is None (e.g., MuseJazz uses same font)
        let symbol_metrics = if self.config.harmony_style.symbol_font_family.is_none() {
            text_metrics.clone()
        } else {
            TextFontMetrics::new(self.symbol_font_data.clone())
        };
        let harmony_style = self
            .config
            .harmony_style
            .clone()
            .with_text_font_metrics(text_metrics.clone())
            .with_symbol_font_metrics(symbol_metrics);
        (text_metrics, harmony_style)
    }
}

/// The most bars a line can hold when every line holds the same number.
///
/// `needed` is each bar's width; bars are taken in order, `n` to a line, and
/// `n` is the largest (up to `cap`) for which no line overflows `usable`.
/// A bar wider than the line still gets a line to itself.
pub(super) fn uniform_bars_per_line(needed: &[f64], usable: f64, cap: usize) -> usize {
    (1..=cap.max(1))
        .rev()
        .find(|&n| {
            needed
                .chunks(n)
                .all(|line| line.iter().sum::<f64>() <= usable)
        })
        .unwrap_or(1)
}

/// How far past the bars-per-line cap a system may run to keep an ending with
/// its phrase. Two, so a first and second ending both fit.
pub(super) const VOLTA_SYSTEM_SLACK: usize = 2;

/// Pull a stranded last bar back onto a line with company.
///
/// The grouper takes four bars and breaks, so a five-bar section comes out
/// four-and-one and the fifth bar gets a line of the page to itself. A phrase
/// that runs one past the line reads as three-and-two everywhere else — the
/// bar belongs with the music, not alone under it.
///
/// Only the true orphan is rescued. A two- or three-bar tail is an ordinary
/// short line and the four-bar grid is worth more than evening it out.
pub(super) fn rescue_orphan_bar(
    systems: &mut [Vec<usize>],
    widths: &[f64],
    usable: f64,
    measures: &[crate::chart::types::Measure],
) {
    // Only a section of one line and a bar is rebalanced. In a longer one the
    // full lines above set the four-bar grid the reader counts by, and a
    // short line just before the end would break it: seventeen bars read as
    // four, four, four, four and one, not four, four, four, three and two.
    if systems.len() != 2 {
        return;
    }
    let Some((last, rest)) = systems.split_last_mut() else {
        return;
    };
    let Some(previous) = rest.last_mut() else {
        return;
    };
    if last.len() != 1 || previous.len() < 3 {
        return;
    }

    let Some(moved) = previous.last().copied() else {
        return;
    };

    // The bar that moves must not be in the middle of an ending: a volta drawn
    // across a line break is worse than a bar on its own. A bar that *starts*
    // one is free to move — a second ending opening the next line is how every
    // chart with two endings is set.
    let splits_an_ending = previous.iter().any(|idx| {
        measures.get(*idx).is_some_and(|measure| {
            measure.volta_start.as_ref().is_some_and(|volta| {
                *idx < moved && *idx + usize::from(volta.length_measures) > moved
            })
        })
    });
    if splits_an_ending {
        return;
    }
    let width = widths.get(moved).copied().unwrap_or(0.0)
        + last
            .first()
            .and_then(|idx| widths.get(*idx))
            .copied()
            .unwrap_or(0.0);
    if width > usable {
        return;
    }

    previous.pop();
    last.insert(0, moved);
}

/// A bar whose written chord rhythm has attacks shorter than a beat — a
/// comp figure, not a chord held under slashes. It needs room like notes
/// do, and its section keeps one grid.
pub(crate) fn is_dense_rhythm(measure: &crate::chart::types::Measure) -> bool {
    let ts = crate::time::TimeSignature::new(
        measure.time_signature.0.into(),
        measure.time_signature.1.into(),
    );
    // Rests and spaces are chord entries too (`r8`, `s4`), each with its
    // length; a bar written as one chord has one entry of a whole bar.
    measure.chords.len() > 1 && measure.chords.iter().any(|c| c.duration.to_beats(ts) < 1.0)
}

pub(super) fn starts_long_volta(measure: &crate::chart::types::Measure) -> bool {
    measure
        .volta_start
        .as_ref()
        .is_some_and(|volta| volta.length_measures > 1)
}
