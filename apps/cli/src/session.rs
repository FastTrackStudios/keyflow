//! `kf session`: a song folder the Session app opens — a REAPER project whose
//! grid follows a recording bar by bar, beside a structure chart.
//!
//! The input is a JSON spec, so any source of a bar grid can feed it: a
//! reference tab's video sync today, our own beat tracking tomorrow.
//!
//! ```json
//! {
//!   "title": "Espresso", "artist": "Sabrina Carpenter",
//!   "audio": "Media/Espresso.wav", "audio_duration": 175.2,
//!   "bars": [{"start": 1.23, "quarters": 4.0, "num": 4, "den": 4}, …],
//!   "end": 171.9,
//!   "sections": [{"name": "Chorus 1", "bar": 4}, …],
//!   "lyrics": [{"text": "Now", "bar": 3, "beat": 3.0}, …],
//!   "tempo": 104.0
//! }
//! ```
//!
//! `bars[].start` are times in the audio file; `end` is when the last bar
//! ends. The project gets count-in bars so the song's first bar lands on a
//! barline, a tempo point per bar so REAPER's grid follows the recording,
//! `=START` / `SONGSTART` / `SONGEND` / `=END` markers, a region per section,
//! and a lyrics MIDI track placed by bar and beat (MIDI follows the tempo
//! map, so the words land on the recording).

