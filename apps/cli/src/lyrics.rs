//! `kf lyrics-lrc` — put an LRC's lyrics into a chart as timed lyric lanes.

use std::path::Path;

use keyflow_lang::text::chart::Chart;
use keyflow_lang::text::lyric_lane::{self, PlaceOptions, Timeline};

/// Where the chart's bars sit in the recording.
pub enum Clock<'a> {
    /// Bar 1 starts this many seconds in; the chart's tempo does the rest.
    Downbeat(f64),
    /// A Songsterr `sync.json`; `first_bar` is the sync bar that is the
    /// chart's bar 1 (`None`: guess — skip a lone pickup bar).
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

/// Bar start times from a Songsterr `sync.json`: the first `main` video's
/// `bars[].start_s` (or its raw `points`).
fn read_sync(path: &Path) -> Result<Vec<f64>, String> {
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

fn bar_count(chart: &Chart) -> usize {
    chart.sections.iter().map(|s| s.measures().len()).sum()
}

fn bar_counts(chart: &Chart) -> Vec<usize> {
    chart.sections.iter().map(|s| s.measures().len()).collect()
}

pub fn cmd_lyrics_lrc(opts: LyricsLrcOpts<'_>) -> Result<(), String> {
    let source = std::fs::read_to_string(opts.chart)
        .map_err(|e| format!("{}: {e}", opts.chart.display()))?;
    if keyflow_lang::text::chart::parser::block::is_block_format(&source) {
        return Err(
            "lane-format charts (`sections {}` / `rhythm {}`) are not supported; \
                    use a classic section chart"
                .into(),
        );
    }
    let chart = keyflow_lang::text::chart::parse_chart(&source)?;
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

    let bars = bar_count(&chart);
    let timeline = match opts.clock {
        Clock::Downbeat(t) => Timeline::from_chart(&chart, t),
        Clock::Sync { path, first_bar } => {
            let starts = read_sync(path)?;
            // A sync with exactly one bar more than the chart almost always
            // starts on a pickup the chart leaves out.
            let first = first_bar.unwrap_or(usize::from(starts.len() == bars + 1));
            if first >= starts.len() {
                return Err(format!(
                    "--sync-first-bar {first} is past the sync's {} bars",
                    starts.len()
                ));
            }
            let starts = &starts[first..];
            if starts.len() != bars {
                eprintln!(
                    "note: the sync has {} bars from bar {first}, the chart {bars}; \
                     bars past the sync continue at the chart's tempo",
                    starts.len()
                );
            }
            Timeline::from_bar_starts(&chart, starts)
        }
    };

    let placement = lyric_lane::place_lrc(
        &lrc,
        &timeline,
        &PlaceOptions {
            grid: opts.grid,
            ..PlaceOptions::default()
        },
    );
    let out = lyric_lane::write_lanes(&source, &chart, &placement)?;

    // The lanes must not move a single bar.
    let again = keyflow_lang::text::chart::parse_chart(&out)
        .map_err(|e| format!("the chart with lyrics no longer parses: {e}"))?;
    if bar_counts(&again) != bar_counts(&chart) {
        return Err("writing the lyric lanes changed the chart's bars; nothing written".into());
    }

    match opts.output {
        Some(path) => std::fs::write(path, &out).map_err(|e| format!("{}: {e}", path.display()))?,
        None => print!("{out}"),
    }

    let sung = lrc
        .lines
        .iter()
        .filter(|l| !l.text.trim().is_empty())
        .count();
    eprintln!(
        "placed {} of {sung} lyric lines in {} sections ({} bars, grid 1/{})",
        placement.placed_lines,
        placement.lanes.len(),
        bars,
        opts.grid
    );
    for u in &placement.unplaced {
        let m = (u.time / 60.0).floor();
        eprintln!(
            "  not placed [{:02}:{:05.2}] {}: {}",
            m,
            u.time - m * 60.0,
            u.reason,
            u.text
        );
    }
    Ok(())
}
