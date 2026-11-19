//! Actions on what is selected: rough the chart in by text, then select on
//! the engraved chart (or by script) and run an action on the selection.
//!
//! A selection is a list of [`Pick`]s: bars, a place (a bar and a beat), a
//! piece of the chart's text (what a clicked chord symbol hands over), or
//! something that is not music an action can move (a section label, a
//! barline), which an action refuses by name. [`apply`] resolves the picks
//! to the chord tokens they name and rewrites those tokens in the chart's
//! text, each in place and nothing else: the person's line breaks, spacing
//! and comments stay as they were, and what is written is the language's
//! own syntax, never a second representation.
//!
//! # Push and Pull
//!
//! A push is an anticipation: the chord arrives early and is tied over
//! into the beat it is written on. The language writes it before the
//! chord: `'G` an eighth early, `''G` a sixteenth, `'''G` a thirty-second
//! (`'tG` a triplet eighth, `'_4G` a quarter). A pull is the opposite,
//! written after it: `G'` an eighth late, `G''` a sixteenth.
//!
//! [`Action::Push`] with no step adds one apostrophe on that ladder and
//! [`Action::Pull`] takes one off, through zero into the pulls:
//!
//! ```text
//! G''  ←  G'  ←  G  →  'G  →  ''G  →  '''G
//!         pull       push
//! ```
//!
//! so a push and then a pull give back the text there was. Note what the
//! apostrophes mean: the second one makes an eighth's push a sixteenth's,
//! which is *nearer* the beat, not further from it. With a [`Step`] the
//! action writes that step's spelling outright (`Push(Sixteenth)` is
//! `''G` whatever was there), and the opposite action with the same step
//! takes it off again. A step the syntax has no spelling for is refused.
//!
//! What the bar's slashes do: nothing. A push does not change what is
//! written on the beat (`C // 'G / F /` is still four beats); the engraver
//! draws the early arrival and its tie, over a barline, a line or a page
//! too. So the bar always still adds up.

use std::collections::BTreeSet;

use keyflow_proto::chart::Measure;

use super::chords;
use super::input::{bars, replays};
use crate::text::chart::{Chart, parse_chart};

/// One thing selected.
#[derive(Debug, Clone, PartialEq)]
pub enum Pick {
    /// Every chord written in bars `from..=to` (the chart's bars from 0,
    /// as the engraver numbers them: [`bars`]).
    Bars { from: usize, to: usize },
    /// The chord at `beat` of `bar` (beats from 1, the meter's): the one
    /// written there, else the one sounding there.
    At { bar: usize, beat: f64 },
    /// The chord whose text is at bytes `start..end` of the chart's
    /// source: what a chord symbol clicked on the chart hands over.
    Span { start: usize, end: usize },
    /// Something an action on music cannot change, by its kind's name
    /// (`section`, `key`, `time`, `clef`, `measure-number`…).
    Other(String),
}

/// How far a push or a pull moves a chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Eighth,
    Sixteenth,
    ThirtySecond,
    TripletEighth,
    Quarter,
}

impl Step {
    /// A step by its name (`eighth`, `8`, `sixteenth`, `16`, `32`,
    /// `triplet`, `quarter`, `4`).
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Some(match name.trim().to_ascii_lowercase().as_str() {
            "eighth" | "8" | "8th" => Self::Eighth,
            "sixteenth" | "16" | "16th" => Self::Sixteenth,
            "thirty-second" | "thirtysecond" | "32" | "32nd" => Self::ThirtySecond,
            "triplet" | "triplet-eighth" | "8t" => Self::TripletEighth,
            "quarter" | "4" => Self::Quarter,
            _ => return None,
        })
    }

    fn shift(self) -> Shift {
        match self {
            Self::Eighth => Shift::Count(1),
            Self::Sixteenth => Shift::Count(2),
            Self::ThirtySecond => Shift::Count(3),
            Self::TripletEighth => Shift::Triplet,
            Self::Quarter => Shift::Quarter,
        }
    }
}

/// What to do to the chords selected.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Earlier: one apostrophe more (`G` → `'G` → `''G`), or `step`'s
    /// spelling.
    Push(Option<Step>),
    /// Later: one apostrophe fewer, through none into the pulls (`'G` →
    /// `G` → `G'`), or `step`'s spelling.
    Pull(Option<Step>),
    /// An accent put on (`>G`), or taken off when every chord has one.
    Accent,
    /// A staccato put on (`.G`), or taken off when every chord has one.
    Staccato,
    /// Every root (and bass) moved by so many semitones: up spelled with
    /// sharps, down with flats.
    Transpose(i8),
    /// Each chord's symbol made this one, its length and marks kept.
    Retype(String),
}