use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Spec {
    pub title: String,
    #[serde(default)]
    pub artist: Option<String>,
    /// Audio path relative to the output folder (`Media/<song>.wav`).
    pub audio: String,
    pub audio_duration: f64,
    pub bars: Vec<Bar>,
    /// When the last bar ends (audio seconds).
    pub end: f64,
    #[serde(default)]
    pub sections: Vec<SectionMark>,
    #[serde(default)]
    pub lyrics: Vec<Lyric>,
    /// The written tempo, for the chart header.
    #[serde(default)]
    pub tempo: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct Bar {
    pub start: f64,
    /// Length in quarter notes (4/4 = 4, 6/8 = 3).
    pub quarters: f64,
    pub num: i32,
    pub den: i32,
}

#[derive(Debug, Deserialize)]
pub struct SectionMark {
    pub name: String,
    pub bar: usize,
}

#[derive(Debug, Deserialize)]
pub struct Lyric {
    pub text: String,
    pub bar: usize,
    /// Quarter notes into the bar.
    pub beat: f64,
}

/// Where everything sits on the project timeline.
struct Timeline {
    /// Project time of each bar start, plus the end.
    bar_times: Vec<f64>,
    /// Where the audio item starts.
    audio_pos: f64,
    count_in_bars: usize,
    first_bar_len: f64,
}

fn timeline(spec: &Spec) -> Result<Timeline, String> {
    let first = spec.bars.first().ok_or("spec has no bars")?;
    let t0 = first.start;
    let first_bar_len = spec
        .bars
        .get(1)
        .map_or(spec.end - t0, |b| b.start - t0)
        .max(0.1);
    // Count-in bars so the audio before the song fits and the song's first
    // bar lands on a barline; at least one, for a count-in.
    let count_in_bars = ((t0 / first_bar_len).ceil() as usize).max(1);
    let offset = count_in_bars as f64 * first_bar_len - t0;
    let mut bar_times: Vec<f64> = spec.bars.iter().map(|b| b.start + offset).collect();
    bar_times.push(spec.end + offset);
    Ok(Timeline {
        bar_times,
        audio_pos: offset,
        count_in_bars,
        first_bar_len,
    })
}

#[cfg(feature = "reaper")]
fn build_rpp(spec: &Spec, tl: &Timeline) -> String {
    use dawfile_reaper::RppSerialize;
    use dawfile_reaper::builder::ReaperProjectBuilder;

    const TPQN: u32 = 960;
    let first = &spec.bars[0];
    let bpm_of = |i: usize| -> f64 {
        let len = tl.bar_times[i + 1] - tl.bar_times[i];
        if len > 0.0 {
            // To a thousandth of a bpm: a fitted 174 reads as 174, not
            // 174.0000435, and the rounding moves a bar by under a µs.
            (spec.bars[i].quarters * 60.0 / len * 1000.0).round() / 1000.0
        } else {
            spec.tempo.unwrap_or(120.0)
        }
    };
    let first_bpm = (first.quarters * 60.0 / tl.first_bar_len * 1000.0).round() / 1000.0;
    let song_start = tl.bar_times[0];
    let song_end = *tl.bar_times.last().expect("end");
    let audio_end = tl.audio_pos + spec.audio_duration;

    let mut builder = ReaperProjectBuilder::new()
        .tempo_with_time_sig(first_bpm, first.num, first.den)
        .sample_rate(44100)
        .tempo_envelope(|mut env| {
            env = env.point_with_time_sig(0.0, first_bpm, first.num, first.den);
            // A point only where the tempo or meter changes: a song played to
            // a click is one tempo, and should read as one in REAPER.
            // A point is skipped only when the running tempo already lands
            // this bar within a millisecond, so the grid can never drift.
            let mut sig = (first.num, first.den);
            let (mut last_bpm, mut last_t, mut q_since) = (first_bpm, 0.0, 0.0);
            for (i, bar) in spec.bars.iter().enumerate() {
                let bpm = bpm_of(i);
                let predicted = last_t + q_since * 60.0 / last_bpm;
                if (bar.num, bar.den) != sig || i == 0 {
                    env = env.point_with_time_sig(tl.bar_times[i], bpm, bar.num, bar.den);
                    sig = (bar.num, bar.den);
                } else if (predicted - tl.bar_times[i]).abs() > 0.001
                    || (bpm - last_bpm).abs() > 0.05
                {
                    env = env.point(tl.bar_times[i], bpm);
                } else {
                    q_since += bar.quarters;
                    continue;
                }
                (last_bpm, last_t, q_since) = (bpm, tl.bar_times[i], bar.quarters);
            }
            env
        })
        .marker(1, 0.0, "=START")
        .marker(2, 0.0, "COUNT-IN")
        .marker(3, song_start, "SONGSTART")
        .marker(4, song_end, "SONGEND")
        .marker(5, audio_end.max(song_end), "=END");
    // A region per section, to the next section (or the song's end).
    for (k, s) in spec.sections.iter().enumerate() {
        let start_bar = s.bar.min(spec.bars.len());
        let end_bar = spec
            .sections
            .get(k + 1)
            .map_or(spec.bars.len(), |n| n.bar.min(spec.bars.len()));
        if end_bar <= start_bar {
            continue;
        }
        builder = builder.region(
            10 + k as i32,
            tl.bar_times[start_bar],
            tl.bar_times[end_bar],
            &s.name,
        );
    }
    let audio_len = spec.audio_duration;
    builder = builder.track("Reference", |t| {
        t.item(tl.audio_pos, audio_len, |i| {
            i.name("Reference").source_wave(&spec.audio)
        })
    });

    // Lyrics: a MIDI item from the song's first bar, one note per syllable
    // at its bar + beat (in quarters from the first bar).
    let mut bar_q = Vec::with_capacity(spec.bars.len() + 1);
    let mut q = 0.0;
    for b in &spec.bars {
        bar_q.push(q);
        q += b.quarters;
    }
    bar_q.push(q);
    let mut events: Vec<(u64, &str)> = spec
        .lyrics
        .iter()
        .filter(|l| l.bar < spec.bars.len())
        .map(|l| {
            (
                ((bar_q[l.bar] + l.beat) * f64::from(TPQN)).round() as u64,
                l.text.as_str(),
            )
        })
        .collect();
    events.sort_by_key(|e| e.0);
    if !events.is_empty() {
        let len = song_end - song_start;
        builder = builder.track("Lyrics", |t| {
            t.item(song_start, len, |i| {
                i.name("Lyrics").midi(|mut m| {
                    m = m.ticks_per_qn(TPQN);
                    let mut prev = 0u64;
                    for (k, (on, _)) in events.iter().enumerate() {
                        let next = events.get(k + 1).map_or(on + u64::from(TPQN), |e| e.0);
                        let dur = (next.saturating_sub(*on)).clamp(1, u64::from(TPQN));
                        m = m.note((on - prev) as u32, 0, 60, 80, dur as u32);
                        prev = *on;
                    }
                    m
                })
            })
        });
    }
    let mut project = builder.build();
    if !events.is_empty()
        && let Some(src) = project
            .tracks
            .last_mut()
            .and_then(|t| t.items.get_mut(0))
            .and_then(|i| i.takes.get_mut(0))
            .and_then(|tk| tk.source.as_mut())
            .and_then(|s| s.midi_data.as_mut())
    {
        let mut abs = crate::sync::absolute_events(src);
        for (on, text) in &events {
            abs.push((*on, 0, crate::sync::lyric_meta_event(text)));
        }
        crate::sync::store_delta_encoded(src, abs);
    }
    project.to_rpp_string()
}

/// The structure chart: header, then a section per marked section with its
/// bars as rhythm slashes (chords come later, from the PDF reader).
fn build_chart(spec: &Spec) -> String {
    use std::fmt::Write as _;
    let first = &spec.bars[0];
    let mut out = String::new();
    match &spec.artist {
        Some(a) if !a.is_empty() => {
            let _ = writeln!(out, "{} - {}", spec.title, a);
        }
        _ => {
            let _ = writeln!(out, "{}", spec.title);
        }
    }
    let tempo = spec
        .tempo
        .map(|t| format!("{}bpm ", t.round()))
        .unwrap_or_default();
    let _ = writeln!(out, "{tempo}{}/{}", first.num, first.den);
    // Beats a bar holds for slash notation (compound meters count dotted
    // quarters).
    let beats = |b: &Bar| -> usize {
        if b.den == 8 && b.num % 3 == 0 && b.num > 3 {
            (b.num / 3) as usize
        } else {
            b.num.max(1) as usize
        }
    };
    let mut marks: Vec<(usize, String)> = spec
        .sections
        .iter()
        .map(|s| (s.bar.min(spec.bars.len()), s.name.clone()))
        .collect();
    if marks.first().is_none_or(|m| m.0 > 0) {
        marks.insert(0, (0, "Intro".to_string()));
    }
    for (k, (start, name)) in marks.iter().enumerate() {
        let end = marks.get(k + 1).map_or(spec.bars.len(), |m| m.0);
        if end <= *start {
            continue;
        }
        let header = section_header(name);
        let _ = writeln!(out, "\n{header} {}", end - start);
        let bars: Vec<String> = spec.bars[*start..end]
            .iter()
            .map(|b| "/".repeat(beats(b)))
            .collect();
        let _ = writeln!(out, "{}", bars.join(" | "));
    }
    out
}

/// A section name as a keyflow header: a built-in type keeps its spoken
/// form (`Chorus 2`), anything else is bracketed (`[Guitar Solo]`).
fn section_header(name: &str) -> String {
    let first = name.split_whitespace().next().unwrap_or("");
    let rest: Vec<&str> = name.split_whitespace().skip(1).collect();
    let builtin = keyflow::sections::SectionType::parse(first)
        .ok()
        .filter(|t| !matches!(t, keyflow::sections::SectionType::Custom(_)));
    match builtin {
        // The number in the name is the occurrence, which keyflow counts
        // itself; a non-numeric remainder becomes a comment.
        Some(t) => {
            let note: Vec<&str> = rest
                .into_iter()
                .filter(|w| !w.chars().all(|c| c.is_ascii_digit()))
                .collect();
            if note.is_empty() {
                t.abbreviation()
            } else {
                format!("{} \"{}\"", t.abbreviation(), note.join(" "))
            }
        }
        None => format!("[{}]", name.trim()),
    }
}

pub struct SessionOpts<'a> {
    pub spec: &'a Path,
    pub out_dir: &'a Path,
}

