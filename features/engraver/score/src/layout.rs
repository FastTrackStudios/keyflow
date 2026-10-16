//! Single-part page layout (P2 of the score-engraving effort).
//!
//! Turns one [`crate::model::Part`] into engraved pages: systems of measures
//! with clef/key/time prefixes, beamed rhythms via the engraver's
//! `MeasureBuilder`, key-aware accidentals with measure-local state, and
//! duration-based multi-measure rests (Finale writes plain `<rest/>` bars,
//! never `measure="yes"`).
//!
//! Coordinate model matches the chart pipeline: one scene, pages stacked
//! vertically, each page cropped at export time by its `PageRect`.

use engraver_proto::engraver::layout::chart::prefix_renderer::{
    PrefixRenderContext, calculate_prefix_width, render_system_prefix,
};
use engraver_proto::engraver::layout::chart::{
    constants, measure_layout, spacing as chart_spacing,
};
use engraver_proto::engraver::layout::context::LayoutContextOwned;
use engraver_proto::engraver::layout::tlayout::{Accidental, ClefType, TupletRatio};
use engraver_proto::engraver::model::{Octave, Pitch, PitchClass};
use engraver_proto::engraver::notation::{
    Duration, DurationKind, MeasureBuilder, RhythmEntry, TimeSignature,
};
use engraver_proto::engraver::scene::id::{ElementType, SemanticId};
use engraver_proto::engraver::scene::node::SceneNode;
use engraver_proto::engraver::scene::paint::{FontStyle, FontWeight, PaintCommand, TextAnchor};
use kurbo::{Affine, Point, Rect};
use peniko::Color;

mod vertical;

use crate::model::{self, BarStyle, ClefSign, Event, NoteTypeValue, Score, Step, WrittenPitch};

/// Page + engraving options. Defaults follow the Columbus part books:
/// US Letter portrait, generous margins, 6.5pt spatium.
///
/// Everything here is engraver-native (points, measure indices). A source
/// that knows its own layout — Finale's linked parts — fills
/// [`LayoutOptions::explicit`] and [`LayoutOptions::page_texts`] so the part
/// comes out with the arranger's system breaks, page turns and headers;
/// without them the engine packs systems itself.
#[derive(Debug, Clone)]
pub struct LayoutOptions {
    pub page_width: f64,
    pub page_height: f64,
    pub margin: f64,
    /// Staff space in points.
    pub spatium: f64,
    /// Vertical distance between system tops.
    pub system_gap: f64,
    /// Vertical space reserved for the page-1 title block.
    pub title_block_height: f64,
    /// Minimum measures a rest run needs to collapse into a multirest.
    pub multirest_min: usize,
    /// Family for all page and staff text (title, cues, measure numbers).
    pub text_font: String,
    pub measure_numbers: MeasureNumbers,
    /// Page headers/footers. `None` draws the default title block (part
    /// name, title, composer on page 1).
    pub page_texts: Option<Vec<PageText>>,
    /// Source-supplied system/page/multirest layout.
    pub explicit: Option<ExplicitLayout>,
    /// Per-side margins; `None` = `margin` on every side.
    pub margins: Option<Margins>,
}

/// Page margins in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl LayoutOptions {
    #[must_use]
    pub fn margins(&self) -> Margins {
        self.margins.unwrap_or(Margins {
            top: self.margin,
            right: self.margin,
            bottom: self.margin,
            left: self.margin,
        })
    }
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            page_width: 612.0,
            page_height: 792.0,
            margin: 44.0,
            spatium: 6.5,
            system_gap: 74.0,
            title_block_height: 84.0,
            multirest_min: 2,
            text_font: crate::fonts::SERIF_FAMILY.to_string(),
            measure_numbers: MeasureNumbers::default(),
            page_texts: None,
            explicit: None,
            margins: None,
        }
    }
}

/// A text face: family, point size, weight and slant.
#[derive(Debug, Clone, PartialEq)]
pub struct TextFont {
    pub family: String,
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
}

impl TextFont {
    #[must_use]
    pub fn serif(size: f64) -> Self {
        Self {
            family: crate::fonts::SERIF_FAMILY.to_string(),
            size,
            bold: false,
            italic: false,
        }
    }

    #[must_use]
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    #[must_use]
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VAlign {
    #[default]
    Top,
    Center,
    Bottom,
}

/// Text placed on a range of pages (title, running header, footer).
///
/// `text` may contain `{page}` and `{pages}`, filled per page. The anchor
/// is the margin box edge chosen by `h_align`/`v_align`; `x` moves right
/// and `y` moves DOWN from it (to the text's top edge).
#[derive(Debug, Clone, PartialEq)]
pub struct PageText {
    pub text: String,
    pub font: TextFont,
    /// 1-based.
    pub first_page: usize,
    /// 1-based, inclusive; `None` = through the last page.
    pub last_page: Option<usize>,
    pub h_align: HAlign,
    pub v_align: VAlign,
    pub x: f64,
    pub y: f64,
}

/// Where measure numbers print.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasureNumberMode {
    None,
    /// Above the first measure of each system.
    SystemStart,
    /// Every `n` measures (and at system starts).
    Every(u32),
}

/// Measure-number style for a part.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasureNumbers {
    pub mode: MeasureNumberMode,
    /// Skip the piece's first measure.
    pub exclude_first: bool,
    /// Print "11-14" under a multimeasure rest.
    pub multirest_ranges: bool,
    pub font: TextFont,
    pub multirest_font: TextFont,
    /// Offset from a measure's left barline at the top staff line
    /// (x right, y down), points.
    pub offset: (f64, f64),
    /// Offset from a multirest's center at the top staff line.
    pub multirest_offset: (f64, f64),
}

impl Default for MeasureNumbers {
    fn default() -> Self {
        Self {
            mode: MeasureNumberMode::SystemStart,
            exclude_first: false,
            multirest_ranges: false,
            font: TextFont::serif(9.0).italic(),
            multirest_font: TextFont::serif(9.0).italic(),
            offset: (0.0, -8.0),
            multirest_offset: (0.0, 38.0),
        }
    }
}

