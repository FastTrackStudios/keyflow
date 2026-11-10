//! `kf lyrics-lrc` and `kf lyrics-sheet` — put a song's lyrics into a
//! chart as timed lyric lanes.

use std::path::Path;

use keyflow_lang::text::chart::Chart;
use keyflow_lang::text::lyric_lane::{self, PlaceOptions, Placement, Timeline};

/// Where the chart's bars sit in the recording.
pub enum Clock<'a> {
    /// Bar 1 (after any count-in) starts this many seconds in; the chart's
    /// tempo does the rest.
    Downbeat(f64),
    /// A Songsterr `sync.json`; `first_bar` is the sync bar that is the
    /// chart's bar 1 (`None`: guess — skip a short lead-in bar).
    Sync {
        path: &'a Path,
        first_bar: Option<usize>,
    },
}

pub struct LyricsLrcOpts<'a> {
    pub chart: &'a Path,
    pub lrc: &'a Path,
    pub clock: Clock<'a>,
    /// Seconds added to every LRC time (a late LRC wants a negative one).
    pub offset: f64,
    pub grid: u32,
    /// `None` writes to stdout.
    pub output: Option<&'a Path>,
}

pub struct LyricsSheetOpts<'a> {
    pub chart: &'a Path,
    /// A chord-over-lyrics sheet, or with `map` a bar/beat/word map.
    pub sheet: &'a Path,
    pub map: bool,
    pub grid: u32,
    pub output: Option<&'a Path>,
}

/// Bar start times from a Songsterr `sync.json`: the first `main` video's
/// `bars[].start_s` (or its raw `points`).
pub(crate) fn read_sync(path: &Path) -> Result<Vec<f64>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let videos: Vec<&serde_json::Value> = match &json {
        serde_json::Value::Array(v) => v.iter().collect(),
        other => vec![other],
    };
    let video = videos
        .iter()
        .find(|v| v.get("kind").and_then(|k| k.as_str()) == Some("main"))
        .or(videos.first())
        .ok_or_else(|| format!("{}: no synced video", path.display()))?;
    let from_bars: Option<Vec<f64>> = video.get("bars").and_then(|b| b.as_array()).map(|bars| {
        bars.iter()
            .filter_map(|b| b.get("start_s").and_then(|s| s.as_f64()))
            .collect()
    });
    let starts = match from_bars {
        Some(s) if !s.is_empty() => s,
        _ => video
            .get("points")
            .and_then(|p| p.as_array())
            .map(|p| p.iter().filter_map(|x| x.as_f64()).collect())
            .unwrap_or_default(),
    };
    if starts.is_empty() {
        return Err(format!(
            "{}: no bar times (`bars[].start_s` or `points`)",
            path.display()
        ));
    }
    Ok(starts)
}

/// The sync bar that is the chart's bar 1: skip a lead-in bar that is
/// clearly shorter than the one after it (a 2/4 pickup before 4/4).
pub(crate) fn guess_first_sync_bar(starts: &[f64]) -> usize {
    match starts {
        [a, b, c, ..] if (b - a) < 0.75 * (c - b) => 1,
        _ => 0,
    }
}

fn bar_counts(chart: &Chart) -> Vec<usize> {
    chart.sections.iter().map(|s| s.measures().len()).collect()
}

fn read_chart(path: &Path) -> Result<(String, Chart), String> {
    let source = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if keyflow_lang::text::chart::parser::block::is_block_format(&source) {
        return Err(
            "lane-format charts (`sections {}` / `rhythm {}`) are not supported; \
             use a classic section chart"
                .into(),
        );
    }
    let chart = keyflow_lang::text::chart::parse_chart(&source)?;
    Ok((source, chart))
}

/// Write the lanes, check no bar moved, and save (or print) the chart.
fn write_out(
    source: &str,
    chart: &Chart,
    placement: &Placement,
    output: Option<&Path>,
) -> Result<(), String> {
    let out = lyric_lane::write_lanes(source, chart, placement)?;
    let again = keyflow_lang::text::chart::parse_chart(&out)
        .map_err(|e| format!("the chart with lyrics no longer parses: {e}"))?;
    if bar_counts(&again) != bar_counts(chart) {
        return Err("writing the lyric lanes changed the chart's bars; nothing written".into());
    }
    match output {
        Some(path) => std::fs::write(path, &out).map_err(|e| format!("{}: {e}", path.display())),
        None => {
            print!("{out}");
            Ok(())
        }
    }
}