impl Action {
    /// Its undo step's name.
    #[must_use]
    pub fn name(&self) -> String {
        match self {
            Self::Push(_) => "Push".to_owned(),
            Self::Pull(_) => "Pull".to_owned(),
            Self::Accent => "Accent".to_owned(),
            Self::Staccato => "Staccato".to_owned(),
            Self::Transpose(n) => format!("Transpose {n:+}"),
            Self::Retype(t) => format!("Chord {t}"),
        }
    }
}

/// One chord as the chart's text writes it.
#[derive(Debug, Clone, PartialEq)]
pub struct ChordRef {
    /// Its bar ([`bars`], from 0).
    pub bar: usize,
    /// Its place among the bar's chords, from 0.
    pub index: usize,
    /// The beat it is written on, from 1 (the meter's beats); `None` in a
    /// bar whose chords cannot be told apart by beat.
    pub beat: Option<f64>,
    /// Its token, as written (`'F#m7/A_4`).
    pub token: String,
    /// Where its token is in the source, in bytes.
    pub start: usize,
    pub end: usize,
}

/// One chord an action rewrote.
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub bar: usize,
    pub was: String,
    pub now: String,
}

/// What an action did.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    /// The chart's text after it.
    pub text: String,
    pub changed: Vec<Change>,
    /// What it left alone, and why, in words for a status line.
    pub skipped: Vec<String>,
}

/// Every chord `source` writes, in the chart's order. A bar that plays
/// another again (`%`, a recalled section) writes none of its own: its
/// chords are the bar's it plays.
#[must_use]
pub fn chords_of(source: &str, chart: &Chart) -> Vec<ChordRef> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for (bar, measure) in bars(chart).into_iter().enumerate() {
        let beats = beats_of(measure);
        for (index, chord) in measure.chords.iter().enumerate() {
            let Some(span) = chord.source_span else {
                continue;
            };
            let (start, end) = (span.start, span.start + span.len);
            let Some(token) = source.get(start..end) else {
                continue;
            };
            if token.trim().is_empty() || !seen.insert(start) {
                continue;
            }
            out.push(ChordRef {
                bar,
                index,
                beat: beats.as_ref().and_then(|b| b.get(index).copied()),
                token: token.to_owned(),
                start,
                end,
            });
        }
    }
    out
}

/// The beat each of `measure`'s chords is written on, from 1: where it
/// sounds, put back by its push (it sounds early) or its pull (late),
/// whatever value that is by. `None` when they do not come out one after
/// another.
fn beats_of(measure: &Measure) -> Option<Vec<f64>> {
    let time = measure.time_signature;
    let whole = i64::from(super::music::TICKS_PER_WHOLE);
    let length = i64::from(super::music::bar_ticks(time)).max(1);
    let beat = whole as f64 / f64::from(time.1.max(1));
    let mut beats = Vec::with_capacity(measure.chords.len());
    for c in &measure.chords {
        let sounds = i64::from(chords::ticks_of(&c.position.total_duration, time));
        let shift = c.push_pull.map_or(0, |(push, amount)| {
            let ticks = (amount.to_beats() * whole as f64 / 4.0).round() as i64;
            if push { ticks } else { -ticks }
        });
        let written = (sounds + shift).rem_euclid(length);
        beats.push(1.0 + written as f64 / beat);
    }
    beats.windows(2).all(|w| w[1] > w[0]).then_some(beats)
}

