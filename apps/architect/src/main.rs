//! `keyflow-architect`: Keyflow's development app. The Architect app's
//! window with the Keyflow extension compiled in; `devenv up` runs it.

use architect_desktop::Distribution;

/// Its name, and its folder under the platform's config dir: its own, so
/// a development run never writes into a real Architect's settings.
const NAME: &str = "Keyflow";
const FOLDER: &str = "keyflow-dev";
const BUNDLE_ID: &str = "app.fasttrackstudio.keyflow-dev";

fn distribution() -> Distribution {
    architect_desktop::distribution::architect()
        .named(NAME)
        .app(FOLDER)
        .folder(FOLDER)
        .bundle_id(BUNDLE_ID)
        .version(env!("CARGO_PKG_VERSION"))
        .linked(&keyflow_extension::Keyflow)
}

fn main() -> std::process::ExitCode {
    architect_desktop::run(distribution())
}