/// System, page and multirest layout dictated by the source.
///
/// Measure indices are positions in [`crate::model::Part::measures`].
#[derive(Debug, Clone, Default)]
pub struct ExplicitLayout {
    pub systems: Vec<ExplicitSystem>,
    pub multirests: Vec<ExplicitMultiRest>,
    /// Relative width (spring) per measure index; `None` = engine weight.
    pub measure_widths: Vec<Option<f64>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExplicitSystem {
    /// Measure indices, end exclusive.
    pub measures: std::ops::Range<usize>,
    /// 0-based page.
    pub page: usize,
    /// Top staff line, points from the page top.
    pub staff_top: f64,
    /// Staff start and end, points from the page's left edge.
    pub left: f64,
    pub right: f64,
    pub spatium: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExplicitMultiRest {
    /// Measure indices, end exclusive.
    pub measures: std::ops::Range<usize>,
    /// Relative width (same scale as `measure_widths`).
    pub width: f64,
}

/// One exported page's crop box within the shared scene.
#[derive(Debug, Clone, Copy)]
pub struct PageRect {
    pub x_offset: f64,
    pub y_offset: f64,
    pub width: f64,
    pub height: f64,
}

/// Result of laying out one part.
pub struct PartLayout {
    pub scene: SceneNode,
    pub pages: Vec<PageRect>,
    /// Diagnostics: events dropped because they sat in a non-primary voice.
    pub dropped_voice_events: usize,
}

// ───────────────────────── measure preparation ─────────────────────────

/// A prepared measure ready for `MeasureBuilder`, or a multirest run.
enum Prepared {
    Notated(Box<NotatedMeasure>),
    /// Consecutive resting measures collapsed into one H-bar block.
    MultiRest(MultiRestBlock),
}

struct MultiRestBlock {
    /// Index of the first measure in [`Part::measures`].
    first: usize,
    count: usize,
    number: String,
    last_number: String,
    clef: ClefType,
    key_fifths: i8,
    time: (u32, u32),
    /// Cues and texts on the first measure (rehearsal marks, tempo).
    directions: Vec<model::Direction>,
    barline: BarStyle,
}

impl Prepared {
    fn first_index(&self) -> usize {
        match self {
            Prepared::Notated(m) => m.index,
            Prepared::MultiRest(r) => r.first,
        }
    }

    fn barline(&self) -> BarStyle {
        match self {
            Prepared::Notated(m) => m.barline,
            Prepared::MultiRest(r) => r.barline,
        }
    }

    fn key_fifths(&self) -> i8 {
        match self {
            Prepared::Notated(m) => m.key_fifths,
            Prepared::MultiRest(r) => r.key_fifths,
        }
    }

    fn directions(&self) -> &[model::Direction] {
        match self {
            Prepared::Notated(m) => &m.directions,
            Prepared::MultiRest(r) => &r.directions,
        }
    }
}

struct NotatedMeasure {
    /// Index in [`Part::measures`].
    index: usize,
    number: String,
    barline: BarStyle,
    entries: Vec<RhythmEntry>,
    pitches: Vec<Option<(i32, Accidental)>>,
    stacks: Vec<Vec<(i32, Accidental)>>,
    tuplets: Vec<(usize, usize, TupletRatio)>,
    /// Per entry: staff lines of noteheads that START a tie.
    tie_start_lines: Vec<Vec<i32>>,
    /// Per entry: staff lines of noteheads that END a tie.
    tie_stop_lines: Vec<Vec<i32>>,
    /// Per entry: slur start/stop flags.
    slur_starts: Vec<bool>,
    slur_stops: Vec<bool>,
    /// Per entry: tick onset (for direction anchoring).
    entry_ticks: Vec<u32>,
    /// Measure length in the same ticks (0 when unknown).
    len_ticks: u32,
    /// Per entry: articulation tags (staccato, accent, …) and the side the
    /// source pins each to (`Some(true)` = above), if any.
    arts: Vec<Vec<(String, Option<bool>)>>,
    /// Directions in this measure (dynamics, hairpins, texts).
    directions: Vec<model::Direction>,
    /// Clef change taking effect at this measure (drawn in-measure when
    /// mid-system, absorbed by the prefix at system start).
    clef: ClefType,
    clef_changed: bool,
    key_fifths: i8,
    key_changed: bool,
    time: (u32, u32),
    time_changed: bool,
}

impl NotatedMeasure {
    /// Nothing but one whole-bar rest (and nothing drawn in the bar that
    /// would need the measure builder: no clef/key/time change).
    fn is_bar_rest(&self) -> bool {
        matches!(
            self.entries.as_slice(),
            [RhythmEntry::Rest(d)] if d.kind == DurationKind::Whole && d.dots == 0
        ) && !self.clef_changed
            && !self.key_changed
            && !self.time_changed
    }

    /// A whole-bar-rest measure with the given attribute state.
    fn rest_bar(
        index: usize,
        number: String,
        clef: ClefType,
        key_fifths: i8,
        time: (u32, u32),
    ) -> Self {
        Self {
            index,
            number,
            barline: BarStyle::Regular,
            entries: vec![RhythmEntry::Rest(Duration::Whole)],
            pitches: vec![None],
            stacks: vec![Vec::new()],
            tuplets: Vec::new(),
            tie_start_lines: vec![Vec::new()],
            tie_stop_lines: vec![Vec::new()],
            slur_starts: vec![false],
            slur_stops: vec![false],
            entry_ticks: vec![0],
            len_ticks: 0,
            arts: vec![Vec::new()],
            directions: Vec::new(),
            clef,
            clef_changed: false,
            key_fifths,
            key_changed: false,
            time,
            time_changed: false,
        }
    }
}

/// Walk state: clef/key/time inherited across measures.
struct AttrState {
    clef: ClefType,
    key_fifths: i8,
    time: (u32, u32),
}

fn clef_type(clef: &model::Clef) -> ClefType {
    match (clef.sign, clef.line) {
        (ClefSign::G, _) => ClefType::Treble,
        (ClefSign::F, _) => ClefType::Bass,
        (ClefSign::C, Some(4)) => ClefType::Tenor,
        (ClefSign::C, _) => ClefType::Alto,
        (ClefSign::Percussion, _) => ClefType::Percussion,
        (ClefSign::Tab, _) => ClefType::Tab,
        (ClefSign::None, _) => ClefType::Treble,
    }
}

/// Staff position (diatonic steps) of the pitch sitting ON the middle line.
fn middle_line_ref(clef: ClefType, octave_change: i8) -> i32 {
    let base = match clef {
        ClefType::Treble => Pitch::new(PitchClass::B, Octave::new(4)),
        ClefType::Bass => Pitch::new(PitchClass::D, Octave::new(3)),
        ClefType::Alto => Pitch::new(PitchClass::C, Octave::new(4)),
        ClefType::Tenor => Pitch::new(PitchClass::A, Octave::new(3)),
        // Percussion/tab: treat like treble for display positions.
        ClefType::Percussion | ClefType::Tab => Pitch::new(PitchClass::B, Octave::new(4)),
    };
    base.staff_position() + i32::from(octave_change) * 7
}

fn pitch_class(step: Step) -> PitchClass {
    match step {
        Step::A => PitchClass::A,
        Step::B => PitchClass::B,
        Step::C => PitchClass::C,
        Step::D => PitchClass::D,
        Step::E => PitchClass::E,
        Step::F => PitchClass::F,
        Step::G => PitchClass::G,
    }
}

/// Alteration the key signature implies for a step (sharps F C G D A E B,
/// flats B E A D G C F).
fn key_alteration(step: Step, fifths: i8) -> i8 {
    const SHARPS: [Step; 7] = [
        Step::F,
        Step::C,
        Step::G,
        Step::D,
        Step::A,
        Step::E,
        Step::B,
    ];
    if fifths > 0 {
        let n = fifths.min(7) as usize;
        i8::from(SHARPS[..n].contains(&step))
    } else if fifths < 0 {
        let n = (-fifths).min(7) as usize;
        -i8::from(SHARPS[7 - n..].contains(&step))
    } else {
        0
    }
}

fn accidental_from_alter(alter: i8) -> Accidental {
    match alter {
        1 => Accidental::Sharp,
        -1 => Accidental::Flat,
        2 => Accidental::DoubleSharp,
        -2 => Accidental::DoubleFlat,
        _ => Accidental::Natural,
    }
}

/// Measure-local accidental memory: (step, octave) → alteration in effect.
#[derive(Default)]
struct AccidentalState(std::collections::HashMap<(Step, i8), i8>);

impl AccidentalState {
    /// Decide the printed accidental for a note and update the memory.
    fn resolve(&mut self, pitch: &WrittenPitch, key_fifths: i8) -> Accidental {
        let key = (pitch.step, pitch.octave);
        let in_effect = self
            .0
            .get(&key)
            .copied()
            .unwrap_or_else(|| key_alteration(pitch.step, key_fifths));
        if pitch.alter == in_effect {
            return Accidental::None;
        }
        self.0.insert(key, pitch.alter);
        accidental_from_alter(pitch.alter)
    }
}

fn duration_from_type(t: NoteTypeValue, dots: u8, tuplet: Option<(u8, u8)>) -> Duration {
    let kind = match t {
        NoteTypeValue::Maxima
        | NoteTypeValue::Long
        | NoteTypeValue::Breve
        | NoteTypeValue::Whole => DurationKind::Whole,
        NoteTypeValue::Half => DurationKind::Half,
        NoteTypeValue::Quarter => DurationKind::Quarter,
        NoteTypeValue::Eighth => DurationKind::Eighth,
        NoteTypeValue::Sixteenth => DurationKind::Sixteenth,
        NoteTypeValue::ThirtySecond => DurationKind::ThirtySecond,
        _ => DurationKind::SixtyFourth,
    };
    Duration {
        kind,
        dots: dots.min(2),
        tuplet: tuplet.map(|(a, n)| TupletRatio {
            numerator: a,
            denominator: n,
        }),
    }
}

/// Derive a written duration from raw ticks when `<type>` is absent.
fn duration_from_ticks(ticks: u32, divisions: u32) -> Duration {
    let quarters = f64::from(ticks) / f64::from(divisions.max(1));
    const KINDS: [DurationKind; 7] = [
        DurationKind::Whole,
        DurationKind::Half,
        DurationKind::Quarter,
        DurationKind::Eighth,
        DurationKind::Sixteenth,
        DurationKind::ThirtySecond,
        DurationKind::SixtyFourth,
    ];
    let mut best = (f64::MAX, Duration::Quarter);
    for kind in KINDS {
        for dots in 0..=2u8 {
            let mut value = kind.quarters();
            if dots >= 1 {
                value *= 1.5;
            }
            if dots == 2 {
                value = kind.quarters() * 1.75;
            }
            let err = (value - quarters).abs();
            if err < best.0 {
                best = (
                    err,
                    Duration {
                        kind,
                        dots,
                        tuplet: None,
                    },
                );
            }
        }
    }
    best.1
}

/// Is this measure "empty" — nothing but rests (or nothing at all)?
fn is_resting(measure: &model::Measure) -> bool {
    measure.events.iter().all(|e| matches!(e, Event::Rest(_)))
}

/// Choose the primary voice for a staff-1 measure: the voice with the most
/// chord events (ties broken toward the lowest voice number).
fn primary_voice(measure: &model::Measure) -> Option<u32> {
    let mut counts: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
    for event in &measure.events {
        if let Event::Chord(c) = event
            && c.staff == 1
        {
            *counts.entry(c.voice).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(v, _)| v)
}

/// The directions a part shows at measure `index`: its own, with rehearsal
/// marks taken from the score (top part) so every part gets the cues.
fn part_directions(score: &Score, part_index: usize, index: usize) -> Vec<model::Direction> {
    let mut out: Vec<model::Direction> = score.parts[part_index]
        .measures
        .get(index)
        .map(|m| {
            m.directions
                .iter()
                .filter(|d| !matches!(d.kind, model::DirectionKind::Rehearsal(_)))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    for (tick, mark) in score.rehearsals_for_part(part_index, index) {
        out.push(model::Direction {
            tick,
            kind: model::DirectionKind::Rehearsal(mark),
        });
    }
    out
}

/// A cue a multirest must not swallow: a rehearsal mark or staff text.
fn breaks_multirest(directions: &[model::Direction]) -> bool {
    directions.iter().any(|d| {
        matches!(
            d.kind,
            model::DirectionKind::Rehearsal(_) | model::DirectionKind::Words(_)
        )
    })
}

fn prepare_measures(
    score: &Score,
    part_index: usize,
    opts: &LayoutOptions,
    dropped: &mut usize,
) -> Vec<Prepared> {
    let part = &score.parts[part_index];
    let mut state = AttrState {
        clef: ClefType::Treble,
        key_fifths: 0,
        time: (4, 4),
    };
    let mut prepared = Vec::new();

    /// A resting measure waiting to be grouped.
    struct RestInfo {
        index: usize,
        number: String,
        clef: ClefType,
        key_fifths: i8,
        time: (u32, u32),
        directions: Vec<model::Direction>,
        barline: BarStyle,
    }
    let mut rest_run: Vec<RestInfo> = Vec::new();

    let rest_bar = |r: RestInfo| -> Prepared {
        let mut bar = NotatedMeasure::rest_bar(r.index, r.number, r.clef, r.key_fifths, r.time);
        bar.directions = r.directions;
        bar.barline = r.barline;
        Prepared::Notated(Box::new(bar))
    };
    let flush_rests = |run: &mut Vec<RestInfo>, out: &mut Vec<Prepared>, min: usize| {
        if run.is_empty() {
            return;
        }
        if run.len() >= min.max(2) {
            let count = run.len();
            let mut drained = run.drain(..);
            let first = drained.next().expect("non-empty run");
            let last = drained.next_back();
            let (last_number, barline) = match &last {
                Some(l) => (l.number.clone(), l.barline),
                None => (first.number.clone(), first.barline),
            };
            out.push(Prepared::MultiRest(MultiRestBlock {
                first: first.index,
                count,
                number: first.number,
                last_number,
                clef: first.clef,
                key_fifths: first.key_fifths,
                time: first.time,
                directions: first.directions,
                barline,
            }));
        } else {
            for r in run.drain(..) {
                out.push(rest_bar(r));
            }
        }
    };

    // Source-dictated multirests: start index → end index (exclusive).
    let explicit_rests: Option<std::collections::HashMap<usize, usize>> =
        opts.explicit.as_ref().map(|e| {
            e.multirests
                .iter()
                .filter(|r| r.measures.len() >= 2)
                .map(|r| (r.measures.start, r.measures.end.min(part.measures.len())))
                .collect()
        });
    // Explicit multirest still being consumed: its end index.
    let mut in_explicit_rest: Option<usize> = None;

    for (index, measure) in part.measures.iter().enumerate() {
        let mut clef_changed = false;
        let mut key_changed = false;
        let mut time_changed = false;
        for change in &measure.attributes {
            if let Some((_, clef)) = change.clefs.iter().find(|(s, _)| *s == 1) {
                let ct = clef_type(clef);
                if ct != state.clef {
                    state.clef = ct;
                    clef_changed = true;
                }
            }
            if let Some(k) = change.key_fifths
                && k != state.key_fifths
            {
                state.key_fifths = k;
                key_changed = true;
            }
            if let Some(t) = change.time
                && t != state.time
            {
                state.time = t;
                time_changed = true;
            }
        }
        let directions = part_directions(score, part_index, index);

        if let Some(end) = in_explicit_rest {
            if index < end {
                continue;
            }
            in_explicit_rest = None;
        }
        if let Some(rests) = &explicit_rests {
            if let Some(&end) = rests.get(&index)
                && end > index + 1
            {
                let last = &part.measures[end - 1];
                prepared.push(Prepared::MultiRest(MultiRestBlock {
                    first: index,
                    count: end - index,
                    number: measure.number.clone(),
                    last_number: last.number.clone(),
                    clef: state.clef,
                    key_fifths: state.key_fifths,
                    time: state.time,
                    directions,
                    barline: last.barline,
                }));
                in_explicit_rest = Some(end);
                continue;
            }
        }

        let attrs_changed = clef_changed || key_changed || time_changed;
        if is_resting(measure) && !attrs_changed {
            let info = RestInfo {
                index,
                number: measure.number.clone(),
                clef: state.clef,
                key_fifths: state.key_fifths,
                time: state.time,
                directions,
                barline: measure.barline,
            };
            if explicit_rests.is_some() {
                // The source decided which rests group; a lone one is a bar.
                prepared.push(rest_bar(info));
                continue;
            }
            if breaks_multirest(&info.directions) {
                flush_rests(&mut rest_run, &mut prepared, opts.multirest_min);
            }
            let section_end = info.barline.is_section_end();
            rest_run.push(info);
            if section_end {
                flush_rests(&mut rest_run, &mut prepared, opts.multirest_min);
            }
            continue;
        }
        flush_rests(&mut rest_run, &mut prepared, opts.multirest_min);

        let Some(voice) = primary_voice(measure) else {
            // Rests only but with attribute changes: render a whole-rest bar.
            let mut bar = NotatedMeasure::rest_bar(
                index,
                measure.number.clone(),
                state.clef,
                state.key_fifths,
                state.time,
            );
            bar.directions = directions;
            bar.barline = measure.barline;
            bar.clef_changed = clef_changed;
            bar.key_changed = key_changed;
            bar.time_changed = time_changed;
            prepared.push(Prepared::Notated(Box::new(bar)));
            continue;
        };

        // Homophonic second voice: when another voice's chords share the
        // primary voice's exact rhythm signature (tick + duration), merge its
        // noteheads into the primary chords instead of dropping them
        // (violin div., horns a2…). True polyphony is P3.
        let merged_voice = homophonic_partner(measure, voice);

        let octave_change = 0i8; // per-measure clef octave marks: P3
        let middle = middle_line_ref(state.clef, octave_change);
        let mut acc_state = AccidentalState::default();
        let mut entries = Vec::new();
        let mut pitches = Vec::new();
        let mut stacks = Vec::new();
        let mut tuplet_marks: Vec<Option<TupletRatio>> = Vec::new();

        let mut tie_start_lines: Vec<Vec<i32>> = Vec::new();
        let mut tie_stop_lines: Vec<Vec<i32>> = Vec::new();
        let mut slur_starts: Vec<bool> = Vec::new();
        let mut slur_stops: Vec<bool> = Vec::new();
        let mut entry_ticks: Vec<u32> = Vec::new();
        let mut arts: Vec<Vec<(String, Option<bool>)>> = Vec::new();

        // Chords of the merged voice, keyed by (tick, duration).
        let partner_chords: std::collections::HashMap<(u32, u32), &model::Chord> = merged_voice
            .map(|mv| {
                measure
                    .events
                    .iter()
                    .filter_map(|e| match e {
                        Event::Chord(c) if c.voice == mv && c.staff == 1 && !c.grace => {
                            Some(((c.tick, c.duration), c))
                        }
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut events: Vec<&Event> = measure
            .events
            .iter()
            .filter(|e| {
                let is_merged_chord = matches!(
                    (e, merged_voice),
                    (Event::Chord(c), Some(mv)) if c.voice == mv && c.staff == 1 && !c.grace
                );
                let keep = e.voice() == voice
                    && match e {
                        Event::Chord(c) => c.staff == 1 && !c.grace,
                        Event::Rest(r) => r.staff == 1,
                    };
                if !keep
                    && !is_merged_chord
                    && !matches!((e, merged_voice), (Event::Rest(r), Some(mv)) if r.voice == mv)
                {
                    *dropped += 1;
                }
                keep
            })
            .collect();
        events.sort_by_key(|e| e.tick());

        for event in events {
            match event {
                Event::Chord(chord) => {
                    let duration = chord
                        .written
                        .note_type
                        .map(|t| duration_from_type(t, chord.written.dots, chord.written.tuplet))
                        .unwrap_or_else(|| duration_from_ticks(chord.duration, measure.divisions));
                    entries.push(RhythmEntry::Note(duration));
                    tuplet_marks.push(duration.tuplet);

                    let partner = partner_chords.get(&(chord.tick, chord.duration));
                    let all_notes = chord
                        .notes
                        .iter()
                        .chain(partner.into_iter().flat_map(|p| p.notes.iter()));

                    let mut lines: Vec<(i32, Accidental)> = Vec::new();
                    let mut starts: Vec<i32> = Vec::new();
                    let mut stops: Vec<i32> = Vec::new();
                    for n in all_notes {
                        let p = Pitch::with_alteration(
                            pitch_class(n.pitch.step),
                            Octave::new(n.pitch.octave),
                            n.pitch.alter,
                        );
                        let line = p.staff_position() - middle;
                        // A tied-over note never repeats its accidental,
                        // and does not establish one for the bar either: a
                        // later untied note of that pitch still shows it.
                        let acc = if n.tie_stop {
                            Accidental::None
                        } else {
                            acc_state.resolve(&n.pitch, state.key_fifths)
                        };
                        if !lines.iter().any(|(l, _)| *l == line) {
                            lines.push((line, acc));
                        }
                        if n.tie_start {
                            starts.push(line);
                        }
                        if n.tie_stop {
                            stops.push(line);
                        }
                    }
                    // Primary head = highest note; extras stack below it.
                    lines.sort_by_key(|(line, _)| -line);
                    let primary = lines.remove(0);
                    pitches.push(Some(primary));
                    stacks.push(lines);
                    tie_start_lines.push(starts);
                    tie_stop_lines.push(stops);
                    slur_starts.push(chord.slur_start);
                    slur_stops.push(chord.slur_stop);
                    entry_ticks.push(chord.tick);
                    arts.push(
                        chord
                            .articulations
                            .iter()
                            .map(|t| (t.clone(), chord.articulation_above.get(t).copied()))
                            .collect(),
                    );
                }
                Event::Rest(rest) => {
                    let duration = if rest.measure_rest || rest.duration >= measure.len_ticks {
                        Duration::Whole
                    } else {
                        rest.written
                            .note_type
                            .map(|t| duration_from_type(t, rest.written.dots, rest.written.tuplet))
                            .unwrap_or_else(|| {
                                duration_from_ticks(rest.duration, measure.divisions)
                            })
                    };
                    entries.push(RhythmEntry::Rest(duration));
                    tuplet_marks.push(duration.tuplet);
                    pitches.push(None);
                    stacks.push(Vec::new());
                    tie_start_lines.push(Vec::new());
                    tie_stop_lines.push(Vec::new());
                    slur_starts.push(false);
                    slur_stops.push(false);
                    entry_ticks.push(rest.tick);
                    arts.push(Vec::new());
                }
            }
        }

        if entries.is_empty() {
            entries.push(RhythmEntry::Rest(Duration::Whole));
            pitches.push(None);
            stacks.push(Vec::new());
            tuplet_marks.push(None);
            tie_start_lines.push(Vec::new());
            tie_stop_lines.push(Vec::new());
            slur_starts.push(false);
            slur_stops.push(false);
            entry_ticks.push(0);
            arts.push(Vec::new());
        }

        // Contiguous runs of same-ratio tuplet entries become bracket groups.
        let mut tuplets = Vec::new();
        let mut run_start: Option<(usize, TupletRatio)> = None;
        for (idx, mark) in tuplet_marks.iter().enumerate() {
            match (run_start, mark) {
                (None, Some(r)) => run_start = Some((idx, *r)),
                (Some((start, r)), Some(m)) if *m == r => {
                    let _ = start; // run continues
                }
                (Some((start, r)), _) => {
                    if idx - start >= 2 {
                        tuplets.push((start, idx - 1, r));
                    }
                    run_start = mark.as_ref().map(|m| (idx, *m));
                }
                (None, None) => {}
            }
        }
        if let Some((start, r)) = run_start
            && tuplet_marks.len() - start >= 2
        {
            tuplets.push((start, tuplet_marks.len() - 1, r));
        }

        prepared.push(Prepared::Notated(Box::new(NotatedMeasure {
            index,
            number: measure.number.clone(),
            barline: measure.barline,
            entries,
            pitches,
            stacks,
            tuplets,
            tie_start_lines,
            tie_stop_lines,
            slur_starts,
            slur_stops,
            entry_ticks,
            len_ticks: measure.len_ticks,
            arts,
            directions,
            clef: state.clef,
            clef_changed,
            key_fifths: state.key_fifths,
            key_changed,
            time: state.time,
            time_changed,
        })));
    }
    flush_rests(&mut rest_run, &mut prepared, opts.multirest_min);
    prepared
}

/// Find a second voice whose chords share the primary voice's exact rhythm
/// signature (same (tick, duration) multiset) — safe to merge as stacked
/// noteheads.
fn homophonic_partner(measure: &model::Measure, primary: u32) -> Option<u32> {
    let signature = |v: u32| -> Vec<(u32, u32)> {
        let mut sig: Vec<(u32, u32)> = measure
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Chord(c) if c.voice == v && c.staff == 1 && !c.grace => {
                    Some((c.tick, c.duration))
                }
                _ => None,
            })
            .collect();
        sig.sort_unstable();
        sig
    };
    let primary_sig = signature(primary);
    if primary_sig.is_empty() {
        return None;
    }
    let voices: std::collections::BTreeSet<u32> = measure
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Chord(c) if c.staff == 1 && !c.grace => Some(c.voice),
            _ => None,
        })
        .collect();
    voices
        .into_iter()
        .find(|v| *v != primary && signature(*v) == primary_sig)
}

// ───────────────────────── system + page assembly ─────────────────────────

const MULTIREST_WIDTH_SP: f64 = 16.0;

fn build_measure(
    m: &NotatedMeasure,
    ctx: &LayoutContextOwned,
    width_sp: Option<f64>,
    draw_clef: bool,
    draw_time: bool,
    id_base: u64,
) -> engraver_proto::engraver::notation::MeasureScene {
    let mut builder = MeasureBuilder::new()
        // Parts follow engraving convention: a middle-line note stems down.
        .middle_line_stem_down(true)
        .id_base(id_base)
        .entries(m.entries.clone())
        .note_pitches(m.pitches.clone())
        .note_pitch_stacks(m.stacks.clone())
        .time_signature_meta(TimeSignature::new(m.time.0 as u8, m.time.1 as u8))
        .clef_meta(m.clef);
    if draw_clef {
        builder = builder.clef(m.clef);
    }
    if draw_time {
        builder = builder.time_signature(m.time.0 as u8, m.time.1 as u8);
    }
    for (start, end, ratio) in &m.tuplets {
        builder = builder.tuplet_group(*start, *end, *ratio);
    }
    if let Some(w) = width_sp {
        builder = builder.justify_to(w);
    } else {
        builder = builder.compact();
    }
    // System pass draws its own barlines: builder barlines use top-line y
    // coords while notes use middle-line coords (chart does the same).
    builder = builder.no_barlines();
    builder.build(&ctx.as_context())
}

/// A barline of `style` ending at `x` in staff coords (top line = 0).
/// Thick strokes sit inside the measure so a final barline ends flush with
/// the staff.
fn barline_scene(x: f64, style: BarStyle, spatium: f64) -> Option<SceneNode> {
    let thin = spatium * 0.16;
    let thick = spatium * 0.5;
    let gap = spatium * 0.4;
    let line = |cx: f64, w: f64| PaintCommand::Line {
        start: Point::new(cx, 0.0),
        end: Point::new(cx, 4.0 * spatium),
        width: w,
        color: Color::BLACK,
        line_cap: Default::default(),
    };
    let cmds = match style {
        BarStyle::None => return None,
        BarStyle::Regular | BarStyle::Dashed => vec![line(x - thin / 2.0, thin)],
        BarStyle::LightLight => vec![line(x - thin * 1.5 - gap, thin), line(x - thin / 2.0, thin)],
        BarStyle::LightHeavy => vec![
            line(x - thick - gap - thin / 2.0, thin),
            line(x - thick / 2.0, thick),
        ],
        BarStyle::HeavyLight => vec![
            line(x - thin - gap - thick / 2.0, thick),
            line(x - thin / 2.0, thin),
        ],
    };
    Some(SceneNode::anonymous_leaf(cmds))
}

fn styled_text(s: &str, font: &TextFont, pos: Point, anchor: TextAnchor) -> SceneNode {
    SceneNode::anonymous_leaf(vec![PaintCommand::Text {
        text: s.to_string(),
        font_family: font.family.clone(),
        font_size: font.size,
        position: pos,
        color: Color::BLACK,
        anchor,
        weight: if font.bold {
            FontWeight::Bold
        } else {
            FontWeight::Normal
        },
        style: if font.italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        },
    }])
}

/// What a piece of a chord's ink is (for tie clearance rules).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InkPart {
    /// A notehead, with its staff line.
    Head(i32),
    Stem,
    Flag,
    Ledger,
    Dot,
    Accidental,
    Other,
}

/// The ink of one built chord, in system coordinates.
#[derive(Debug, Clone, Default)]
struct EntryInk {
    parts: Vec<(Rect, InkPart)>,
}

impl EntryInk {
    fn head(&self, line: i32) -> Option<Rect> {
        self.parts
            .iter()
            .find(|(_, p)| *p == InkPart::Head(line))
            .map(|(r, _)| *r)
    }

    fn has_stem(&self) -> bool {
        self.parts.iter().any(|(_, p)| *p == InkPart::Stem)
    }
}

/// Classify a paint command inside a chord: SMuFL ranges for heads
/// (U+E0A0–E0FF), flags (U+E240–E24F), dots (U+E1E7) and accidentals
/// (U+E260–E2FF); lines are ledgers inside a note, the stem outside.
fn ink_part(cmd: &PaintCommand, in_note: Option<i32>) -> InkPart {
    let glyph = match cmd {
        PaintCommand::Text { text, .. } => text.chars().next(),
        PaintCommand::Glyph { codepoint, .. } => Some(*codepoint),
        PaintCommand::Line { .. } => {
            return if in_note.is_some() {
                InkPart::Ledger
            } else {
                InkPart::Stem
            };
        }
        _ => None,
    };
    let Some(c) = glyph.map(u32::from) else {
        return InkPart::Other;
    };
    match c {
        0xE0A0..=0xE0FF => in_note.map_or(InkPart::Other, InkPart::Head),
        0xE240..=0xE24F => InkPart::Flag,
        0xE1E7 => InkPart::Dot,
        0xE260..=0xE2FF => InkPart::Accidental,
        _ => InkPart::Other,
    }
}

/// Per entry (chords and rests in scene order), the ink of a built
/// measure; `None` for rests. `origin` places the measure in the system.
///
/// Unbeamed chords are `Chord` groups holding their notes, stem and flag.
/// Beamed notes are bare `Note` groups, their stems and beams drawn after
/// them as loose lines and paths; each loose stem is given to the note
/// whose head it touches.
fn entry_inks(scene: &SceneNode, origin: Affine) -> Vec<Option<EntryInk>> {
    fn collect(node: &SceneNode, t: Affine, in_note: Option<i32>, out: &mut EntryInk) {
        let t = t * node.transform;
        let note_line = node
            .get_metadata("pitch-line")
            .and_then(|l| l.parse::<i32>().ok())
            .or(in_note);
        crate::metrics::with_faces(|faces| {
            for cmd in &node.commands {
                if let Some(r) = crate::metrics::command_ink(faces, cmd, t) {
                    out.parts.push((r, ink_part(cmd, note_line)));
                }
            }
        });
        for child in &node.children {
            collect(child, t, note_line, out);
        }
    }
    fn visit(
        node: &SceneNode,
        t: Affine,
        out: &mut Vec<Option<EntryInk>>,
        loose: &mut Vec<(Rect, InkPart)>,
        last_note_x: &mut Option<f64>,
    ) {
        let t = t * node.transform;
        let kind = node.id.as_ref().map(|id| id.element_type);
        if kind != Some(ElementType::Note) && kind.is_some() {
            *last_note_x = None;
        }
        match kind {
            Some(ElementType::Chord | ElementType::Note) => {
                let mut ink = EntryInk::default();
                crate::metrics::with_faces(|faces| {
                    for cmd in &node.commands {
                        if let Some(r) = crate::metrics::command_ink(faces, cmd, t) {
                            let line = node
                                .get_metadata("pitch-line")
                                .and_then(|l| l.parse::<i32>().ok());
                            ink.parts.push((r, ink_part(cmd, line)));
                        }
                    }
                });
                for child in &node.children {
                    collect(child, t, None, &mut ink);
                }
                // A beamed chord draws one Note group per head, all at the
                // chord's x: they are one entry.
                let x = t.translation().x;
                let same_chord = kind == Some(ElementType::Note)
                    && last_note_x.is_some_and(|lx| (lx - x).abs() < 0.5);
                match (same_chord, out.last_mut()) {
                    (true, Some(Some(prev))) => prev.parts.extend(ink.parts),
                    _ => out.push(Some(ink)),
                }
                *last_note_x = (kind == Some(ElementType::Note)).then_some(x);
            }
            Some(ElementType::Rest) => out.push(None),
            _ => {
                crate::metrics::with_faces(|faces| {
                    for cmd in &node.commands {
                        if let Some(r) = crate::metrics::command_ink(faces, cmd, t) {
                            loose.push((r, ink_part(cmd, None)));
                        }
                    }
                });
                for child in &node.children {
                    visit(child, t, out, loose, last_note_x);
                }
            }
        }
    }
    let mut out = Vec::new();
    let mut loose = Vec::new();
    visit(scene, origin, &mut out, &mut loose, &mut None);
    // Loose stems (beamed groups) join the entry whose head they touch.
    for (r, part) in loose {
        if part != InkPart::Stem || r.width() > 1.0 {
            continue;
        }
        let owner = out.iter_mut().flatten().find(|ink| {
            ink.parts.iter().any(|(h, p)| {
                matches!(p, InkPart::Head(_))
                    && r.x0 >= h.x0 - 1.0
                    && r.x1 <= h.x1 + 1.0
                    && r.y0 <= h.y1
                    && r.y1 >= h.y0
            })
        });
        if let Some(ink) = owner {
            ink.parts.push((r, InkPart::Stem));
        }
    }
    out
}

/// Anchors for the tie/slur overlay: one notated measure with the
/// system-local x of each ChordRest segment, in entry order.
struct MeasureAnchors<'a> {
    m: &'a NotatedMeasure,
    entry_x: Vec<f64>,
    /// Where the measure ends (its closing barline), system x.
    x_end: f64,
    /// Per entry: the stem direction the builder chose (`Some(true)` = up).
    stems: Vec<Option<bool>>,
    /// Per entry: the chord's ink (empty when it could not be matched).
    inks: Vec<Option<EntryInk>>,
}

impl MeasureAnchors<'_> {
    /// x for a tick: the entry starting there, or — for a direction between
    /// notes (a dynamic on beat 3 of a whole note) — a point interpolated by
    /// time between its neighbours, as a segment of its own would sit.
    fn x_at(&self, tick: u32) -> f64 {
        let ticks = &self.m.entry_ticks;
        let first = self.entry_x.first().copied().unwrap_or(0.0);
        let Some(prev) = ticks.iter().rposition(|t| *t <= tick) else {
            return first;
        };
        let x0 = self.entry_x.get(prev).copied().unwrap_or(first);
        if ticks[prev] == tick {
            return x0;
        }
        let (t1, x1) = match ticks.get(prev + 1) {
            Some(t) => (
                *t,
                self.entry_x.get(prev + 1).copied().unwrap_or(self.x_end),
            ),
            None => (self.m.len_ticks.max(tick + 1), self.x_end - 1.0),
        };
        let span = f64::from(t1.saturating_sub(ticks[prev]).max(1));
        x0 + (x1 - x0) * f64::from(tick - ticks[prev]) / span
    }
}

/// Map a MusicXML articulation tag to a tlayout articulation type.
fn articulation_type(
    tag: &str,
) -> Option<engraver_proto::engraver::layout::tlayout::ArticulationType> {
    use engraver_proto::engraver::layout::tlayout::ArticulationType as A;
    Some(match tag {
        "staccato" => A::Staccato,
        "staccatissimo" => A::Staccatissimo,
        "accent" => A::Accent,
        "strong-accent" => A::Marcato,
        "tenuto" => A::Tenuto,
        "detached-legato" => A::TenutoStaccato,
        "stress" => A::Stress,
        "unstress" => A::Unstress,
        "soft-accent" => A::SoftAccent,
        _ => return None,
    })
}

/// Map a conventional dynamic name to the tlayout dynamic type.
fn dynamic_type(name: &str) -> engraver_proto::engraver::layout::tlayout::DynamicType {
    use engraver_proto::engraver::layout::tlayout::DynamicType as D;
    match name {
        "ppp" | "pppp" => D::Ppp,
        "pp" => D::Pp,
        "p" => D::P,
        "mp" => D::Mp,
        "mf" => D::Mf,
        "f" => D::F,
        "ff" => D::Ff,
        "fff" | "ffff" => D::Fff,
        "sf" | "sfz" => D::Sfz,
        "sffz" => D::Sffz,
        "fp" => D::Fp,
        "fz" => D::Fz,
        "rf" => D::Rf,
        "rfz" => D::Rfz,
        "sfp" => D::Sfp,
        _ => D::Other,
    }
}

/// Draw a multi-measure rest block: serifs + H-bar on the middle line and
/// the count in SMuFL time-signature digits above the staff.
fn multirest_scene(count: usize, width: f64, spatium: f64, id: u64) -> SceneNode {
    let mut node = SceneNode::group(SemanticId::new(ElementType::Measure, id));
    let mid_y = 2.0 * spatium;
    let pad = 1.6 * spatium;
    let bar_h = spatium * 1.0;
    let mut cmds = Vec::new();
    // H-bar body
    cmds.push(PaintCommand::Rect {
        rect: Rect::new(pad, mid_y - bar_h / 2.0, width - pad, mid_y + bar_h / 2.0),
        fill: Some(Color::BLACK),
        stroke: None,
        stroke_width: 0.0,
        corner_radius: None,
    });
    // End serifs
    for x in [pad, width - pad] {
        cmds.push(PaintCommand::Line {
            start: Point::new(x, mid_y - spatium),
            end: Point::new(x, mid_y + spatium),
            width: spatium * 0.16,
            color: Color::BLACK,
            line_cap: Default::default(),
        });
    }
    // Count in SMuFL timeSig digits, centered above the staff.
    let digits: String = count
        .to_string()
        .chars()
        .filter_map(|c| c.to_digit(10))
        .map(|d| char::from_u32(0xE080 + d).unwrap_or('0'))
        .collect();
    cmds.push(PaintCommand::Text {
        text: digits,
        font_family: "Leland".to_string(),
        font_size: spatium * 4.0,
        position: Point::new(width / 2.0, -1.2 * spatium),
        color: Color::BLACK,
        anchor: TextAnchor::Middle,
        weight: FontWeight::Normal,
        style: FontStyle::Normal,
    });
    node.add_child(SceneNode::anonymous_leaf(cmds));
    node
}

fn staff_lines(width: f64, spatium: f64) -> SceneNode {
    let mut cmds = Vec::new();
    for i in 0..5 {
        let y = f64::from(i) * spatium;
        cmds.push(PaintCommand::Line {
            start: Point::new(0.0, y),
            end: Point::new(width, y),
            width: spatium * 0.1,
            color: Color::BLACK,
            line_cap: Default::default(),
        });
    }
    SceneNode::anonymous_leaf(cmds)
}

/// A layout context whose style spatium matches the staff being drawn —
/// the measure builder sizes noteheads and stems from the style, so it must
/// agree with the staff lines.
fn context_for(spatium: f64) -> LayoutContextOwned {
    use engraver_proto::engraver::style::{MStyle, Sid, StyleValue};
    let mut style = MStyle::lead_sheet();
    style.set(Sid::Spatium, StyleValue::Real(spatium as f32));
    LayoutContextOwned::new_minimal_arc(std::sync::Arc::new(style))
}

/// Distribute `total` across items in proportion to `weights`, never below
/// each item's minimum (items pinned at their minimum drop out and the rest
/// re-share what is left).
fn proportional_widths(weights: &[f64], mins: &[f64], total: f64) -> Vec<f64> {
    let n = weights.len();
    let mut pinned = vec![false; n];
    loop {
        let pinned_sum: f64 = (0..n).filter(|i| pinned[*i]).map(|i| mins[i]).sum();
        let free_weight: f64 = (0..n).filter(|i| !pinned[*i]).map(|i| weights[i]).sum();
        if free_weight <= 0.0 {
            return mins.to_vec();
        }
        let k = (total - pinned_sum).max(0.0) / free_weight;
        let mut changed = false;
        for i in 0..n {
            if !pinned[i] && weights[i] * k < mins[i] {
                pinned[i] = true;
                changed = true;
            }
        }
        if !changed {
            return (0..n)
                .map(|i| if pinned[i] { mins[i] } else { weights[i] * k })
                .collect();
        }
    }
}

/// One system: which prepared items it holds and where it sits.
struct SystemPlan {
    items: Vec<usize>,
    page: usize,
    /// Top staff line, from the page top.
    staff_top: f64,
    left: f64,
    right: f64,
    spatium: f64,
}

/// The title block a part gets when the source supplies no page text:
/// part name left, title centred, composer right on page 1, and a running
/// "Title - Part - Pg. N" header after it.
#[must_use]
pub fn default_page_texts(score: &Score, part_name: &str) -> Vec<PageText> {
    let title = score
        .work_title
        .clone()
        .or_else(|| score.movement_title.clone())
        .unwrap_or_default();
    let mut texts = vec![
        PageText {
            text: part_name.to_string(),
            font: TextFont::serif(14.0),
            first_page: 1,
            last_page: Some(1),
            h_align: HAlign::Left,
            v_align: VAlign::Top,
            x: 0.0,
            y: -14.0,
        },
        PageText {
            text: title.clone(),
            font: TextFont::serif(24.0).bold(),
            first_page: 1,
            last_page: Some(1),
            h_align: HAlign::Center,
            v_align: VAlign::Top,
            x: 0.0,
            y: -17.0,
        },
    ];
    if let Some(composer) = score.composer.clone().or_else(|| score.arranger.clone()) {
        texts.push(PageText {
            text: composer,
            font: TextFont::serif(12.0).bold(),
            first_page: 1,
            last_page: Some(1),
            h_align: HAlign::Right,
            v_align: VAlign::Top,
            x: 0.0,
            y: -6.0,
        });
    }
    if !title.is_empty() {
        texts.push(PageText {
            text: format!("{title} - {part_name} - Pg. {{page}}"),
            font: TextFont::serif(12.0),
            first_page: 2,
            last_page: None,
            h_align: HAlign::Center,
            v_align: VAlign::Top,
            x: 0.0,
            y: -12.0,
        });
    }
    texts
}

fn draw_page_texts(
    root: &mut SceneNode,
    pages: &[PageRect],
    texts: &[PageText],
    opts: &LayoutOptions,
) {
    let margins = opts.margins();
    let total = pages.len();
    for (i, page) in pages.iter().enumerate() {
        let number = i + 1;
        for t in texts {
            if number < t.first_page || t.last_page.is_some_and(|last| number > last) {
                continue;
            }
            let content = t
                .text
                .replace("{page}", &number.to_string())
                .replace("{pages}", &total.to_string());
            if content.trim().is_empty() {
                continue;
            }
            let (x, anchor) = match t.h_align {
                HAlign::Left => (margins.left + t.x, TextAnchor::Start),
                HAlign::Center => (page.width / 2.0 + t.x, TextAnchor::Middle),
                HAlign::Right => (page.width - margins.right + t.x, TextAnchor::End),
            };
            // `y` positions the text's top edge (Top) or its descender line
            // (Bottom); the baseline follows from the face's metrics.
            let baseline = match t.v_align {
                VAlign::Top => margins.top + t.y + 0.8 * t.font.size,
                VAlign::Center => page.height / 2.0 + t.y + 0.3 * t.font.size,
                VAlign::Bottom => page.height - margins.bottom + t.y - 0.22 * t.font.size,
            };
            root.add_child(styled_text(
                &content,
                &t.font,
                Point::new(x, page.y_offset + baseline),
                anchor,
            ));
        }
    }
}

/// Final (page, staff top) of every system.
///
/// MuseScore's `SystemLayout::minDistance`: a system goes no closer to the
/// one above than its north skyline allows against that system's south
/// skyline, plus `minVerticalDistance` (0.5 sp). With a source layout the
/// planned positions are kept unless content collides — then the page's
/// later systems move down, as long as the last one still fits on the page.
/// Automatic layout re-flows pages with the real heights.
fn stack_systems(
    systems: &[SystemPlan],
    rendered: &[(SceneNode, crate::skyline::Skyline)],
    opts: &LayoutOptions,
    page_texts: &[PageText],
    explicit: bool,
) -> Vec<(usize, f64)> {
    let margins = opts.margins();
    // Music stays above the bottom margin and above any footer text on the
    // page (a copyright line, a "Rev." stamp).
    let bottom_limit_of = |page: usize| -> f64 {
        let number = page + 1;
        page_texts
            .iter()
            .filter(|t| t.v_align == VAlign::Bottom)
            .filter(|t| number >= t.first_page && t.last_page.is_none_or(|l| number <= l))
            .map(|t| opts.page_height - margins.bottom + t.y - 1.1 * t.font.size)
            .fold(opts.page_height - margins.bottom, f64::min)
    };
    let needed = |above: usize, below: usize| -> f64 {
        let sp = systems[below].spatium;
        let d = rendered[above]
            .1
            .min_distance_to(&rendered[below].1, 0.25 * sp);
        let floor = 4.0 * sp + 0.5 * sp;
        if d.is_finite() {
            (d + 0.5 * sp).max(floor)
        } else {
            floor
        }
    };
    let mut out: Vec<(usize, f64)> = systems.iter().map(|s| (s.page, s.staff_top)).collect();
    if explicit {
        let mut i = 0;
        while i < systems.len() {
            let page = systems[i].page;
            let end = (i..systems.len())
                .find(|j| systems[*j].page != page)
                .unwrap_or(systems.len());
            let mut tops: Vec<f64> = systems[i..end].iter().map(|s| s.staff_top).collect();
            for k in 1..tops.len() {
                tops[k] = tops[k].max(tops[k - 1] + needed(i + k - 1, i + k));
            }
            let last = end - 1;
            let fits = tops.last().copied().unwrap_or(0.0) + rendered[last].1.south_extent()
                <= bottom_limit_of(page) + 1e-6;
            if fits {
                for (k, t) in tops.into_iter().enumerate() {
                    out[i + k].1 = t;
                }
            }
            i = end;
        }
        return out;
    }
    // Automatic: re-flow with the measured heights.
    let mut page = 0usize;
    for i in 0..systems.len() {
        let sp = systems[i].spatium;
        let sky = &rendered[i].1;
        let mut top = if i == 0 {
            systems[0]
                .staff_top
                .max(margins.top - sky.north_extent() + 0.5 * sp)
        } else {
            out[i - 1].1 + needed(i - 1, i).max(opts.system_gap)
        };
        if i > 0 && top + sky.south_extent() > bottom_limit_of(page) {
            page += 1;
            top = margins.top + (-sky.north_extent()).max(2.0 * sp) + 0.5 * sp;
        }
        out[i] = (page, top);
    }
    out
}

/// Lay out one part into pages.
#[must_use]
pub fn layout_part(score: &Score, part_index: usize, opts: &LayoutOptions) -> PartLayout {
    let part = &score.parts[part_index];
    let margins = opts.margins();

    let mut dropped = 0usize;
    let prepared = prepare_measures(score, part_index, opts, &mut dropped);
    if dropped > 0 {
        tracing::info!(
            part = part.name,
            dropped,
            "non-primary-voice events dropped (P2 single-voice)"
        );
    }

    let base_spatium = opts
        .explicit
        .as_ref()
        .and_then(|e| e.systems.first())
        .map_or(opts.spatium, |s| s.spatium);
    let ctx = context_for(base_spatium);

    // ── measure metrics: min width (compact build) + content weight ──
    //
    // Mirrors the chart engine's spacing model (width_dist.rs): weights come
    // from rhythmic density via `chart_spacing::duration_stretch`, minimum
    // widths from a compact pre-build, and the per-system distribution is the
    // same spring physics (`measure_layout::distribute_measure_widths_spring`).
    // A source with its own measure widths (Finale) replaces the weights.
    const SLOPE: f64 = chart_spacing::DEFAULT_SPACING_SLOPE;
    const DENSITY: f64 = chart_spacing::DEFAULT_SPACING_DENSITY;

    let measure_ticks_of = |time: (u32, u32)| -> f64 {
        f64::from(time.0) * 4.0 * constants::TICKS_PER_QUARTER / f64::from(time.1.max(1))
    };

    struct MeasureMetrics {
        min_width: f64,
        weight: f64,
        /// The source's own width for this item, when it gave one.
        source_width: Option<f64>,
    }
    let explicit = opts.explicit.as_ref();
    let source_width_of = |item: &Prepared| -> Option<f64> {
        let e = explicit?;
        match item {
            Prepared::Notated(m) => e.measure_widths.get(m.index).copied().flatten(),
            Prepared::MultiRest(r) => e
                .multirests
                .iter()
                .find(|mr| mr.measures.start == r.first)
                .map(|mr| mr.width),
        }
    };
    let metrics: Vec<MeasureMetrics> = prepared
        .iter()
        .map(|item| match item {
            Prepared::Notated(m) => {
                let min_width = build_measure(m, &ctx, None, false, false, 1).width;
                let measure_ticks = measure_ticks_of(m.time);
                let note_ticks: Vec<f64> = m
                    .entries
                    .iter()
                    .filter_map(|e| match e {
                        RhythmEntry::Note(d) => Some(f64::from(d.ticks())),
                        RhythmEntry::Rest(_) => None,
                    })
                    .collect();
                let shortest = note_ticks.iter().copied().fold(f64::INFINITY, f64::min);
                let dense = note_ticks.len() >= 4 || shortest <= constants::TICKS_PER_QUARTER / 2.0;
                let duration_weight = if note_ticks.is_empty() || !dense {
                    chart_spacing::duration_stretch(
                        measure_ticks,
                        constants::TICKS_PER_QUARTER,
                        SLOPE,
                    )
                } else {
                    note_ticks
                        .iter()
                        .map(|t| {
                            chart_spacing::duration_stretch(*t, constants::TICKS_PER_QUARTER, SLOPE)
                        })
                        .sum()
                };
                let tuplet_bonus: f64 = m
                    .tuplets
                    .iter()
                    .map(|(start, end, _)| (end - start) as f64 * 0.08)
                    .sum();
                MeasureMetrics {
                    min_width,
                    weight: (duration_weight + tuplet_bonus).max(0.5),
                    source_width: source_width_of(item),
                }
            }
            Prepared::MultiRest(r) => MeasureMetrics {
                min_width: MULTIREST_WIDTH_SP * base_spatium,
                weight: chart_spacing::duration_stretch(
                    measure_ticks_of(r.time),
                    constants::TICKS_PER_QUARTER,
                    SLOPE,
                ),
                source_width: source_width_of(item),
            },
        })
        .collect();

    // ── systems: the source's breaks, or min-width packing ──
    const MAX_MEASURES_PER_SYSTEM: usize = 6;
    let mut systems: Vec<SystemPlan> = Vec::new();
    match explicit.filter(|e| !e.systems.is_empty()) {
        Some(e) => {
            for es in &e.systems {
                systems.push(SystemPlan {
                    items: Vec::new(),
                    page: es.page,
                    staff_top: es.staff_top,
                    left: es.left,
                    right: es.right,
                    spatium: es.spatium,
                });
            }
            for (idx, item) in prepared.iter().enumerate() {
                let first = item.first_index();
                // The system holding this item's first measure (or the last
                // system that starts before it, for measures past the end).
                let sys = e
                    .systems
                    .iter()
                    .rposition(|s| s.measures.start <= first)
                    .unwrap_or(0);
                systems[sys].items.push(idx);
            }
            systems.retain(|s| !s.items.is_empty());
        }
        None => {
            let avail_width = opts.page_width - margins.left - margins.right;
            let mut current: Vec<usize> = Vec::new();
            let mut used = 0.0;
            let mut packed: Vec<Vec<usize>> = Vec::new();
            for idx in 0..prepared.len() {
                let (_, _, _, prefix_w) = calculate_prefix_width(
                    base_spatium,
                    true,
                    true,
                    prepared[idx].key_fifths(),
                    current.is_empty(),
                );
                let w = metrics[idx].min_width;
                let full = current.len() >= MAX_MEASURES_PER_SYSTEM
                    || (!current.is_empty() && used + w > avail_width - prefix_w);
                if full {
                    packed.push(std::mem::take(&mut current));
                    used = 0.0;
                }
                current.push(idx);
                used += w;
            }
            if !current.is_empty() {
                packed.push(current);
            }
            // Vertical flow: fill each page top to bottom, starting below
            // page 1's top-anchored text (a three-line title block needs
            // more than the default title height) plus room for cues.
            let system_height = 4.0 * base_spatium;
            let mut page = 0usize;
            let title_bottom = opts
                .page_texts
                .iter()
                .flatten()
                .filter(|t| t.first_page == 1 && t.v_align == VAlign::Top)
                .map(|t| margins.top + t.y + 1.15 * t.font.size)
                .fold(0.0, f64::max);
            let mut y = (margins.top + opts.title_block_height).max(title_bottom + 46.0);
            for items in packed {
                if y + system_height > opts.page_height - margins.bottom {
                    page += 1;
                    y = margins.top + 30.0;
                }
                systems.push(SystemPlan {
                    items,
                    page,
                    staff_top: y,
                    left: margins.left,
                    right: opts.page_width - margins.right,
                    spatium: base_spatium,
                });
                y += opts.system_gap;
            }
        }
    }

    // Expansion gating, verbatim from the chart engine's width_dist.rs
    // (pub(super) there): only stretch past base width when a measure's
    // rhythm or minimum actually demands it.
    let expansion_stretches = |weights: &[f64], min_widths: &[f64], base: f64| -> Vec<f64> {
        const RHYTHM_EXPANSION_THRESHOLD: f64 = 3.0;
        let has_expander = weights
            .iter()
            .zip(min_widths)
            .any(|(w, m)| *w >= RHYTHM_EXPANSION_THRESHOLD || *m > base * 1.05);
        if !has_expander {
            return vec![1.0; weights.len()];
        }
        weights
            .iter()
            .zip(min_widths)
            .map(|(weight, min_width)| {
                if *weight >= RHYTHM_EXPANSION_THRESHOLD || *min_width > base * 1.05 {
                    weight.max(min_width / base.max(1.0)).max(1.0).sqrt()
                } else {
                    1.0
                }
            })
            .collect()
    };

    let page_y = |page: usize| page as f64 * (opts.page_height + 24.0);
    let mut root = SceneNode::group(SemanticId::new(ElementType::System, 0));
    let page_texts = opts
        .page_texts
        .clone()
        .unwrap_or_else(|| default_page_texts(score, &part.name));
    // Systems are drawn first (each with its skyline), then stacked.
    let mut rendered: Vec<(SceneNode, crate::skyline::Skyline)> = Vec::new();

    let words_font = TextFont {
        family: opts.text_font.clone(),
        size: 11.0,
        bold: false,
        italic: true,
    };
    let numbers = &opts.measure_numbers;
    let first_measure_index = 0usize;

    // ── render systems ──
    let mut id: u64 = 10;
    for (sys_idx, sys) in systems.iter().enumerate() {
        let spatium = sys.spatium;
        let sys_ctx;
        let ctx = if (spatium - base_spatium).abs() < 1e-6 {
            &ctx
        } else {
            sys_ctx = context_for(spatium);
            &sys_ctx
        };
        let mut sys_node =
            SceneNode::group(SemanticId::new(ElementType::System, sys_idx as u64 + 1));

        // Prefix from the first item's attribute state.
        let (first_clef, first_key, first_time, show_time) = match &prepared[sys.items[0]] {
            Prepared::Notated(m) => (m.clef, m.key_fifths, m.time, sys_idx == 0 || m.time_changed),
            Prepared::MultiRest(r) => (r.clef, r.key_fifths, r.time, sys_idx == 0),
        };
        let (_, key_w, time_w, _) =
            calculate_prefix_width(spatium, true, true, first_key, show_time);
        // The chart helper leaves 0.5 sp after a key signature; a part's
        // time signature wants a full space.
        let key_w = if key_w > 0.0 {
            key_w + 0.5 * spatium
        } else {
            0.0
        };
        // The chart helper sizes every clef as a treble clef, and the
        // tlayout table under-sizes the C clef (2.2 sp; Bravura's cClef
        // advance is 2.8): use the glyph advances, then a 1 sp gap so a
        // viola part's 4/4 does not touch its clef.
        let clef_advance = match first_clef {
            ClefType::Alto | ClefType::Tenor => 2.8,
            ClefType::Bass => 2.76,
            other => other.width(),
        };
        let clef_w = (clef_advance + 1.0) * spatium;
        let prefix = render_system_prefix(
            &PrefixRenderContext {
                x: 0.0,
                staff_y: 0.0,
                spatium,
                include_clef: true,
                clef_type: first_clef,
                include_key_sig: true,
                include_time_sig: show_time,
                key_signature: first_key,
                key_sig_color: None,
                time_signature: (first_time.0 as u8, first_time.1 as u8),
                clef_width: clef_w,
                key_sig_width: key_w,
                time_sig_width: time_w,
                page_number: None,
            },
            id,
            &ctx.as_context(),
        );
        id = prefix.next_id + 100;
        let prefix_w = prefix.total_width;

        let measures_area = (sys.right - sys.left) - prefix_w;
        let min_widths: Vec<f64> = sys.items.iter().map(|&i| metrics[i].min_width).collect();
        let source_widths: Option<Vec<f64>> =
            sys.items.iter().map(|&i| metrics[i].source_width).collect();
        let widths = match source_widths {
            // The source's springs: Finale justifies each system by scaling
            // its measure widths uniformly.
            Some(sw) => proportional_widths(&sw, &min_widths, measures_area),
            None => {
                let weights: Vec<f64> = sys.items.iter().map(|&i| metrics[i].weight).collect();
                let base_measure_width = measures_area / MAX_MEASURES_PER_SYSTEM as f64;
                let is_last_system = sys_idx + 1 == systems.len();
                let min_sum: f64 = min_widths.iter().sum();
                // Every system justifies to the full line except a sparse
                // final one (auto layout only — a source's breaks are kept).
                let total_to_distribute = if explicit.is_none()
                    && is_last_system
                    && sys.items.len() < MAX_MEASURES_PER_SYSTEM
                    && min_sum < measures_area * 0.7
                {
                    (sys.items.len() as f64 * base_measure_width).max(min_sum)
                } else {
                    measures_area
                };
                let stretches = expansion_stretches(&weights, &min_widths, base_measure_width);
                measure_layout::distribute_measure_widths_spring(
                    &stretches,
                    0,
                    total_to_distribute,
                    0.4,
                    base_measure_width,
                    &min_widths,
                    spatium,
                    SLOPE,
                    DENSITY,
                )
            }
        };

        let content_width: f64 = widths.iter().sum();
        sys_node.add_child(staff_lines(prefix_w + content_width, spatium));
        for node in prefix.nodes {
            sys_node.add_child(node);
        }

        let mut anchors: Vec<MeasureAnchors<'_>> = Vec::new();
        // Per item: (x start, x end) in system coords.
        let mut spans: Vec<(f64, f64)> = Vec::new();

        let mut x = prefix_w;
        for (item_pos, &item_idx) in sys.items.iter().enumerate() {
            let width = widths[item_pos];
            let mut container =
                SceneNode::group(SemanticId::new(ElementType::Measure, item_idx as u64 + 1));
            match &prepared[item_idx] {
                Prepared::Notated(m) if m.is_bar_rest() => {
                    // A whole-bar rest is centred in its bar, hanging from
                    // the second line — not set at the first beat.
                    anchors.push(MeasureAnchors {
                        m,
                        entry_x: vec![x + width / 2.0],
                        x_end: x + width,
                        stems: vec![None],
                        inks: Vec::new(),
                    });
                    container.transform = Affine::translate((x, 0.0));
                    let glyph_w = 1.13 * spatium;
                    container.add_child(SceneNode::anonymous_leaf(vec![PaintCommand::glyph(
                        engraver_proto::engraver::layout::tlayout::rest::glyphs::REST_WHOLE,
                        Point::new((width - glyph_w) / 2.0, spatium),
                        spatium,
                        Color::BLACK,
                    )]));
                }
                Prepared::Notated(m) => {
                    let scene = build_measure(
                        m,
                        ctx,
                        Some(width / spatium),
                        m.clef_changed && item_pos > 0,
                        m.time_changed && !(item_pos == 0 && show_time),
                        id,
                    );
                    id += 1000;
                    let entry_x = engraver_proto::engraver::layout::chart::chord_layout::get_chord_rest_positions(&scene)
                        .into_iter()
                        .map(|seg_x| x + seg_x)
                        .collect();
                    // Chords and rests in scene order map onto entries
                    // one to one; if they do not, drop the ink rather
                    // than misattribute it.
                    let mut inks = entry_inks(&scene.scene, Affine::translate((x, 2.0 * spatium)));
                    if inks.len() != m.entries.len() {
                        tracing::debug!(
                            inks = inks.len(),
                            entries = m.entries.len(),
                            number = m.number,
                            "entry ink mismatch"
                        );
                        inks.clear();
                    }
                    anchors.push(MeasureAnchors {
                        m,
                        entry_x,
                        x_end: x + width,
                        stems: scene.entry_stem_up.clone(),
                        inks,
                    });
                    // MeasureScene notation is authored around the MIDDLE
                    // line (y=0) — same +2sp shift the chart pipeline applies
                    // to its measure containers.
                    container.transform = Affine::translate((x, 2.0 * spatium));
                    container.add_child(scene.scene.clone());
                }
                Prepared::MultiRest(r) => {
                    container.transform = Affine::translate((x, 0.0));
                    container.add_child(multirest_scene(r.count, width, spatium, id));
                    id += 10;
                }
            }
            sys_node.add_child(container);
            spans.push((x, x + width));
            // Advance by the ALLOCATED width (chart does the same): any
            // spring-justification shortfall stays inside this measure's
            // slot instead of accumulating as a gap at the end of the line.
            x += width;
            if let Some(bar) = barline_scene(x, prepared[item_idx].barline(), spatium) {
                sys_node.add_child(bar);
            }
        }

        // ── measure numbers (positions from the number style; placed with
        // everything else below) ──
        let mut number_marks: Vec<vertical::NumberMark> = Vec::new();
        for (item_pos, &item_idx) in sys.items.iter().enumerate() {
            let (start, end) = spans[item_pos];
            let item = &prepared[item_idx];
            let is_first_of_piece = item.first_index() == first_measure_index;
            let number = match item {
                Prepared::Notated(m) => m.number.clone(),
                Prepared::MultiRest(r) => r.number.clone(),
            };
            match numbers.mode {
                MeasureNumberMode::None => {}
                MeasureNumberMode::SystemStart => {
                    if item_pos != 0 || (is_first_of_piece && numbers.exclude_first) || sys_idx == 0
                    {
                        continue;
                    }
                    number_marks.push(vertical::NumberMark {
                        text: number,
                        font: numbers.font.clone(),
                        pos: Point::new(numbers.offset.0, numbers.offset.1),
                        anchor: TextAnchor::Start,
                    });
                }
                MeasureNumberMode::Every(every) => match item {
                    Prepared::MultiRest(r) if numbers.multirest_ranges => {
                        number_marks.push(vertical::NumberMark {
                            text: format!("{}-{}", r.number, r.last_number),
                            font: numbers.multirest_font.clone(),
                            pos: Point::new(
                                (start + end) / 2.0 + numbers.multirest_offset.0,
                                numbers.multirest_offset.1,
                            ),
                            anchor: TextAnchor::Middle,
                        });
                    }
                    _ => {
                        let shown = number.parse::<u32>().is_ok_and(|v| v % every.max(1) == 0)
                            && !(is_first_of_piece && numbers.exclude_first);
                        if shown {
                            // At the measure's left barline (past the prefix
                            // on a system's first measure).
                            number_marks.push(vertical::NumberMark {
                                text: number,
                                font: numbers.font.clone(),
                                pos: Point::new(start + numbers.offset.0, numbers.offset.1),
                                anchor: TextAnchor::Middle,
                            });
                        }
                    }
                },
            }
        }

        // ── vertical placement: ties, articulations, slurs, dynamics,
        // staff text, measure numbers, rehearsal marks — each pushed clear
        // of the staff skyline in MuseScore's order ──
        let items: Vec<vertical::ItemMarks<'_>> = sys
            .items
            .iter()
            .enumerate()
            .map(|(item_pos, &item_idx)| vertical::ItemMarks {
                span: spans[item_pos],
                first: item_pos == 0,
                directions: prepared[item_idx].directions(),
                notated: matches!(prepared[item_idx], Prepared::Notated(_)),
            })
            .collect();
        let placed = vertical::place_system(
            &sys_node.children,
            &vertical::SystemInput {
                spatium,
                width: prefix_w + content_width,
                content_start: prefix_w,
                anchors: &anchors,
                items,
                numbers: number_marks,
                text_family: &opts.text_font,
                words_font: &words_font,
                ctx,
            },
            &mut id,
        );
        for node in placed.nodes {
            sys_node.add_child(node);
        }

        rendered.push((sys_node, placed.skyline));
    }

    // ── stack systems: each clears the one above by its skylines ──
    let placements = stack_systems(&systems, &rendered, opts, &page_texts, explicit.is_some());
    let page_count = placements.iter().map(|(p, _)| p + 1).max().unwrap_or(1);
    let pages: Vec<PageRect> = (0..page_count)
        .map(|i| PageRect {
            x_offset: 0.0,
            y_offset: page_y(i),
            width: opts.page_width,
            height: opts.page_height,
        })
        .collect();
    {
        let mut block = SceneNode::group(SemanticId::new(ElementType::Text, 1));
        draw_page_texts(&mut block, &pages, &page_texts, opts);
        root.add_child(block);
    }
    for ((mut node, _), (sys, (page, top))) in
        rendered.into_iter().zip(systems.iter().zip(placements))
    {
        node.transform = Affine::translate((sys.left, page_y(page) + top));
        root.add_child(node);
    }

    PartLayout {
        scene: root,
        pages,
        dropped_voice_events: dropped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proportional_widths_scale_uniformly() {
        let w = proportional_widths(&[252.0, 378.0, 252.0, 252.0], &[10.0; 4], 1134.0);
        let k = w[0] / 252.0;
        assert!((w[1] / 378.0 - k).abs() < 1e-9);
        assert!((w.iter().sum::<f64>() - 1134.0).abs() < 1e-9);
    }

    #[test]
    fn proportional_widths_respect_minimums() {
        let w = proportional_widths(&[1.0, 1.0, 10.0], &[50.0, 5.0, 5.0], 120.0);
        assert!((w[0] - 50.0).abs() < 1e-9, "{w:?}");
        assert!((w.iter().sum::<f64>() - 120.0).abs() < 1e-9);
        assert!((w[2] / w[1] - 10.0).abs() < 1e-9);
    }
}
