//! chordsheet.com [`Document`] → keyflow [`Chart`].
//!
//! What survives, and what does not, is listed in the [crate docs](crate).
//! The short version: everything musical crosses over, and everything that is
//! purely about how the page looks — palette colors, rulers, TAB rows, page
//! breaks, bar indents, chord-diagram definitions — is dropped. The AST keeps
//! all of it, so a future consumer that cares can read it from there.

use keyflow_proto::chart::notations::{BarlineStyle, Placement, RepeatMark, StaffText, Volta};
use keyflow_proto::chart::types::{
    ChartSection, ChordInstance, Measure, RestInstance, RhythmElement, SpaceInstance,
};
use keyflow_proto::chord::{Chord, ChordRhythm, PushPullAmount, PushPullBase};
use keyflow_proto::core::duration::NotationDuration;
use keyflow_proto::key::Key;
use keyflow_proto::metadata::SongMetadata;
use keyflow_proto::primitives::RootNotation;
use keyflow_proto::sections::{Section, SectionType};
use keyflow_proto::time::{
    AbsolutePosition, MusicalDuration, MusicalPosition, Tempo, TimeSignature,
};
use keyflow_proto::Chart;
use keyflow_syntax::parsing::Lexer;

use crate::ast::*;

/// Everything the chordsheet.com *source text* cannot tell us.
///
/// The site keeps the title, artist, key, tempo and song-wide time signature
/// in database columns, not in the chord data — so a bare `.txt` from an
/// account backup has none of them. Fill these in from the backup manifest
/// ([`crate::manifest`]) or from the filename ([`ImportOptions::from_stem`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImportOptions {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub key: Option<Key>,
    pub tempo: Option<Tempo>,
    /// The song-wide time signature. Per-bar `3:4` markers override it from
    /// the bar they appear on. Defaults to 4/4, which is the site's default.
    pub time_signature: Option<TimeSignature>,
}

impl ImportOptions {
    /// Recover title and artist from a backup filename stem.
    ///
    /// The backup writes `ARTIST - TITLE.txt`. A stem with no ` - ` is taken
    /// as the title alone, and a trailing `[123456]` (the site's song id,
    /// which the backup appends when two songs share a name) is stripped.
    #[must_use]
    pub fn from_stem(stem: &str) -> Self {
        let stem = match stem.rsplit_once(" [") {
            Some((head, tail))
                if tail
                    .trim_end_matches(']')
                    .chars()
                    .all(|c| c.is_ascii_digit()) =>
            {
                head
            }
            _ => stem,
        }
        .trim();

        match stem.split_once(" - ") {
            Some((artist, title)) => Self {
                title: Some(title.trim().to_string()),
                artist: Some(artist.trim().to_string()),
                ..Self::default()
            },
            None => Self {
                title: Some(stem.to_string()),
                ..Self::default()
            },
        }
    }
}

/// Translate a parsed chordsheet.com document into a keyflow [`Chart`].
#[must_use]
pub fn to_chart(document: &Document, options: &ImportOptions) -> Chart {
    Converter::new(options).run(document)
}

struct Converter<'a> {
    options: &'a ImportOptions,
    chart: Chart,
    /// The section being filled. `None` until the first title line — bars
    /// before any title land in an implicit intro.
    section: Section,
    measures: Vec<Measure>,
    /// Text lines seen since the last measure, waiting for one to attach to.
    pending_text: Vec<PendingText>,
    /// Index into `measures` (or, once the section is flushed, into the next
    /// section) where an open `(` wants its forward repeat mark.
    repeat_open: Option<usize>,
    /// Volta numbers waiting for the measure that starts the ending.
    pending_volta: Option<Vec<u8>>,
    /// Index of the measure a volta bracket opened on, so its length can be
    /// closed out when the next ending or the repeat close arrives.
    open_volta: Option<usize>,
    /// Set by a `=` title or a `||` marker: the *previous* measure closes with
    /// a double line.
    pending_double_bar: bool,
    time_signature: (u8, u8),
    /// The meter in force when the first bar was written. chordsheet.com has
    /// no song-wide time signature in the source — charts that aren't in 4/4
    /// open with a `6:8` on the first bar line — so this becomes the chart's
    /// header meter when the caller supplied none.
    first_time_signature: Option<(u8, u8)>,
    /// Whether a section title has been seen. Bars before the first one land
    /// in an implicit intro.
    started: bool,
}

