//! `kf svg --follow`: an engraving a page playing the recording follows.
//!
//! Each bar gets its time (`data-at`, seconds) and the SVG root
//! `data-follow-playhead` (+ `data-media`); see
//! `keyflow::engraver::export::follow`. The times come from, in order of
//! preference: a measured start per bar (`--sync`, a Songsterr
//! `sync.json`), else the chart's tempo, tempo changes and meters from
//! bar 1 at `--downbeat` (or the chart's `\downbeat`).

use std::path::Path;

use keyflow::Chart;
use keyflow::engraver::export::follow::Follow;
use keyflow_lang::text::bar_times::BarClock;

/// What the command line asks for.
#[derive(Debug, Default, Clone)]
pub struct FollowArgs<'a> {
    pub follow: bool,
    pub downbeat: Option<f64>,
    pub media: Option<&'a str>,
    pub sync: Option<&'a Path>,
    pub sync_first_bar: Option<usize>,
}

impl FollowArgs<'_> {
    fn asked(&self) -> bool {
        self.follow || self.downbeat.is_some() || self.media.is_some() || self.sync.is_some()
    }
}

/// The follow attributes for `chart`, or `None` when neither the command
/// line nor the chart's header (`\downbeat`, `\media`) asks for them — the
/// engraving is then exactly what it always was.
///
/// # Errors
///
/// The sync file cannot be read, or names no bars from `--sync-first-bar`.
pub fn follow_for(chart: &Chart, args: &FollowArgs<'_>) -> Result<Option<Follow>, String> {
    let header = &chart.settings;
    if !args.asked() && header.downbeat.is_none() && header.media.is_none() {
        return Ok(None);
    }
    let downbeat = args.downbeat.or(header.downbeat).unwrap_or(0.0);
    let media = args
        .media
        .map(str::to_owned)
        .or_else(|| header.media.clone());

    let measured = match args.sync {
        Some(path) => {
            let starts = crate::lyrics::read_sync(path)?;
            let first = args
                .sync_first_bar
                .unwrap_or_else(|| crate::lyrics::guess_first_sync_bar(&starts));
            if first >= starts.len() {
                return Err(format!(
                    "--sync-first-bar {first} is past the sync's {} bars",
                    starts.len()
                ));
            }
            Some(starts[first..].to_vec())
        }
        None => None,
    };
    let clock = BarClock {
        downbeat: measured
            .as_ref()
            .and_then(|m| m.first().copied())
            .unwrap_or(downbeat),
        measured: measured.as_deref(),
    };
    Ok(Some(Follow::new(chart, clock, media)))
}