fn report_unplaced(placement: &Placement) {
    for u in &placement.unplaced {
        let at = match (u.time, u.bar) {
            (Some(t), _) => {
                let m = (t / 60.0).floor();
                format!("[{:02}:{:05.2}]", m, t - m * 60.0)
            }
            (None, Some(bar)) => format!("bar {bar}"),
            (None, None) => String::new(),
        };
        eprintln!("  not placed {at} {}: {}", u.reason, u.text);
    }
}

pub fn cmd_lyrics_lrc(opts: LyricsLrcOpts<'_>) -> Result<(), String> {
    let (source, chart) = read_chart(opts.chart)?;
    let lrc_text =
        std::fs::read_to_string(opts.lrc).map_err(|e| format!("{}: {e}", opts.lrc.display()))?;
    let mut lrc = keyflow_lang::text::proto::lrc::parse(&lrc_text);
    if opts.offset != 0.0 {
        let off = opts.offset as f32;
        for line in &mut lrc.lines {
            line.start += off;
            for w in &mut line.words {
                w.start += off;
            }
        }
    }

    let count_in = lyric_lane::count_in_bars(&chart);
    let real_bars = bar_counts(&chart).iter().sum::<usize>() - count_in;
    let timeline = match opts.clock {
        Clock::Downbeat(t) => Timeline::from_chart(&chart, t),
        Clock::Sync { path, first_bar } => {
            let starts = read_sync(path)?;
            let first = first_bar.unwrap_or_else(|| guess_first_sync_bar(&starts));
            if first >= starts.len() {
                return Err(format!(
                    "--sync-first-bar {first} is past the sync's {} bars",
                    starts.len()
                ));
            }
            let starts = &starts[first..];
            eprintln!(
                "sync bar {first} = chart bar 1{}",
                if count_in > 0 {
                    format!(" (after the {count_in}-bar count-in)")
                } else {
                    String::new()
                }
            );
            if starts.len() != real_bars {
                eprintln!(
                    "note: the sync has {} bars from there, the chart {real_bars}; \
                     bars past the sync continue at the chart's tempo",
                    starts.len()
                );
            }
            Timeline::from_bar_starts(&chart, starts, count_in)
        }
    };

    let placement = lyric_lane::place_lrc(
        &lrc,
        &timeline,
        &PlaceOptions {
            grid: opts.grid,
            ..PlaceOptions::default()
        },
        count_in,
    );
    write_out(&source, &chart, &placement, opts.output)?;

    let sung = lrc
        .lines
        .iter()
        .filter(|l| !l.text.trim().is_empty())
        .count();
    eprintln!(
        "placed {} of {sung} lyric lines in {} sections ({real_bars} bars, grid 1/{}, offset {:+.2}s)",
        placement.placed_lines,
        placement.lanes.len(),
        opts.grid,
        opts.offset
    );
    report_unplaced(&placement);
    Ok(())
}

pub fn cmd_lyrics_sheet(opts: LyricsSheetOpts<'_>) -> Result<(), String> {
    let (source, chart) = read_chart(opts.chart)?;
    let text = std::fs::read_to_string(opts.sheet)
        .map_err(|e| format!("{}: {e}", opts.sheet.display()))?;
    let (words, notes) = if opts.map {
        (lyric_lane::read_word_map(&text)?, Vec::new())
    } else {
        lyric_lane::read_chord_sheet(&text, &chart)
    };
    let placement = lyric_lane::place_bar_words(
        &chart,
        &words,
        &PlaceOptions {
            grid: opts.grid,
            ..PlaceOptions::default()
        },
    );
    write_out(&source, &chart, &placement, opts.output)?;
    eprintln!(
        "placed {} lyric lines ({} words) in {} sections",
        placement.placed_lines,
        words.len(),
        placement.lanes.len()
    );
    for n in notes {
        eprintln!("  note: {n}");
    }
    report_unplaced(&placement);
    Ok(())
}