struct PendingText {
    text: String,
    boxed: bool,
    bold: bool,
}

impl<'a> Converter<'a> {
    fn new(options: &'a ImportOptions) -> Self {
        let ts = options
            .time_signature
            .map(|ts| (ts.numerator as u8, ts.denominator as u8))
            .unwrap_or((4, 4));
        Self {
            options,
            chart: Chart::new(),
            section: Section::new(SectionType::Intro),
            measures: Vec::new(),
            pending_text: Vec::new(),
            repeat_open: None,
            pending_volta: None,
            open_volta: None,
            pending_double_bar: false,
            time_signature: ts,
            first_time_signature: None,
            started: false,
        }
    }

    fn run(mut self, document: &Document) -> Chart {
        for line in &document.lines {
            self.line(line);
        }
        self.flush_section();

        self.chart.metadata = SongMetadata {
            title: self.options.title.clone(),
            artist: self.options.artist.clone(),
            tempo: self.options.tempo.map(|t| t.bpm as u32),
            ..SongMetadata::new()
        };
        self.chart.initial_key = self.options.key.clone();
        self.chart.current_key = self.options.key.clone();
        self.chart.tempo = self.options.tempo;
        let initial = self.options.time_signature.unwrap_or_else(|| {
            let (numerator, denominator) = self.first_time_signature.unwrap_or((4, 4));
            TimeSignature::new(u32::from(numerator), u32::from(denominator))
        });
        self.chart.initial_time_signature = Some(initial);
        self.chart.time_signature = Some(initial);
        self.chart
    }

    fn line(&mut self, line: &Line) {
        for label in &line.floating {
            self.pending_text.push(PendingText {
                text: label.text.clone(),
                boxed: label.boxed,
                bold: false,
            });
        }

        match &line.kind {
            // Comments, and everything that is purely page furniture.
            LineKind::Comment(_)
            | LineKind::TabRow { .. }
            | LineKind::Ruler { .. }
            | LineKind::DiagramDefs { .. } => {}

            LineKind::Text { columns } => {
                let text = unquote(&columns.join("  "));
                if !text.is_empty() {
                    self.pending_text.push(PendingText {
                        text,
                        boxed: false,
                        bold: false,
                    });
                }
            }
            LineKind::Literal(text) => {
                let text = unquote(text);
                if !text.is_empty() {
                    self.pending_text.push(PendingText {
                        text,
                        boxed: false,
                        bold: false,
                    });
                }
            }

            LineKind::SectionTitle { text, delimiter } => {
                self.start_section(text, *delimiter);
            }
            // `+ Chorus` is shorthand for a page break plus `= Chorus`. The
            // page break itself is layout; the title is not.
            LineKind::PageBreak { title } => {
                if let Some(title) = title {
                    self.start_section(title, true);
                }
            }

            LineKind::Bars(items) => {
                for item in items {
                    self.item(item);
                }
            }
        }
    }

    // ── Section handling ────────────────────────────────────────────────

    fn start_section(&mut self, title: &str, delimiter: bool) {
        let header = SectionHeader::parse(title);
        if delimiter {
            // `=` closes the previous bar with a double line.
            if let Some(last) = self.measures.last_mut() {
                last.end_barline = BarlineStyle::LightLight;
            }
        }
        self.flush_section();

        if let Some(ts) = header.time_signature {
            self.time_signature = ts;
        }
        if let Some(bpm) = header.tempo {
            let tempo = Tempo::from_bpm(f64::from(bpm));
            if self.chart.tempo.is_none() {
                self.chart.tempo = Some(tempo);
            }
        }

        let mut section = Section::new(header.section_type);
        section.number = header.number;
        section.comment = header.comment;
        section.name = Some(title.trim().to_string());
        self.section = section;
        self.started = true;
    }

