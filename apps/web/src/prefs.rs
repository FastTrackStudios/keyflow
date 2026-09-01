//! Small, boring preferences that belong to a browser rather than an
//! account.
//!
//! `localStorage`, not a cookie: nothing here is the server's business
//! and there is no server. A cookie would be sent with every request
//! this site does not make, and would need a consent conversation it
//! does not otherwise need. This is one boolean about how a text box
//! behaves, and it should stay on the machine that chose it.
//!
//! Every access is fallible and none of it matters: private windows,
//! cleared site data and browsers configured to refuse storage all make
//! these no-ops. A preference that cannot be read is simply the default,
//! and a preference that cannot be written is a preference that does not
//! persist — neither is worth an error path in a caller.

/// Whether the editor starts in vim mode. Off unless someone asked.
pub const VIM_MODE: &str = "keyflow.editor.vim";

#[cfg(target_arch = "wasm32")]
fn storage() -> Option<web_sys::Storage> {
    // `local_storage()` is `Err` when the browser refuses storage
    // outright, and `Ok(None)` when there simply is none.
    web_sys::window()?.local_storage().ok().flatten()
}

/// Read a stored boolean, or `default` if it has never been set.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn bool_or(key: &str, default: bool) -> bool {
    storage()
        .and_then(|s| s.get_item(key).ok().flatten())
        .map_or(default, |v| v == "1")
}

/// Remember a boolean. Silently does nothing where storage is refused.
#[cfg(target_arch = "wasm32")]
pub fn set_bool(key: &str, value: bool) {
    if let Some(s) = storage() {
        let _ = s.set_item(key, if value { "1" } else { "0" });
    }
}

// The site is checked for the host as well as for wasm, where there is
// no browser to remember anything: every preference is its default.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub const fn bool_or(_key: &str, default: bool) -> bool {
    default
}

#[cfg(not(target_arch = "wasm32"))]
pub const fn set_bool(_key: &str, _value: bool) {}
