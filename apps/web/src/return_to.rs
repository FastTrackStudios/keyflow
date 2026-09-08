//! Not losing the chart when we leave for the issuer.
//!
//! This is the part of the redirect flow that is Keyflow's own problem.
//! Everywhere else, "sign in" happens on a page whose state the server
//! already holds, so leaving the tab and coming back costs nothing. Here
//! the chart someone is editing *is* the URL — [`crate::chart_url`]
//! deflates it into a `/c/:data` path segment, and there is no server
//! and no database behind it. A redirect that forgets where it started
//! does not merely inconvenience: it destroys the document.
//!
//! So before the browser is sent to `auth.fasttrackstudio.app`, the
//! current path is parked here, and the `/auth/callback` screen navigates
//! back to it once the code is redeemed. Someone who signs in from the
//! editor lands back in the editor, with their chart.
//!
//! # Why the path and not the chart
//!
//! Parking the chart *source* would be a second encoding of the same
//! thing, and a second thing to keep in step with the route table. The
//! path already carries the chart, losslessly, and it also carries which
//! screen someone was on — the workbench, a guide chapter, the appendix.
//! One value restores all of it.
//!
//! # Why `sessionStorage`
//!
//! It belongs to one attempt in one tab, exactly like the PKCE verifier
//! beside it. In `localStorage` a chart abandoned mid-sign-in months ago
//! would still be sitting there, and a later, unrelated sign-in would
//! navigate to it.
//!
//! # Why it is sanitised
//!
//! What comes back out of storage is fed to the router as a destination.
//! [`sanitize`] admits only a path on this origin, so a value that was
//! tampered with (or written by some other script on the origin) cannot
//! turn "finish signing in" into a trip to somebody else's site with a
//! fresh token in the tab.

// Parking a destination only happens on the way to the issuer, which
// only happens in a browser; the host build has no caller for half of
// this. The host half of `store` is not dead weight either way — it is
// what makes the round trip below a real test on the target `just test`
// runs.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

/// Where the destination waits while the browser is away at the issuer.
pub const KEY: &str = "keyflow.auth.return_to";

/// Where to go when there is nothing parked, or what was parked is not
/// usable. The editor rather than the landing page: signing in is
/// something people do while working, not while reading.
pub const FALLBACK: &str = "/editor";

/// Reduce a parked value to a path this site may navigate to, or reject
/// it.
///
/// The rules, and what each is keeping out:
///
/// * It must start with `/` — an absolute path on this origin. A value
///   like `https://elsewhere.test/` is an open redirect wearing a
///   destination's clothes.
/// * It must not start with `//` — a protocol-relative URL, which is the
///   same open redirect with the scheme left off, and which *does* start
///   with `/`.
/// * It must not contain a backslash or an ASCII control character.
///   Browsers normalise `\` to `/` in URLs, so `/\evil.test` is one more
///   spelling of the same trick, and a control character is not
///   something a route ever contains.
/// * It must not be the callback itself. Returning to `/auth/callback`
///   with the verifier already spent is a loop that ends in "no sign-in
///   was started in this tab".
#[must_use]
pub fn sanitize(raw: &str) -> Option<String> {
    let path = raw.trim();
    if !path.starts_with('/')
        || path.starts_with("//")
        || path.contains('\\')
        || path.chars().any(|c| c.is_ascii_control())
    {
        return None;
    }
    let route = path.split(['?', '#']).next().unwrap_or(path);
    if route == CALLBACK_PATH || route.starts_with(&format!("{CALLBACK_PATH}/")) {
        return None;
    }
    Some(path.to_owned())
}

/// The route the issuer sends people back to. Named here as well as in
/// the route table because [`sanitize`] has to refuse it.
pub const CALLBACK_PATH: &str = "/auth/callback";

/// Park a destination. A value [`sanitize`] refuses is dropped rather
/// than stored: the fallback is a worse landing than the editor someone
/// was in, but it is a landing, and a rejected value is one we have
/// decided not to trust.
pub fn stash(path: &str) {
    match sanitize(path) {
        Some(clean) => store::set(&clean),
        None => store::clear(),
    }
}

/// Take the parked destination, clearing it.
///
/// Cleared on read, not on arrival: the value is spent by the navigation
/// it causes, and one left behind would hijack the next sign-in in this
/// tab. Sanitised again on the way out — storage is writable by anything
/// on the origin, so what went in is not necessarily what comes back.
#[must_use]
pub fn take() -> String {
    let parked = store::take();
    parked
        .as_deref()
        .and_then(sanitize)
        .unwrap_or_else(|| FALLBACK.to_owned())
}

/// Look at the parked destination without spending it.
///
/// For the failure screen, which offers both "try again" and "back to
/// what you were doing" and must not have the first of those consume
/// what the second needs. `None` when there is nothing usable parked —
/// the caller then picks its own landing rather than being handed
/// [`FALLBACK`] as though it had been asked for.
#[must_use]
pub fn peek() -> Option<String> {
    store::peek().as_deref().and_then(sanitize)
}