    fn flush_section(&mut self) {
        // Text written after the last bar of a section still belongs to it.
        self.drain_pending_text_onto_last();

        let mut recalled = false;
        if self.measures.is_empty() {
            if !self.started {
                return;
            }
            // A title with no bars under it — `= CHORUS`, or `= VERSE 2`
            // followed by nothing but `-"Same as VERSE 1"`. On the page that
            // is a header over empty space; to a player it means "the chorus
            // again". Recall the last section of the same type so the chart
            // has the music the form promises, and keep whatever the author
            // wrote as the section comment.
            let recall = self
                .chart
                .sections
                .iter()
                .rev()
                .find(|s| {
                    s.section.section_type == self.section.section_type && !s.measures().is_empty()
                })
                .map(|s| s.measures().to_vec());
            match recall {
                Some(measures) => {
                    self.measures = measures;
                    recalled = true;
                }
                None => {
                    // Nothing to recall — a form-only chart, all headers and
                    // no chords, which is a real way people use the site. Give
                    // the section one empty bar: keyflow has no way to write a
                    // section of zero measures, and dropping the header would
                    // throw away the only thing the chart says.
                    let mut placeholder = Measure::new();
                    placeholder.time_signature = self.time_signature;
                    self.measures.push(placeholder);
                }
            }
        }

        let measures = std::mem::take(&mut self.measures);
        let mut section = std::mem::replace(&mut self.section, Section::new(SectionType::Intro));
        section.measure_count = Some(measures.len());
        let mut chart_section = ChartSection::new(section).with_measures(measures);
        chart_section.from_template = recalled;
        self.chart.sections.push(chart_section);
        self.repeat_open = None;
        self.open_volta = None;
        self.pending_volta = None;
    }

