//! The account affordance in the site header.
//!
//! It gates nothing. Signed out it is a "Sign in" button; signed in it
//! shows who you are and offers a way out. The editor behaves
//! identically either way — that is the point.

use dioxus::prelude::*;

use crate::auth::{AuthState, use_auth};

#[component]
pub fn AccountMenu() -> Element {
    let auth = use_auth();
    let mut open = use_signal(|| false);

    let state = (auth.state)();

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
                        onclick: move |_| open.toggle(),
                        "Sign in"
                    }
                },
                AuthState::SignedIn(_) => {
                    let label = state.user_label().unwrap_or_else(|| "Account".to_owned());
                    rsx! {
                        span { class: "kf-account-name", "{label}" }
                        button {
                            class: "kf-account-button",
                            onclick: {
                                let auth = auth.clone();
                                move |_| {
                                    let mut auth = auth.clone();
                                    spawn(async move { auth.sign_out().await });
                                }
                            },
                            "Sign out"
                        }
                    }
                }
            }

            if open() && matches!(state, AuthState::SignedOut) {
                SignInPanel { on_close: move |_| open.set(false) }
            }
        }
    }
}

/// Email + password, with a toggle between signing in and creating an
/// account. One panel rather than two screens: this is a side door on a
/// site that works without it, not a front gate.
#[component]
fn SignInPanel(on_close: EventHandler<()>) -> Element {
    let auth = use_auth();
    let mut email = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut creating = use_signal(|| false);

    let pending = (auth.pending)();
    let error = (auth.error)();

    let submit = {
        let auth = auth.clone();
        move |event: FormEvent| {
            // Keep the browser from navigating; the form is submitted
            // over fetch.
            event.prevent_default();
            let mut auth = auth.clone();
            let (email, password, creating) = (email(), password(), creating());
            spawn(async move {
                if creating {
                    auth.sign_up(email, password).await;
                } else {
                    auth.sign_in(email, password).await;
                }
            });
        }
    };

    rsx! {
        div { class: "kf-account-panel",
            form {
                class: "kf-account-form",
                onsubmit: submit,

                h2 { class: "kf-account-title",
                    if creating() { "Create an account" } else { "Sign in" }
                }
                p { class: "kf-account-note",
                    "The editor works without one. An account keeps your charts."
                }

                label { r#for: "kf-account-email", "Email" }
                input {
                    id: "kf-account-email",
                    r#type: "email",
                    autocomplete: "email",
                    required: true,
                    value: "{email}",
                    disabled: pending,
                    oninput: move |event| email.set(event.value()),
                }

                label { r#for: "kf-account-password", "Password" }
                input {
                    id: "kf-account-password",
                    r#type: "password",
                    // Tells a password manager which of the two this is,
                    // so it offers to fill rather than to save, or the
                    // reverse.
                    autocomplete: if creating() { "new-password" } else { "current-password" },
                    required: true,
                    value: "{password}",
                    disabled: pending,
                    oninput: move |event| password.set(event.value()),
                }

                if let Some(message) = error {
                    p { class: "kf-account-error", role: "alert", "{message}" }
                }

                div { class: "kf-account-actions",
                    button {
                        r#type: "submit",
                        class: "kf-account-submit",
                        disabled: pending,
                        if pending {
                            "Working…"
                        } else if creating() {
                            "Create account"
                        } else {
                            "Sign in"
                        }
                    }
                    button {
                        r#type: "button",
                        class: "kf-account-link",
                        disabled: pending,
                        onclick: move |_| creating.toggle(),
                        if creating() { "I already have an account" } else { "Create one instead" }
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