/// Write `<title>.RPP` and `<title>.kf` into the song folder.
///
/// # Errors
/// The spec can't be read or parsed, or the files can't be written.
pub fn cmd_session(opts: &SessionOpts<'_>) -> Result<(), String> {
    let text =
        std::fs::read_to_string(opts.spec).map_err(|e| format!("{}: {e}", opts.spec.display()))?;
    let spec: Spec =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", opts.spec.display()))?;
    let tl = timeline(&spec)?;
    std::fs::create_dir_all(opts.out_dir).map_err(|e| e.to_string())?;
    let stem = crate::parts::file_safe(&spec.title);
    let chart = build_chart(&spec);
    let kf_path = opts.out_dir.join(format!("{stem}.kf"));
    std::fs::write(&kf_path, &chart).map_err(|e| e.to_string())?;
    #[cfg(feature = "reaper")]
    {
        let rpp = build_rpp(&spec, &tl);
        let rpp_path = opts.out_dir.join(format!("{stem}.RPP"));
        std::fs::write(&rpp_path, rpp).map_err(|e| e.to_string())?;
        println!(
            "{}: {} bars, {} count-in bar(s), {} sections, {} lyric syllables -> {}",
            spec.title,
            spec.bars.len(),
            tl.count_in_bars,
            spec.sections.len(),
            spec.lyrics.len(),
            rpp_path.display()
        );
    }
    #[cfg(not(feature = "reaper"))]
    {
        let _ = &tl;
        eprintln!(
            "note: the .RPP needs the `reaper` feature; wrote only {}",
            kf_path.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> Spec {
        Spec {
            title: "T".into(),
            artist: None,
            audio: "Media/t.wav".into(),
            audio_duration: 20.0,
            bars: vec![
                Bar {
                    start: 0.5,
                    quarters: 4.0,
                    num: 4,
                    den: 4,
                },
                Bar {
                    start: 2.5,
                    quarters: 4.0,
                    num: 4,
                    den: 4,
                },
                Bar {
                    start: 4.6,
                    quarters: 4.0,
                    num: 4,
                    den: 4,
                },
            ],
            end: 6.6,
            sections: vec![SectionMark {
                name: "Chorus 1".into(),
                bar: 1,
            }],
            lyrics: vec![],
            tempo: Some(120.0),
        }
    }

    #[test]
    fn first_bar_lands_on_a_barline_after_a_count_in() {
        let tl = timeline(&spec()).unwrap();
        assert_eq!(tl.count_in_bars, 1);
        assert!((tl.bar_times[0] - 2.0).abs() < 1e-9);
        assert!((tl.audio_pos - 1.5).abs() < 1e-9);
    }

    #[test]
    fn chart_sections_and_slashes() {
        let c = build_chart(&spec());
        assert!(c.contains("120bpm 4/4"));
        assert!(c.contains("IN 1\n////"));
        assert!(c.contains("CH 2\n//// | ////"));
    }
}