    fn drain_pending_text_onto_last(&mut self) {
        if self.pending_text.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.pending_text);
        if let Some(last) = self.measures.last_mut() {
            for p in pending {
                last.staff_text.push(staff_text(p, 1));
            }
        } else {
            // No bars to hang it on — fold it into the section comment so it
            // is not silently lost.
            let joined = pending
                .into_iter()
                .map(|p| p.text)
                .collect::<Vec<_>>()
                .join(" · ");
            self.section.comment = Some(match self.section.comment.take() {
                Some(prev) => format!("{prev} · {joined}"),
                None => joined,
            });
        }
    }

    // ── Bar-line items ──────────────────────────────────────────────────

    fn item(&mut self, item: &Item) {
        match item {
            Item::RepeatOpen { .. } => {
                self.repeat_open = Some(self.measures.len());
            }
            Item::RepeatClose { times, .. } => self.close_repeat(*times),
            Item::DoubleBar { .. } => {
                if let Some(last) = self.measures.last_mut() {
                    last.end_barline = BarlineStyle::LightLight;
                } else {
                    self.pending_double_bar = true;
                }
            }
            Item::Ending { numbers, .. } => {
                self.close_open_volta();
                self.pending_volta = Some(numbers.clone());
            }
            Item::TimeSignature {
                numerator,
                denominator,
                ..
            } => {
                self.time_signature = (*numerator, *denominator);
                let position = AbsolutePosition::new(
                    MusicalPosition::new(self.measures.len() as i32, 0, 0),
                    self.chart.sections.len(),
                );
                self.chart.time_signature_changes.push(
                    keyflow_proto::chart::types::TimeSignatureChange::new(
                        position,
                        TimeSignature::new(u32::from(*numerator), u32::from(*denominator)),
                        self.chart.sections.len(),
                    ),
                );
            }
            Item::Navigation { marker, .. } => {
                self.pending_text.push(PendingText {
                    text: marker.to_string(),
                    boxed: true,
                    bold: true,
                });
            }
            // Layout only.
            Item::Indent { .. } | Item::Palette { .. } => {}
            Item::Bar(bar) => self.bar(bar),
        }
    }

    fn close_repeat(&mut self, times: Option<u32>) {
        let Some(start) = self.repeat_open.take() else {
            return;
        };
        if start >= self.measures.len() {
            return;
        }
        self.close_open_volta();

        // `( … )` and `( … )x4` are both repeat signs, and stay repeat signs.
        //
        // Writing the passes out here would throw the shape away before anyone
        // could ask for it: `(Cm,,, Gm,,, Bb,,, Gm,Bb,)x4` is a four-bar phrase
        // played four times, and a chart that says so reads better than
        // sixteen bars that leave the reader to notice. Expanding is the
        // *default* chart mode's job, and it can expand from this; folding
        // cannot un-expand.
        self.measures[start].start_repeat = RepeatMark::Forward;
        let passes = times.unwrap_or(2).max(2) as usize;
        if let Some(last) = self.measures.last_mut() {
            last.end_repeat = RepeatMark::Backward;
            last.repeat_count = passes;
        }
    }

    fn close_open_volta(&mut self) {
        let Some(start) = self.open_volta.take() else {
            return;
        };
        let length = self.measures.len().saturating_sub(start).max(1);
        if let Some(volta) = self.measures[start].volta_start.as_mut() {
            volta.length_measures = length as u16;
        }
    }

    fn bar(&mut self, bar: &Bar) {
        // `%` / `%%` mark the preceding bar(s) as repeated; `%1` / `%2` / `%4`
        // reprint them outright. Neither has a keyflow counterpart, so both
        // become the chords they stand for — the shorthand is lost, the music
        // is not.
        // `%` and `%%` are simile marks: one bar drawn as the mark for each
        // bar they stand for. `%1` / `%2` / `%4` are *reprints* — the site
        // redraws those bars in full — so they come out written.
        if bar.cells.is_empty() {
            if let Some(count) = bar.simile {
                self.reprint(usize::from(count), true);
                return;
            }
            if let Some(count) = bar.reprint {
                self.reprint(count as usize, false);
                return;
            }
        }

        let mut measure = self.measure_from_bar(bar);
        measure.time_signature = self.time_signature;
        self.first_time_signature.get_or_insert(self.time_signature);

        if self.pending_double_bar {
            measure.end_barline = BarlineStyle::LightLight;
            self.pending_double_bar = false;
        }
        if let Some(numbers) = self.pending_volta.take() {
            measure.volta_start = Some(Volta {
                numbers,
                label: String::new(),
                length_measures: 1,
            });
            self.open_volta = Some(self.measures.len());
        }
        for pending in std::mem::take(&mut self.pending_text) {
            measure.staff_text.push(staff_text(pending, 1));
        }

        self.measures.push(measure);
    }

    /// Copy the last `count` measures forward — what `%`, `%%`, `%1`, `%2`
    /// and `%4` all ultimately mean.
    fn reprint(&mut self, count: usize, simile: bool) {
        let len = self.measures.len();
        let start = len.saturating_sub(count.max(1));
        if start == len {
            return;
        }
        let source: Vec<Measure> = self.measures[start..len].to_vec();
        for mut m in source {
            m.start_repeat = RepeatMark::None;
            m.end_repeat = RepeatMark::None;
            m.repeat_count = 1;
            m.volta_start = None;
            m.staff_text.clear();
            m.end_barline = BarlineStyle::Normal;
            m.time_signature = self.time_signature;
            m.simile = simile;
            self.measures.push(m);
        }
    }

    fn measure_from_bar(&mut self, bar: &Bar) -> Measure {
        let mut measure = Measure::new();
        let (numerator, denominator) = self.time_signature;
        measure.time_signature = self.time_signature;
        let beat = beat_ticks(denominator);
        let bar_ticks = beat * u32::from(numerator.max(1));

        let spans = cell_ticks(bar.cells.len(), self.time_signature);
        let mut tick = 0u32;
        let mut has_rest = false;

        for (cell, span) in bar.cells.iter().zip(spans.iter().copied()) {
            let at_beat = (tick / beat + 1).min(255) as u8;
            if let Some(text) = &cell.annotation {
                measure.staff_text.push(staff_text(
                    PendingText {
                        text: unquote(text),
                        boxed: false,
                        bold: false,
                    },
                    at_beat,
                ));
            }

            if let Some(rest) = cell.rest {
                has_rest = true;
                // A rest names its own length — but only a rest that has the
                // bar to itself gets to claim it. Sharing a bar, it takes the
                // slot the even split gave it, like every other cell: an `r1`
                // that grabbed a whole bar from inside a seven-cell bar would
                // push the bar to nearly double length, and the `.kf` would
                // re-parse as two measures.
                let span = if bar.cells.len() == 1 {
                    match rest {
                        Rest::Whole => bar_ticks,
                        Rest::Half => bar_ticks / 2,
                        Rest::Quarter => beat,
                    }
                } else {
                    span
                };
                let duration = note_value_for(span).as_rest();
                measure
                    .rhythm_elements
                    .push(RhythmElement::Rest(RestInstance::new(
                        ChordRhythm::Explicit(duration),
                        duration_for(span, denominator),
                        position_at(tick, denominator),
                        // Deliberately no original token. `r1` in the source
                        // means "the bar", and a rest sharing a bar with six
                        // other cells is not the bar — the exporter names the
                        // rest from the duration instead, and the token as
                        // written is still in the AST.
                        String::new(),
                    )));
                tick += span;
                continue;
            }

            let Some(symbol) = cell.symbol.as_deref() else {
                // A `*` placeholder, or a slot that carries only a stroke:
                // nothing sounds, but the bar still moves.
                measure
                    .rhythm_elements
                    .push(RhythmElement::Space(SpaceInstance::new(
                        ChordRhythm::Explicit(note_value_for(span).as_space()),
                        duration_for(span, denominator),
                        position_at(tick, denominator),
                        String::new(),
                    )));
                tick += span;
                continue;
            };

            if let Some(chord) = self.chord_instance(cell, symbol, span, bar_ticks, tick) {
                measure
                    .rhythm_elements
                    .push(RhythmElement::Chord(chord.clone()));
                measure.chords.push(chord);
            }
            tick += span;
        }

        // `rhythm_elements` is the engraver's override for the whole bar, so
        // it only earns its keep when something in the bar is silent. A bar of
        // plain chords reads better from `chords` alone.
        if !has_rest {
            measure.rhythm_elements.clear();
        }
        measure
    }

    fn chord_instance(
        &mut self,
        cell: &Cell,
        symbol: &str,
        span: u32,
        bar_ticks: u32,
        tick: u32,
    ) -> Option<ChordInstance> {
        let mut lexer = Lexer::new(symbol.to_string());
        let tokens = lexer.tokenize();
        let parsed = match Chord::parse(&tokens) {
            Ok(chord) => chord,
            Err(err) => {
                tracing::debug!(symbol, %err, "chordsheet: unparsable chord symbol, skipped");
                return None;
            }
        };
        let root = RootNotation::from_string(root_of(symbol))?;
        let denominator = self.time_signature.1;

        let mut chord = ChordInstance::new(
            root,
            symbol.to_string(),
            parsed,
            rhythm_for(span, bar_ticks, denominator, cell.tie),
            symbol.to_string(),
            duration_for(span, denominator),
            position_at(tick, denominator),
        );

        if cell.optional {
            chord.display_override = Some(format!("({symbol})"));
        }
        if cell.fermata {
            chord
                .commands
                .push(keyflow_proto::chart::commands::Command::Fermata);
        }
        if let Some(push) = cell.push {
            chord.push_pull = Some((
                true,
                PushPullAmount {
                    level: match push {
                        Push::Eighth => 1,
                        Push::Sixteenth => 2,
                    },
                    base: PushPullBase::Standard,
                },
            ));
        }
        Some(chord)
    }
}