/// Park wherever the browser currently is — path, query and fragment.
///
/// Called immediately before the redirect. On a host build there is no
/// location to read and nothing to park; the site is checked for the
/// host as well as for wasm.
#[cfg(target_arch = "wasm32")]
pub fn stash_current() {
    let Some(location) = web_sys::window().map(|w| w.location()) else {
        return;
    };
    let mut here = location.pathname().unwrap_or_default();
    if let Ok(search) = location.search() {
        here.push_str(&search);
    }
    if let Ok(hash) = location.hash() {
        here.push_str(&hash);
    }
    stash(&here);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn stash_current() {}

/// The parking space itself: `sessionStorage` in a browser, a process
/// slot everywhere else.
///
/// The host half is not a stub for its own sake — it is what lets the
/// round trip below be a real test on the target `just test` runs,
/// rather than something only a browser could exercise.
mod store {
    #[cfg(target_arch = "wasm32")]
    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.session_storage().ok().flatten()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn set(value: &str) {
        if let Some(storage) = storage() {
            let _ = storage.set_item(super::KEY, value);
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn take() -> Option<String> {
        let storage = storage()?;
        let value = storage.get_item(super::KEY).ok().flatten();
        let _ = storage.remove_item(super::KEY);
        value
    }

    #[cfg(target_arch = "wasm32")]
    pub fn peek() -> Option<String> {
        storage()?.get_item(super::KEY).ok().flatten()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn clear() {
        if let Some(storage) = storage() {
            let _ = storage.remove_item(super::KEY);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    thread_local! {
        static PARKED: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn set(value: &str) {
        PARKED.with(|slot| *slot.borrow_mut() = Some(value.to_owned()));
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn take() -> Option<String> {
        PARKED.with(|slot| slot.borrow_mut().take())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn peek() -> Option<String> {
        PARKED.with(|slot| slot.borrow().clone())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn clear() {
        PARKED.with(|slot| *slot.borrow_mut() = None);
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use super::{FALLBACK, peek, sanitize, stash, take};
    use crate::{Route, chart_url};

    /// The one that matters: a chart lives in the URL, so the round trip
    /// through storage has to give back a path that still decodes to the
    /// same chart. Anything less and signing in from the editor throws
    /// away the document.
    #[test]
    fn a_chart_survives_the_round_trip_to_the_issuer_and_back() {
        let source = keyflow_ui::examples::EXAMPLE_THRILLER;
        let encoded = chart_url::encode(source);
        let path = format!("/c/{encoded}");

        stash(&path);
        let restored = take();
        assert_eq!(restored, path, "the parked chart path came back changed");

        // Not just the same string — the same *chart*, reached through
        // the same route the router would resolve it to.
        let route = Route::from_str(&restored).expect("the restored path is a route");
        let Route::Chart { data } = route else {
            panic!("a /c/… path must restore as the chart screen");
        };
        assert_eq!(chart_url::decode(&data).unwrap(), source);
    }

    /// Taking it clears it. A destination left behind would send the
    /// *next* sign-in in this tab somewhere nobody asked to go.
    #[test]
    fn the_destination_is_spent_by_taking_it() {
        stash("/learn/rhythm");
        assert_eq!(take(), "/learn/rhythm");
        assert_eq!(take(), FALLBACK, "the parked value outlived its navigation");
    }

    /// Looking is not spending. The failure screen offers "try again"
    /// and "back to what you were doing" side by side, and reading the
    /// second must leave the first something to return to.
    #[test]
    fn peeking_leaves_the_destination_parked() {
        stash("/c/abc");
        assert_eq!(peek().as_deref(), Some("/c/abc"));
        assert_eq!(peek().as_deref(), Some("/c/abc"));
        assert_eq!(take(), "/c/abc");
        assert_eq!(peek(), None);
    }

    /// Query and fragment travel too — the workbench and the guide both
    /// put state there.
    #[test]
    fn the_query_and_fragment_come_back_as_well() {
        stash("/guide/chords?from=home#voicings");
        assert_eq!(take(), "/guide/chords?from=home#voicings");
    }

    /// Every spelling of "somewhere that is not this site". What comes
    /// out of storage is a navigation target, and storage is writable by
    /// anything on the origin.
    #[test]
    fn nothing_off_this_origin_is_a_destination() {
        for hostile in [
            "https://elsewhere.test/",
            "//elsewhere.test/",
            "/\\elsewhere.test",
            "javascript:alert(1)",
            "editor",
            "",
            "/editor\n/x",
        ] {
            assert_eq!(sanitize(hostile), None, "{hostile:?} was admitted");
            stash(hostile);
            assert_eq!(take(), FALLBACK, "{hostile:?} survived as a destination");
        }
    }

    /// Returning to the callback is a loop: the verifier it wants was
    /// spent by the redemption that got us here.
    #[test]
    fn the_callback_is_not_somewhere_to_return_to() {
        assert_eq!(sanitize("/auth/callback"), None);
        assert_eq!(sanitize("/auth/callback?code=x&state=y"), None);
        // A route that merely starts with the same letters is fine.
        assert!(sanitize("/auth/callbacks-explained").is_some());
    }

    /// Nothing parked at all — a first visit, or a tab whose storage was
    /// refused — lands somewhere useful rather than nowhere.
    #[test]
    fn with_nothing_parked_the_fallback_is_the_editor() {
        let _ = take();
        assert_eq!(take(), FALLBACK);
        assert!(
            Route::from_str(FALLBACK).is_ok(),
            "{FALLBACK} must be a route"
        );
    }
}
