//! Where the issuer sends people back.
//!
//! A screen that is almost never seen: it exists for the half-second
//! between `auth.fasttrackstudio.app` handing back an authorization code
//! and the site putting the person where they were. What it draws is
//! only what a slow token exchange, or a refused one, needs to say.
//!
//! # It restores the chart, and that is the whole point
//!
//! Everywhere else a callback screen can simply go home. Here, home is
//! not where anyone was: a Keyflow chart lives in the URL
//! ([`crate::chart_url`]), so the page someone left was carrying their
//! document. [`crate::return_to`] parked that path before the redirect,
//! and this is what spends it — with `replace`, not `push`, so the
//! browser's Back button steps over the callback rather than into a URL
//! carrying an authorization code that cannot be spent again.

use dioxus::prelude::*;

use crate::Route;
use crate::auth::use_auth;
use crate::oidc;
use crate::return_to;
use crate::routes::Shell;

/// `/auth/callback?code=…&state=…`.
///
/// The whole query arrives as one string rather than as named
/// parameters, because the issuer answers a refusal with `error=` and no
/// `code` at all — a signature demanding both would make the router
/// reject exactly the case this screen exists to explain.
#[component]
pub fn AuthCallback(query: String) -> Element {
    let mut auth = use_auth();
    let navigator = use_navigator();

    // `use_future`, so it runs once on mount. Redeeming twice would
    // present a spent authorization code and turn a successful sign-in
    // into a refusal.
    use_future(move || {
        let query = query.clone();
        async move {
            match oidc::parse_callback_query(&query) {
                Ok((code, state)) => {
                    let destination = auth.complete(code, state).await;
                    navigator.replace(destination);
                }
                // No navigation: the screen below explains what happened
                // and offers the way on. Leaving the person here is the
                // only place the issuer's reason is visible.
                Err(error) => auth.error.set(Some(error.to_string())),
            }
        }
    });

    let error = (auth.error)();

    rsx! {
        Shell {
            section { class: "kf-prose kf-auth-callback",
                match error {
                    None => rsx! {
                        h1 { "Signing you in…" }
                        p { "One moment — finishing with your FastTrackStudio account." }
                    },
                    Some(message) => rsx! {
                        h1 { "That sign-in did not finish" }
                        p { role: "alert", "{message}" }
                        p { "Nothing was lost — your chart is still where you left it." }
                        div { class: "kf-account-actions",
                            button {
                                class: "kf-account-submit",
                                onclick: move |_| auth.begin_sign_in(),
                                "Try again"
                            }
                            Link { class: "kf-account-link", to: back_to_work(),
                                "Back to what you were doing"
                            }
                        }
                    },
                }
            }
        }
    }
}

/// Where "back to what you were doing" goes when the sign-in failed.
///
/// [`return_to::peek`] rather than `take`: this is read on every render
/// of the failure screen, and it is also what "Try again" will restore.
/// Spending it here would leave the retry with nowhere to come back to.
fn back_to_work() -> NavigationTarget {
    match return_to::peek() {
        Some(parked) => parked.into(),
        None => Route::Editor {}.into(),
    }
}