/// Text lines are often written already quoted — `-"The song starts on the &
/// of 3"`. The quotes are the author's, not the format's, and keyflow adds
/// its own when it writes the cue back out.
fn unquote(text: &str) -> String {
    let text = text.trim();
    match text.strip_prefix('"').and_then(|t| t.strip_suffix('"')) {
        Some(inner) if !inner.contains('"') => inner.trim().to_string(),
        _ => text.to_string(),
    }
}

fn position_at(ticks: u32, denominator: u8) -> AbsolutePosition {
    let beat = beat_ticks(denominator);
    AbsolutePosition::new(
        MusicalPosition::new(
            0,
            (ticks / beat) as i32,
            ((ticks % beat) * 1000 / beat).min(999) as i32,
        ),
        0,
    )
}

fn staff_text(pending: PendingText, beat: u8) -> StaffText {
    StaffText {
        text: pending.text,
        beat,
        placement: Placement::Above,
        source_default_x: None,
        boxed: pending.boxed,
        bold: pending.bold,
        italic: false,
    }
}

/// How much of a bar each cell gets, in ticks (a quarter note is 48).
///
/// chordsheet.com divides a bar evenly among whatever is written in it — a
/// bar of eight chords draws eight chords in one bar's width, and the commas
/// that mark strokes are slot markers, not durations. So `Dm7_G7` is two
/// halves and `A,A,A,A,A,G#,C#C#` (the *Africa* riff) is eight eighths.
///
/// When the split isn't exact the leftover ticks go to the last cells, so the
/// spans always add up to the bar. A bar that doesn't add up re-parses as two
/// measures, which is how a `.kf` export silently doubles a section's length.
fn cell_ticks(cell_count: usize, (numerator, denominator): (u8, u8)) -> Vec<u32> {
    if cell_count == 0 {
        return Vec::new();
    }
    let beat = beat_ticks(denominator);
    let bar = beat * u32::from(numerator.max(1));

    // Find the coarsest grid that has a slot for every cell: beats first,
    // then eighths of a beat at the finest.
    let mut unit = beat;
    let mut slots = bar / unit;
    while slots < cell_count as u32 && unit > 1 {
        unit /= 2;
        slots = bar / unit;
    }

    let cells = cell_count as u32;
    if slots < cells {
        // More cells than a 32nd-note grid can hold. Give each one the
        // smallest unit and let the bar overflow — the alternative is
        // silently dropping chords the author wrote.
        return vec![unit; cell_count];
    }

    let base = slots / cells;
    let remainder = slots % cells;
    (0..cells)
        .map(|i| {
            let extra = u32::from(i >= cells - remainder);
            (base + extra) * unit
        })
        .collect()
}

