//! The account affordance in the site header.
//!
//! It gates nothing. Signed out it is a "Sign in" button; signed in it
//! shows who you are and offers a way out. The editor behaves
//! identically either way — that is the point.
//!
//! # There is no form here any more
//!
//! There used to be: an email and password panel that dropped out of
//! this button, with a toggle between signing in and creating an
//! account, and its own vocabulary of failures ("that password is too
//! weak"). All of it is gone, along with the password policy it was
//! quietly restating. Sign-in is now a redirect to
//! `auth.fasttrackstudio.app`, which is where the social providers live
//! and where the password rules are actually enforced — see
//! [`crate::auth`] for why that trade is worth a page navigation.
//!
//! What is left is two buttons and a sentence, because that is all a
//! side door on a site that works without it needs to be.

use dioxus::prelude::*;

use crate::auth::{AuthState, use_auth};

#[component]
pub fn AccountMenu() -> Element {
    let mut auth = use_auth();
    let mut open = use_signal(|| false);

    let state = (auth.state)();
    let pending = (auth.pending)();

    rsx! {
        div { class: "kf-account",
            match state {
                // Render nothing until the stored session has been
                // checked, rather than flashing "Sign in" at someone who
                // turns out to be signed in a moment later.
                AuthState::Loading => rsx! {
                    span { class: "kf-account-loading", aria_hidden: "true" }
                },
                AuthState::SignedOut => rsx! {
                    button {
                        class: "kf-account-button",
                        disabled: pending,
                        onclick: move |_| open.toggle(),
                        "Sign in"
                    }
                },
                AuthState::SignedIn(ref account) => {
                    let label = account.label();
                    rsx! {
                        span { class: "kf-account-name", "{label}" }
                        button {
                            class: "kf-account-button",
                            onclick: move |_| auth.sign_out(),
                            "Sign out"
                        }
                    }
                }
            }

            if open() && matches!(state, AuthState::SignedOut) {
                SignInPanel { on_close: move |()| open.set(false) }
            }
        }
    }
}

/// Two doors to the same place.
///
/// Both leave for the issuer; the difference is only which of its pages
/// someone lands on. It stays a panel rather than collapsing into a bare
/// button because the sentence above the buttons is the point — someone
/// about to be sent to another domain should have been told so first,
/// and told that their work is not at risk.
#[component]
fn SignInPanel(on_close: EventHandler<()>) -> Element {
    let mut auth = use_auth();
    let pending = (auth.pending)();
    let error = (auth.error)();

    rsx! {
        div { class: "kf-account-panel",
            div { class: "kf-account-form",
                h2 { class: "kf-account-title", "Sign in" }
                p { class: "kf-account-note",
                    "The editor works without one. An account keeps your charts."
                }
                p { class: "kf-account-note",
                    "You finish at auth.fasttrackstudio.app — the same account as the
                     rest of FastTrackStudio, and where “Continue with GitHub” lives.
                     Whatever you are working on will still be here when you return."
                }

                if let Some(message) = error {
                    p { class: "kf-account-error", role: "alert", "{message}" }
                }

                div { class: "kf-account-actions",
                    button {
                        r#type: "button",
                        class: "kf-account-submit",
                        disabled: pending,
                        onclick: move |_| auth.begin_sign_in(),
                        if pending { "Taking you there…" } else { "Continue" }
                    }
                    button {
                        r#type: "button",
                        class: "kf-account-link",
                        disabled: pending,
                        onclick: move |_| auth.begin_sign_up(),
                        "Create an account"
                    }
                    button {
                        r#type: "button",
                        class: "kf-account-link",
                        onclick: move |_| on_close.call(()),
                        "Close"
                    }
                }
            }
        }
    }
}