/// The chords `picks` name, in the chart's order, each once; and what was
/// picked that names none, in words.
#[must_use]
pub fn resolve(source: &str, chart: &Chart, picks: &[Pick]) -> (Vec<ChordRef>, Vec<String>) {
    let all = chords_of(source, chart);
    let plays = replays(chart);
    let count = bars(chart).len();
    let mut chosen = BTreeSet::new();
    let mut skipped = Vec::new();
    // A bar that plays another again: the bar it plays is what is written.
    let measures = bars(chart);
    let written = |bar: usize| {
        let mut at = plays.get(bar).copied().flatten().unwrap_or(bar);
        // A line's copy of itself (`x2`), then a `%`: back to the bar
        // whose text it is.
        for _ in 0..measures.len() {
            let Some(m) = measures.get(at) else { break };
            match m.plays_back {
                Some(n) if n > 0 && n <= at => at -= n,
                _ if m.simile && at > 0 => at -= 1,
                _ => break,
            }
        }
        at
    };
    for pick in picks {
        match pick {
            Pick::Bars { from, to } => {
                if *from >= count {
                    skipped.push(format!("There is no bar {}", from + 1));
                    continue;
                }
                let mut any = false;
                for bar in *from..=(*to).min(count - 1) {
                    let at = written(bar);
                    for (i, c) in all.iter().enumerate() {
                        if c.bar == at {
                            chosen.insert(i);
                            any = true;
                        }
                    }
                }
                if !any {
                    skipped.push(if from == to {
                        format!("Bar {} has no chord of its own", from + 1)
                    } else {
                        format!("Bars {} to {} have no chord", from + 1, to + 1)
                    });
                }
            }
            Pick::At { bar, beat } => {
                let at = written(*bar);
                let in_bar: Vec<usize> = (0..all.len()).filter(|&i| all[i].bar == at).collect();
                // The chord written on the beat, else the last one before it
                // (still sounding), else the bar's first.
                let found = in_bar
                    .iter()
                    .copied()
                    .filter(|&i| all[i].beat.is_some_and(|b| b <= *beat + 1e-6))
                    .next_back()
                    .or_else(|| in_bar.first().copied());
                match found {
                    Some(i) => {
                        chosen.insert(i);
                    }
                    None => skipped.push(format!("Bar {} has no chord at beat {beat}", bar + 1)),
                }
            }
            Pick::Span { start, end } => {
                let found: Vec<usize> = (0..all.len())
                    .filter(|&i| all[i].start < *end && *start < all[i].end)
                    .collect();
                if found.is_empty() {
                    skipped.push("That is not a chord".to_owned());
                }
                chosen.extend(found);
            }
            Pick::Other(kind) => skipped.push(not_music(kind)),
        }
    }
    (
        chosen.into_iter().map(|i| all[i].clone()).collect(),
        skipped,
    )
}

/// Why an action does nothing to a selected `kind` of element.
fn not_music(kind: &str) -> String {
    match kind {
        "section" => "A section label is not in time: it names its bars. Select its bars instead",
        "key" | "time" | "clef" => "A key, a meter or a clef holds from where it is written: there is nothing to move",
        "measure-number" | "volta" => "A bar number or an ending bracket is the bar's: select the bar",
        "tie" => "A tie joins two notes: select the chord it ties",
        "cue" | "text" | "dynamic" | "hairpin" | "lyrics" => "A cue, a dynamic or a lyric is placed by its bar and beat, not pushed: this action moves chords",
        "simile" => "A repeat-bar sign plays the bar before it: select that bar",
        _ => "Nothing there for this action",
    }
    .to_owned()
}

/// `action` done to what `picks` select in `source`. `Err` when nothing
/// was changed, with why (the first refusal); else the text, what changed
/// and what was left alone.
///
/// # Errors
///
/// The source does not parse, nothing selected is a chord, or the action
/// was refused for every chord selected.
pub fn apply(source: &str, picks: &[Pick], action: &Action) -> Result<Applied, String> {
    let chart = parse_chart(source)?;
    let (chosen, mut skipped) = resolve(source, &chart, picks);
    if chosen.is_empty() {
        return Err(skipped
            .into_iter()
            .next()
            .unwrap_or_else(|| "Select a chord, a bar or bars first".to_owned()));
    }
    // A toggle is one decision for the whole selection: off only when
    // every chord has the mark, as a palette's cell on a range.
    let all_marked = |mark: char| {
        chosen
            .iter()
            .all(|c| Shape::of(&c.token).is_ok_and(|s| s.lead.contains(mark)))
    };
    let take_off = match action {
        Action::Accent => all_marked('>'),
        Action::Staccato => all_marked('.'),
        _ => false,
    };
    let measures = bars(&chart);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut changed = Vec::new();
    for c in &chosen {
        let now = match rewrite(c, action, take_off, &measures) {
            Ok(now) => now,
            Err(why) => {
                skipped.push(format!("Bar {}, {}: {why}", c.bar + 1, c.token));
                continue;
            }
        };
        if now != c.token {
            edits.push((c.start, c.end, now.clone()));
            changed.push(Change {
                bar: c.bar,
                was: c.token.clone(),
                now,
            });
        }
    }
    if edits.is_empty() {
        return Err(skipped
            .into_iter()
            .next()
            .unwrap_or_else(|| "Nothing to change".to_owned()));
    }
    let mut text = source.to_owned();
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    for (start, end, now) in edits {
        text.replace_range(start..end, &now);
    }
    // What is written must still be the chart it was: the same bars.
    let after = parse_chart(&text).map_err(|e| format!("Not written: {e}"))?;
    if bars(&after).len() != measures.len() {
        return Err("Not written: it would change how many bars the chart has".to_owned());
    }
    Ok(Applied {
        text,
        changed,
        skipped,
    })
}