/// Ticks in one beat, where a quarter note is 48.
fn beat_ticks(denominator: u8) -> u32 {
    match denominator {
        1 => 192,
        2 => 96,
        4 => 48,
        8 => 24,
        16 => 12,
        32 => 6,
        _ => 48,
    }
}

/// A tick span as a keyflow rhythm.
///
/// A whole bar keeps [`ChordRhythm::Default`] so the exporter writes a bare
/// chord symbol; a whole number of beats becomes slashes; anything shorter
/// needs an explicit note value.
fn rhythm_for(ticks: u32, bar_ticks: u32, denominator: u8, tied: bool) -> ChordRhythm {
    if ticks >= bar_ticks && !tied {
        return ChordRhythm::Default;
    }
    let beat = beat_ticks(denominator);
    if ticks.is_multiple_of(beat) {
        return ChordRhythm::Slashes {
            count: (ticks / beat).clamp(1, 255) as u8,
            dotted: false,
            tied,
        };
    }
    let mut duration = note_value_for(ticks);
    if tied {
        duration = duration.tied();
    }
    ChordRhythm::Explicit(duration)
}

fn note_value_for(ticks: u32) -> NotationDuration {
    use keyflow_proto::core::duration::NoteValue;
    const PLAIN: [(u32, NoteValue); 6] = [
        (192, NoteValue::Whole),
        (96, NoteValue::Half),
        (48, NoteValue::Quarter),
        (24, NoteValue::Eighth),
        (12, NoteValue::Sixteenth),
        (6, NoteValue::ThirtySecond),
    ];
    for (base, value) in PLAIN {
        if ticks == base {
            return NotationDuration::new(value);
        }
        if ticks == base + base / 2 {
            return NotationDuration::new(value).dotted();
        }
    }
    // Not a notatable value — take the largest one that fits, so the chord is
    // at least drawn rather than dropped.
    let value = PLAIN
        .iter()
        .find(|(base, _)| ticks >= *base)
        .map_or(NoteValue::ThirtySecond, |(_, v)| *v);
    NotationDuration::new(value)
}

/// A tick span as a `measure.beat.subdivision` duration.
fn duration_for(ticks: u32, denominator: u8) -> MusicalDuration {
    let beat = beat_ticks(denominator);
    let beats = (ticks / beat) as i32;
    let subdivision = ((ticks % beat) * 1000 / beat).min(999) as i32;
    MusicalDuration::new(0, beats, subdivision)
}

/// The root of a chord symbol, for [`RootNotation::from_string`].
fn root_of(symbol: &str) -> &str {
    let bytes = symbol.as_bytes();
    let mut len = 0;
    if bytes.first().is_some_and(|b| b.is_ascii_alphabetic()) {
        len = 1;
        if matches!(bytes.get(1), Some(b'#' | b'b')) {
            len = 2;
        }
    }
    &symbol[..len]
}

