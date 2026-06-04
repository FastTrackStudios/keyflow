//! keyflow developer tasks — thin wrapper over the shared fts-repo battery.
use std::process::ExitCode;

fn main() -> ExitCode {
    // keyflow has no tracey specs and no REAPER harness; the generic gate
    // (fmt/clippy/check/nextest) is the whole surface.
    let cfg = fts_repo::XtaskConfig {
        nextest_profile: "ci".to_string(),
        run_doctests: false,
        run_tracey: false,
        ..fts_repo::XtaskConfig::default()
    };
    fts_repo::dispatch(&cfg, |_cmd, _rest| None)
}