/// `c`'s token after `action`.
fn rewrite(
    c: &ChordRef,
    action: &Action,
    take_off: bool,
    measures: &[&Measure],
) -> Result<String, String> {
    let mut shape = Shape::of(&c.token)?;
    match action {
        Action::Push(step) | Action::Pull(step) => {
            let push = matches!(action, Action::Push(_));
            shape.shift(push, *step)?;
            if shape.push != Shift::None {
                arrives_early(c, &shape, measures)?;
            }
        }
        Action::Accent => shape.set_mark('>', !take_off),
        Action::Staccato => shape.set_mark('.', !take_off),
        Action::Transpose(by) => {
            let mut core = shape.core.clone();
            for _ in 0..by.unsigned_abs() {
                core = transposed(&core, *by > 0)
                    .ok_or("its root is a degree: a number chart transposes by its key")?;
            }
            shape.core = core;
        }
        Action::Retype(typed) => {
            let typed = typed.trim();
            if typed.is_empty() || chords::parts(typed).is_none() {
                return Err(format!("{typed:?} is not a chord symbol"));
            }
            let new = Shape::of(typed)?;
            // Marks typed replace the chord's; none typed keep them.
            if !new.lead.is_empty() || new.push != Shift::None || new.pull != Shift::None {
                shape.lead = new.lead;
                shape.push = new.push;
                shape.pull = new.pull;
            }
            shape.core = new.core;
            if !new.tail.is_empty() {
                shape.tail = new.tail;
            }
        }
    }
    Ok(shape.text())
}

