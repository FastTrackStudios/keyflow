//! The chart a session shows, from its provider's replies, in Keyflow's
//! language on stdout: render it headlessly with `kf png - -o chart.png`.
//!
//! ```text
//! cargo run -p keyflow-session --example session_chart -- \
//!     structure.json [timeline.json data.json] [--title "Song"] | kf png - -o chart.png
//! ```
//!
//! Each file is one `fts.session` reply (`Response` as JSON): the reply to
//! `ReadStructure`, and optionally to `ReadTimeline` and to `ReadData` (a
//! session with typed chart items). Without the last two the chart is the
//! session's road map alone.

use std::io::Write as _;
use std::process::ExitCode;

use keyflow_session::typed::Held;

const NO_TIMELINE: &str = r#"{"Timeline":{"lanes":{},"items":{}}}"#;
const NO_DATA: &str = r#"{"Data":{}}"#;

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let title = match args.iter().position(|a| a == "--title") {
        Some(i) if i + 1 < args.len() => {
            let title = args.remove(i + 1);
            args.remove(i);
            title
        }
        _ => String::new(),
    };
    let read = |path: &String| std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"));
    let replies = match args.as_slice() {
        [structure] => read(structure).map(|s| (s, NO_TIMELINE.to_owned(), NO_DATA.to_owned())),
        [structure, timeline, data] => {
            read(structure).and_then(|s| Ok((s, read(timeline)?, read(data)?)))
        }
        _ => Err(
            "usage: session_chart structure.json [timeline.json data.json] [--title T]".to_owned(),
        ),
    };
    let shown = replies.and_then(|(structure, timeline, data)| {
        Held::of_replies(&timeline, &structure, &data)
            .map(|held| held.shown_source(&title))
            .map_err(|e| e.to_string())
    });
    match shown {
        Ok(source) => {
            let _ = std::io::stdout().write_all(source.as_bytes());
            ExitCode::SUCCESS
        }
        Err(why) => {
            let _ = writeln!(std::io::stderr(), "{why}");
            ExitCode::FAILURE
        }
    }
}