/// A section title, which chordsheet.com users often overload with more than
/// the section name.
///
/// `PART 2 | 6/4 | SYD'S THEME | 137 BPM | MEASURES 68-124` is a real title
/// from the corpus. The first `|`-separated field names the section; the rest
/// are read for a time signature and a tempo and otherwise kept as the
/// section comment, so nothing a player wrote is thrown away.
struct SectionHeader {
    section_type: SectionType,
    number: Option<u32>,
    comment: Option<String>,
    time_signature: Option<(u8, u8)>,
    tempo: Option<u32>,
}

impl SectionHeader {
    fn parse(title: &str) -> Self {
        let mut fields = title.split('|').map(str::trim).filter(|f| !f.is_empty());
        let name = fields.next().unwrap_or("").trim();
        let rest: Vec<&str> = fields.collect();

        let mut time_signature = None;
        let mut tempo = None;
        let mut comment_parts: Vec<&str> = Vec::new();
        for field in &rest {
            if let Some(ts) = parse_time_signature_field(field) {
                time_signature = Some(ts);
            } else if let Some(bpm) = parse_bpm_field(field) {
                tempo = Some(bpm);
            } else {
                comment_parts.push(field);
            }
        }

        let (base, number, extra) = split_name(name);
        if let Some(extra) = extra {
            comment_parts.insert(0, extra);
        }

        let section_type = SectionType::parse(base)
            .unwrap_or_else(|_| SectionType::Custom(custom_section_name(base)));

        Self {
            section_type,
            number,
            comment: (!comment_parts.is_empty()).then(|| comment_parts.join(" · ")),
            time_signature,
            tempo,
        }
    }
}

/// `VERSE 1` → (`VERSE`, 1, None); `Chorus [DOWN]` → (`Chorus`, None, `[DOWN]`).
fn split_name(name: &str) -> (&str, Option<u32>, Option<&str>) {
    let (name, extra) = match name.find(['[', '(']) {
        Some(idx) => (name[..idx].trim(), Some(name[idx..].trim())),
        None => (name, None),
    };
    let trailing_digits = name.len() - name.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    if trailing_digits == 0 {
        return (name, None, extra);
    }
    let (base, digits) = name.split_at(name.len() - trailing_digits);
    let base_trimmed = base.trim_end();
    // Only a *separated* number is a section number: `VERSE 2` yes, `EVE6` no.
    if base_trimmed.len() == base.len() {
        return (name, None, extra);
    }
    (base_trimmed, digits.parse().ok(), extra)
}

fn parse_time_signature_field(field: &str) -> Option<(u8, u8)> {
    let (n, d) = field.split_once('/')?;
    let n: u8 = n.trim().parse().ok()?;
    let d: u8 = d.trim().parse().ok()?;
    (n > 0 && d > 0).then_some((n, d))
}

fn parse_bpm_field(field: &str) -> Option<u32> {
    let lower = field.to_ascii_lowercase();
    let digits = lower.strip_suffix("bpm")?.trim();
    digits.parse().ok()
}

/// A custom section name keyflow can write back out.
///
/// keyflow spells a custom section `[Name] 8`, and its parser stops at a
/// comma — `[Down Chorus, Skip The First] 8` reads as no header at all, so
/// every bar under it joins the section above and the chart quietly grows a
/// section. Punctuation the bracket form can't carry is dropped here, where
/// the name is minted; the title exactly as the author typed it stays on
/// `Section::name`.
fn custom_section_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '\'' | '-' | '&' | '/' | '.') {
                c
            } else {
                ' '
            }
        })
        .collect();
    title_case(&cleaned)
}

fn title_case(s: &str) -> String {
    // Chordsheet titles are usually shouted (`=CHORUS`). Charts read better
    // with them cased, and the verbatim title is kept on `Section::name`.
    let mut out = String::with_capacity(s.len());
    for (i, word) in s.split_whitespace().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(&chars.as_str().to_lowercase());
        }
    }
    if out.is_empty() {
        "Section".to_string()
    } else {
        out
    }
}