/// Whether a pushed `c` has somewhere to arrive: not before the chart's
/// first bar, and not on or before the chord it follows in its bar.
fn arrives_early(c: &ChordRef, shape: &Shape, measures: &[&Measure]) -> Result<(), String> {
    let first_in_bar = c.index == 0 && c.beat.is_none_or(|b| b <= 1.0 + 1e-6);
    if first_in_bar {
        if c.bar == 0 {
            return Err(
                "it is the chart's first chord: there is no bar before bar 1 to arrive in (write a pickup bar first)"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    // Inside the bar: the push must be shorter than the room the chord
    // before it has, or the two would sound together.
    let (Some(beat), Some(measure)) = (c.beat, measures.get(c.bar)) else {
        return Ok(());
    };
    let den = f64::from(measure.time_signature.1.max(1));
    // The step in the meter's beats (a quarter is `den / 4` of them).
    let early = shape.push.quarters() * den / 4.0;
    let before =
        beats_of(measure).and_then(|b| c.index.checked_sub(1).and_then(|i| b.get(i).copied()));
    if let Some(prev) = before
        && beat - early <= prev + 1e-6
    {
        return Err(format!(
            "pushed that far it lands on the chord before it (beat {prev}): that beat is taken"
        ));
    }
    Ok(())
}

/// A chord's core (`F#m7/A`) with its root and bass a semitone up or down.
fn transposed(core: &str, up: bool) -> Option<String> {
    let (root, bass) = core
        .rfind('/')
        .filter(|&i| core[i + 1..].starts_with(|c: char| ('A'..='G').contains(&c)))
        .map_or((core, None), |i| (&core[..i], Some(&core[i + 1..])));
    let root = chords::transpose(root, up)?;
    let root = respell(&root, up);
    Some(match bass {
        Some(bass) => format!("{root}/{}", respell(&chords::transpose(bass, up)?, up)),
        None => root,
    })
}

/// A root spelled the way the direction asks: flats going down.
fn respell(token: &str, up: bool) -> String {
    if up {
        return token.to_owned();
    }
    const SHARPS: [(&str, &str); 5] = [
        ("C#", "Db"),
        ("D#", "Eb"),
        ("F#", "Gb"),
        ("G#", "Ab"),
        ("A#", "Bb"),
    ];
    for (sharp, flat) in SHARPS {
        if let Some(rest) = token.strip_prefix(sharp) {
            return format!("{flat}{rest}");
        }
    }
    token.to_owned()
}

/// How a chord is pushed or pulled, as its token writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shift {
    None,
    /// One, two or three apostrophes: an eighth, a sixteenth, a
    /// thirty-second.
    Count(u8),
    /// `'t`: a triplet eighth.
    Triplet,
    /// `'_4`: a quarter.
    Quarter,
}

impl Shift {
    /// Its length in quarter notes.
    fn quarters(self) -> f64 {
        match self {
            Self::None => 0.0,
            Self::Count(n) => 1.0 / f64::from(1_u32 << n.min(3)),
            Self::Triplet => 1.0 / 3.0,
            Self::Quarter => 1.0,
        }
    }
}

/// The most apostrophes the language reads: a thirty-second.
const MAX_COUNT: u8 = 3;

/// A chord token taken apart: the marks before it (`!`, `>`, `.`), its
/// push, the chord itself, its pull, and its written length (`_4.`).
#[derive(Debug, Clone, PartialEq)]
struct Shape {
    lead: String,
    push: Shift,
    core: String,
    pull: Shift,
    tail: String,
}

impl Shape {
    fn of(token: &str) -> Result<Self, String> {
        let mut lead = String::new();
        let mut count = 0_u8;
        let mut rest = token;
        while let Some(c) = rest.chars().next() {
            match c {
                '\'' => count += 1,
                '!' | '>' | '.' => lead.push(c),
                _ => break,
            }
            rest = &rest[c.len_utf8()..];
        }
        let mut push = match count {
            0 => Shift::None,
            n if n <= MAX_COUNT => Shift::Count(n),
            _ => return Err("it has more apostrophes than the language reads".to_owned()),
        };
        if count > 0 {
            if let Some(after) = rest.strip_prefix('t') {
                push = Shift::Triplet;
                rest = after;
            } else if let Some(after) = rest.strip_prefix("_4") {
                push = Shift::Quarter;
                rest = after;
            } else if rest.starts_with('_') || rest.starts_with(':') {
                return Err(
                    "it is pushed by a value this action does not write: retype it".to_owned(),
                );
            }
        }
        let (body, tail) = rest.find('_').map_or((rest, ""), |i| rest.split_at(i));
        let core = body.trim_end_matches('\'');
        let pulls = body.len() - core.len();
        let (pull, tail) = match (pulls, tail) {
            (0, tail) => (Shift::None, tail),
            (n, "") if n <= usize::from(MAX_COUNT) => (Shift::Count(n as u8), ""),
            (1, "_4") => (Shift::Quarter, ""),
            _ => {
                return Err(
                    "it is pulled by a value this action does not write: retype it".to_owned(),
                );
            }
        };
        if core.is_empty() {
            return Err("it has no chord".to_owned());
        }
        Ok(Self {
            lead,
            push,
            core: core.to_owned(),
            pull,
            tail: tail.to_owned(),
        })
    }

    fn text(&self) -> String {
        let push = match self.push {
            Shift::None => String::new(),
            Shift::Count(n) => "'".repeat(usize::from(n)),
            Shift::Triplet => "'t".to_owned(),
            Shift::Quarter => "'_4".to_owned(),
        };
        let pull = match self.pull {
            Shift::None => String::new(),
            Shift::Count(n) => "'".repeat(usize::from(n)),
            Shift::Triplet => String::new(),
            Shift::Quarter => "'_4".to_owned(),
        };
        format!("{}{push}{}{pull}{}", self.lead, self.core, self.tail)
    }

    fn set_mark(&mut self, mark: char, on: bool) {
        self.lead.retain(|c| c != mark);
        if on {
            // An accent before a staccato (`>.G`), both after a `!`.
            let at = match mark {
                '>' => self.lead.find('.').unwrap_or(self.lead.len()),
                _ => self.lead.len(),
            };
            self.lead.insert(at, mark);
        }
    }

    /// Pushed (`push`) or pulled a step: one apostrophe along the ladder,
    /// or `step`'s spelling.
    fn shift(&mut self, push: bool, step: Option<Step>) -> Result<(), String> {
        let Some(step) = step else {
            // The ladder: pulls below zero, pushes above.
            let level = match (self.push, self.pull) {
                (Shift::None, Shift::None) => 0_i8,
                (Shift::Count(n), Shift::None) => n as i8,
                (Shift::None, Shift::Count(n)) => -(n as i8),
                _ => {
                    return Err(
                        "it is pushed or pulled by a value with no apostrophe to add or take off: name a step"
                            .to_owned(),
                    );
                }
            };
            let next = if push { level + 1 } else { level - 1 };
            if next > MAX_COUNT as i8 {
                return Err(
                    "three apostrophes (a thirty-second) is the most the language writes"
                        .to_owned(),
                );
            }
            if next < -(MAX_COUNT as i8) {
                return Err(
                    "three apostrophes (a thirty-second late) is the most the language writes"
                        .to_owned(),
                );
            }
            return self.set(next);
        };
        let wanted = step.shift();
        // The opposite action with the same step takes it off.
        let (mine, other) = if push {
            (&mut self.push, &mut self.pull)
        } else {
            (&mut self.pull, &mut self.push)
        };
        if *other == wanted {
            *other = Shift::None;
            return Ok(());
        }
        if !push && wanted == Shift::Triplet {
            return Err(
                "the language writes a triplet pull after the chord's letter (`Ct'`), which this action does not produce: type it"
                    .to_owned(),
            );
        }
        if !push && !self.tail.is_empty() {
            return Err(late_with_length());
        }
        *other = Shift::None;
        *mine = wanted;
        Ok(())
    }

    fn set(&mut self, level: i8) -> Result<(), String> {
        if level < 0 && !self.tail.is_empty() {
            return Err(late_with_length());
        }
        self.push = if level > 0 {
            Shift::Count(level.unsigned_abs())
        } else {
            Shift::None
        };
        self.pull = if level < 0 {
            Shift::Count(level.unsigned_abs())
        } else {
            Shift::None
        };
        Ok(())
    }
}

/// Why a chord written with a length is not pulled.
fn late_with_length() -> String {
    "it is written with a length (`_4`), and the language reads an apostrophe before that as the pull's own value (`G'_4` is pulled a quarter): write the late arrival as rhythm instead"
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hand-written chart: comments, uneven spacing, a recall.
    const SONG: &str = "Push - Test\n#C 120bpm 4/4\n\nVS 4\n; the verse groove\nC // G //\nAm   //   F //\nC_2 G_4 F_4\nDm ////\n\nCH 4\nF G Am G\n";

    fn at(bar: usize, beat: f64) -> Vec<Pick> {
        vec![Pick::At { bar, beat }]
    }

    fn run(source: &str, picks: &[Pick], action: Action) -> String {
        apply(source, picks, &action).expect("applies").text
    }

    /// The lines of `after` that differ from `before`'s, by number.
    fn lines_changed(before: &str, after: &str) -> Vec<usize> {
        assert_eq!(before.lines().count(), after.lines().count());
        before
            .lines()
            .zip(after.lines())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i + 1)
            .collect()
    }

    #[test]
    fn push_adds_an_apostrophe_and_pull_takes_it_off() {
        let once = run(SONG, &at(0, 3.0), Action::Push(None));
        assert!(once.contains("C // 'G //"), "{once}");
        let twice = run(&once, &at(0, 3.0), Action::Push(None));
        assert!(twice.contains("C // ''G //"), "{twice}");
        let thrice = run(&twice, &at(0, 3.0), Action::Push(None));
        assert!(thrice.contains("C // '''G //"));
        let why = apply(&thrice, &at(0, 3.0), &Action::Push(None)).unwrap_err();
        assert!(why.contains("thirty-second"), "{why}");
        // Back down the same way, to the text there was.
        let back = run(&twice, &at(0, 3.0), Action::Pull(None));
        assert_eq!(back, once);
        assert_eq!(run(&once, &at(0, 3.0), Action::Pull(None)), SONG);
    }

    #[test]
    fn what_the_apostrophes_mean_is_the_parsers() {
        use keyflow_proto::chord::PushPullBase;
        for (text, level) in [("'G", 1), ("''G", 2), ("'''G", 3)] {
            let src = SONG.replace("C // G //", &format!("C // {text} //"));
            let chart = parse_chart(&src).unwrap();
            let chord = &bars(&chart)[0].chords[1];
            let (push, amount) = chord.push_pull.expect("pushed");
            assert!(push);
            assert_eq!(amount.level, level);
            assert!(matches!(amount.base, PushPullBase::Standard));
            // An eighth, a sixteenth, a thirty-second, in beats.
            assert!((amount.to_beats() - 1.0 / f64::from(1 << level)).abs() < 1e-9);
        }
    }

    #[test]
    fn pull_past_zero_is_the_languages_late_arrival() {
        let late = run(SONG, &at(0, 3.0), Action::Pull(None));
        assert!(late.contains("C // G' //"), "{late}");
        let chart = parse_chart(&late).unwrap();
        let (push, amount) = bars(&chart)[0].chords[1].push_pull.expect("pulled");
        assert!(!push, "a pull, not a push");
        assert_eq!(amount.level, 1);
        let later = run(&late, &at(0, 3.0), Action::Pull(None));
        assert!(later.contains("C // G'' //"));
        assert_eq!(run(&late, &at(0, 3.0), Action::Push(None)), SONG);
    }

    #[test]
    fn a_step_writes_its_own_spelling_and_the_opposite_takes_it_off() {
        for (step, text) in [
            (Step::Eighth, "'G"),
            (Step::Sixteenth, "''G"),
            (Step::ThirtySecond, "'''G"),
            (Step::TripletEighth, "'tG"),
            (Step::Quarter, "'_4G"),
        ] {
            let pushed = run(SONG, &at(0, 3.0), Action::Push(Some(step)));
            assert!(pushed.contains(&format!("C // {text} //")), "{pushed}");
            let chart = parse_chart(&pushed).unwrap();
            let chord = &bars(&chart)[0].chords[1];
            assert!(chord.push_pull.is_some_and(|p| p.0), "{text} parses pushed");
            assert_eq!(chord.full_symbol, "G", "{text}");
            assert_eq!(run(&pushed, &at(0, 3.0), Action::Pull(Some(step))), SONG);
        }
        let pulled = run(SONG, &at(0, 3.0), Action::Pull(Some(Step::Sixteenth)));
        assert!(pulled.contains("C // G'' //"));
        let why = apply(SONG, &at(0, 3.0), &Action::Pull(Some(Step::TripletEighth))).unwrap_err();
        assert!(why.contains("triplet"), "{why}");
        assert_eq!(Step::named("16"), Some(Step::Sixteenth));
        assert_eq!(Step::named("half"), None, "no spelling: refused by name");
    }

    #[test]
    fn a_bar_or_a_range_pushes_every_chord_change_in_it() {
        // Bar 2: both chords; its first arrives in bar 1.
        let bar = run(SONG, &[Pick::Bars { from: 1, to: 1 }], Action::Push(None));
        assert!(bar.contains("'Am   //   'F //"), "spacing kept: {bar}");
        assert_eq!(lines_changed(SONG, &bar), [7]);
        // Bars 2 to 4: every chord but none of bar 1's.
        let out = apply(SONG, &[Pick::Bars { from: 1, to: 3 }], &Action::Push(None)).unwrap();
        assert_eq!(out.changed.len(), 6, "{:?}", out.changed);
        assert_eq!(lines_changed(SONG, &out.text), [7, 8, 9]);
        assert!(out.text.contains("'C_2 'G_4 'F_4"));
        assert!(out.text.contains("; the verse groove"), "the comment stays");
        // And back.
        let back = run(
            &out.text,
            &[Pick::Bars { from: 1, to: 3 }],
            Action::Pull(None),
        );
        assert_eq!(back, SONG);
    }

    #[test]
    fn the_first_chord_has_no_bar_to_arrive_in() {
        let why = apply(SONG, &at(0, 1.0), &Action::Push(None)).unwrap_err();
        assert!(why.contains("no bar before bar 1"), "{why}");
        // A range that starts at bar 1 pushes the rest and says so.
        let out = apply(SONG, &[Pick::Bars { from: 0, to: 0 }], &Action::Push(None)).unwrap();
        assert!(out.text.contains("C // 'G //"));
        assert_eq!(out.skipped.len(), 1);
        // Pulled, it has: later is inside its own bar.
        assert!(run(SONG, &at(0, 1.0), Action::Pull(None)).contains("C' // G //"));
    }

    #[test]
    fn a_push_over_a_barline_is_written_on_the_chord_and_still_parses() {
        let out = run(SONG, &at(3, 1.0), Action::Push(None));
        assert!(out.contains("'Dm ////"), "{out}");
        let chart = parse_chart(&out).unwrap();
        assert_eq!(bars(&chart).len(), 8, "the same bars");
        // Into the last bar of a section from the next one's first.
        let over = run(SONG, &at(4, 1.0), Action::Push(None));
        assert!(over.contains("CH 4\n'F G Am G"), "{over}");
    }

    #[test]
    fn a_push_onto_the_chord_before_it_is_refused() {
        // G_4 then F_4 a beat later: a quarter's push lands on the G.
        let why = apply(SONG, &at(2, 4.0), &Action::Push(Some(Step::Quarter))).unwrap_err();
        assert!(why.contains("beat is taken"), "{why}");
        // An eighth's has room.
        assert!(run(SONG, &at(2, 4.0), Action::Push(None)).contains("C_2 G_4 'F_4"));
    }

    #[test]
    fn a_chord_with_a_length_is_not_pulled() {
        let why = apply(SONG, &at(2, 3.0), &Action::Pull(None)).unwrap_err();
        assert!(why.contains("pulled a quarter"), "{why}");
    }

    #[test]
    fn what_is_not_music_is_refused_by_name() {
        for (kind, word) in [
            ("section", "section label"),
            ("time", "meter"),
            ("measure-number", "bar number"),
            ("dynamic", "placed by its bar and beat"),
        ] {
            let why =
                apply(SONG, &[Pick::Other(kind.to_owned())], &Action::Push(None)).unwrap_err();
            assert!(why.contains(word), "{kind}: {why}");
        }
        // With a chord beside it, the chord is pushed and the rest named.
        let picks = [
            Pick::Other("section".to_owned()),
            Pick::At { bar: 0, beat: 3.0 },
        ];
        let out = apply(SONG, &picks, &Action::Push(None)).unwrap();
        assert_eq!(out.changed.len(), 1);
        assert_eq!(out.skipped.len(), 1);
    }

    #[test]
    fn a_clicked_symbols_text_is_the_chord() {
        let start = SONG.find("G //").unwrap();
        let picks = [Pick::Span {
            start,
            end: start + 1,
        }];
        assert!(run(SONG, &picks, Action::Push(None)).contains("C // 'G //"));
    }

    #[test]
    fn a_bar_that_plays_another_again_changes_the_bar_it_plays() {
        let src = "Again - Test\n#C 120bpm 4/4\n\nVS 4\nC // G //\n%\nF\nC\n";
        let out = apply(src, &[Pick::Bars { from: 1, to: 1 }], &Action::Push(None));
        // Bar 2 is `%`: what is written is bar 1, whose G is pushed (its C
        // is the chart's first chord).
        let out = out.unwrap();
        assert!(out.text.contains("C // 'G //\n%\n"), "{}", out.text);
        // Both bars selected: each chord once, not twice.
        let both = apply(src, &[Pick::Bars { from: 0, to: 1 }], &Action::Push(None)).unwrap();
        assert_eq!(both.text, out.text);
    }

    #[test]
    fn accents_and_staccatos_go_on_the_selection_and_off_when_all_have_one() {
        let bar = [Pick::Bars { from: 0, to: 0 }];
        let on = run(SONG, &bar, Action::Accent);
        assert!(on.contains(">C // >G //"), "{on}");
        // One of the two already accented: the other gets one too.
        let mixed = SONG.replace("C // G //", ">C // G //");
        assert_eq!(run(&mixed, &bar, Action::Accent), on);
        assert_eq!(run(&on, &bar, Action::Accent), SONG, "all on: off");
        let both = run(&on, &bar, Action::Staccato);
        assert!(both.contains(">.C // >.G //"), "{both}");
        // A pushed chord keeps its push under an accent.
        let pushed = run(SONG, &at(0, 3.0), Action::Push(None));
        assert!(run(&pushed, &at(0, 3.0), Action::Accent).contains("C // >'G //"));
    }

    #[test]
    fn transposing_moves_roots_and_basses_and_keeps_the_rest() {
        let src = "Keys - Test\n#C 120bpm 4/4\n\nVS 2\n>F#m7/A_2 'Bb_2\nCsus4 // G/B //\n";
        let all = [Pick::Bars { from: 0, to: 1 }];
        let up = run(src, &all, Action::Transpose(2));
        assert!(up.contains(">G#m7/B_2 'C_2\nDsus4 // A/C# //"), "{up}");
        let down = run(src, &all, Action::Transpose(-1));
        assert!(down.contains(">Fm7/Ab_2 'A_2\nBsus4 // Gb/Bb //"), "{down}");
        assert_eq!(lines_changed(src, &up), [5, 6]);
    }

    #[test]
    fn retyping_keeps_the_length_and_the_marks() {
        let out = run(SONG, &at(2, 3.0), Action::Retype("Gsus4".to_owned()));
        assert!(out.contains("C_2 Gsus4_4 F_4"), "{out}");
        let why = apply(SONG, &at(2, 3.0), &Action::Retype("??".to_owned())).unwrap_err();
        assert!(why.contains("not a chord symbol"));
    }

    #[test]
    fn every_chord_is_listed_with_its_bar_and_beat() {
        let chart = parse_chart(SONG).unwrap();
        let all = chords_of(SONG, &chart);
        let got: Vec<(usize, Option<f64>, &str)> = all
            .iter()
            .map(|c| (c.bar, c.beat, c.token.as_str()))
            .collect();
        assert_eq!(
            got[..8],
            [
                (0, Some(1.0), "C"),
                (0, Some(3.0), "G"),
                (1, Some(1.0), "Am"),
                (1, Some(3.0), "F"),
                (2, Some(1.0), "C_2"),
                (2, Some(3.0), "G_4"),
                (2, Some(4.0), "F_4"),
                (3, Some(1.0), "Dm"),
            ]
        );
        for c in &all {
            assert_eq!(&SONG[c.start..c.end], c.token);
        }
    }
}
