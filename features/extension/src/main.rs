//! Keyflow's process copy: the same [`keyflow_extension::Keyflow`] the app
//! links in, served over vox when a development copy runs
//! (`extension.toml`'s `[entry]`).

#[cfg(not(target_family = "wasm"))]
fn main() -> Result<(), architect_extensions::guest::GuestError> {
    architect_extensions::guest::Guest::extension(&keyflow_extension::Keyflow).serve()
}

// A browser has no processes: the extension is only ever linked in there.
#[cfg(target_family = "wasm")]
fn main() {}
