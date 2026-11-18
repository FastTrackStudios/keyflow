//! A chart on a session: its playhead.
//!
//! A chart pane on `session:<id>` draws where the session's leader is
//! (`session_host::shown_now`: presence in the host's clock), in the
//! chart's own time: the session's seconds from the song's bar 1
//! (`bar_one`, the end of a count-in), so bar 1 of the chart is the song's
//! bar 1 however the multitracks start. Read from the session this copy
//! holds, else through `fts.session`'s structure (whoever provides it:
//! Keyflow, or the DAW engine). The pane is otherwise the chart's own
//! editor (palettes, toolbar, input modes), editing through `fts.session`.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use keyflow_session::ChartSource;

use crate::session_host;

/// How often the playhead is moved (about 30 a second).
const FRAME: Duration = Duration::from_millis(33);
/// How often a chart's start is read again from a session held elsewhere.
const STRUCTURE_EVERY: Duration = Duration::from_secs(2);

/// The chart time to draw for a session at `session_seconds`: from the
/// song's bar 1, which is `chart_start` seconds into the session. Before
/// it (the count-in) the chart's start.
#[must_use]
pub fn chart_seconds(session_seconds: f64, chart_start: f64) -> f64 {
    (session_seconds - chart_start).max(0.0)
}

/// Starts the playhead for every chart pane on a session, once.
pub fn start_playhead() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let _ = std::thread::Builder::new()
            .name("keyflow-session-playhead".into())
            .spawn(run_playhead);
    });
}

fn run_playhead() {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return;
    };
    let mut starts: HashMap<String, (f64, Instant)> = HashMap::new();
    let mut drawn: HashMap<u64, Option<u64>> = HashMap::new();
    loop {
        std::thread::sleep(FRAME);
        let panes: Vec<(u64, String)> = crate::open_charts()
            .iter()
            .filter_map(|(key, open)| {
                open.path
                    .strip_prefix(keyflow_session::SESSION_PREFIX)
                    .map(|doc| (*key, doc.to_owned()))
            })
            .collect();
        if panes.is_empty() {
            continue;
        }
        let shown = session_host::shown_now();
        for (key, doc) in panes {
            let start = match session_host::held_chart_start(&doc) {
                Some(start) => start,
                None => {
                    let stale = starts
                        .get(&doc)
                        .is_none_or(|(_, at)| at.elapsed() >= STRUCTURE_EVERY);
                    // Whether the chart writes its own lead-in (an intro, a
                    // count-in): one that does not starts after the session's.
                    let has_lead_in = crate::open_charts().get(&key).is_none_or(|open| {
                        open.data.read(|s| {
                            // Its first section past a count-in (which is
                            // before the session's bar 1 either way).
                            s.chart
                                .sections
                                .iter()
                                .map(|section| &section.section.section_type)
                                .find(|kind| !matches!(kind, keyflow::SectionType::CountIn))
                                .is_none_or(|kind| {
                                    matches!(
                                        kind,
                                        keyflow::SectionType::Intro | keyflow::SectionType::Opening
                                    )
                                })
                        })
                    });
                    if stale
                        && let Some(read) =
                            runtime.block_on(chart_start_elsewhere(&doc, has_lead_in))
                    {
                        starts.insert(doc.clone(), (read, Instant::now()));
                    }
                    starts.get(&doc).map_or(0.0, |(start, _)| *start)
                }
            };
            let playhead = shown.map(|(at, _)| chart_seconds(at, start));
            // Moved by less than a millisecond: not drawn again.
            let bits = playhead.map(|s| (s * 1000.0).round().to_bits());
            if drawn.get(&key) == Some(&bits) {
                continue;
            }
            drawn.insert(key, bits);
            if let Some(open) = crate::open_charts().get(&key) {
                open.data.update(|s| s.playhead_secs = playhead);
                open.wake.wake();
            }
        }
    }
}

/// The chart's start in a session another extension provides, through
/// `fts.session`'s structure.
async fn chart_start_elsewhere(doc: &str, chart_has_lead_in: bool) -> Option<f64> {
    let source = ChartSource::Session {
        doc: doc.to_owned(),
    };
    let call = source.structure_call().ok()??;
    let handle = crate::handle().ok()?;
    let reply = crate::ask_session(&handle, &call).await.ok()?;
    ChartSource::chart_start_aligned(&reply, chart_has_lead_in).ok()
}

#[cfg(test)]
mod tests {
    use super::chart_seconds;

    #[test]
    fn the_charts_time_starts_at_the_songs_bar_one() {
        // Four bars of count-in at 160 BPM: the verse is 6 s in.
        assert!((chart_seconds(6.0, 6.0) - 0.0).abs() < 1e-12);
        assert!((chart_seconds(9.0, 6.0) - 3.0).abs() < 1e-12);
        // In the count-in, the chart's start.
        assert!((chart_seconds(2.0, 6.0) - 0.0).abs() < 1e-12);
        // No count-in: the session's own time.
        assert!((chart_seconds(9.0, 0.0) - 9.0).abs() < 1e-12);
    }
}
