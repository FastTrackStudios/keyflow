//! The FastTrackStudio account, on the Keyflow site.
//!
//! Deliberately additive. The editor has no backend and needs no
//! account — a chart travels in the URL, so sharing one is sharing a
//! link. Nothing here gates any of that. An account is for *keeping*
//! charts, and this is the groundwork: proof that the site can sign a
//! person in against `auth.fasttrackstudio.app`, and a place for saved
//! charts to hang off later.
//!
//! The session lives in `localStorage`, so a reload does not sign you
//! out, and travels as a bearer token rather than a cookie — the auth
//! server is a different origin, and a bearer header avoids CORS
//! credential rules entirely.

use std::sync::Arc;

use auth_client::MemoryTokenStore;
use auth_http::{AuthHttpClient, Session, SignUpRequest};
use dioxus::prelude::*;

/// Where the account server lives.
///
/// Overridable at build time so a developer can point the site at a
/// local server (`KEYFLOW_AUTH_URL=http://localhost:8080 dx serve`)
/// without editing code.
pub fn auth_base_url() -> String {
    option_env!("KEYFLOW_AUTH_URL")
        .unwrap_or("https://auth.fasttrackstudio.app")
        .to_owned()
}

/// Who is signed in, as far as the browser knows.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum AuthState {
    /// Startup, before the stored token has been checked. Distinct from
    /// `SignedOut` so the header does not flash "Sign in" at someone who
    /// is in fact signed in.
    #[default]
    Loading,
    SignedOut,
    SignedIn(Session),
}

impl AuthState {
    pub fn user_label(&self) -> Option<String> {
        match self {
            Self::SignedIn(session) => session
                .user
                .name
                .clone()
                .or_else(|| session.user.email.clone())
                .or(Some("Account".to_owned())),
            _ => None,
        }
    }
}

/// The account context: state plus the client that changes it.
#[derive(Clone)]
pub struct Auth {
    pub state: Signal<AuthState>,
    /// Last failure, for the form to show. Cleared on every attempt.
    pub error: Signal<Option<String>>,
    /// True while a request is in flight, so the form can disable itself
    /// rather than let someone submit twice.
    pub pending: Signal<bool>,
    client: AuthHttpClient,
}

impl Auth {
    pub async fn sign_in(&mut self, email: String, password: String) {
        self.run(self.client.clone(), move |client| async move {
            client.sign_in(email, password).await
        })
        .await;
    }

    pub async fn sign_up(&mut self, email: String, password: String) {
        self.run(self.client.clone(), move |client| async move {
            client.sign_up(&SignUpRequest::new(email, password)).await
        })
        .await;
    }

    /// Shared shape of both credential flows: mark pending, clear the
    /// last error, run, then land in exactly one of the two states.
    async fn run<F, Fut>(&mut self, client: AuthHttpClient, call: F)
    where
        F: FnOnce(AuthHttpClient) -> Fut,
        Fut: std::future::Future<Output = Result<Session, auth_http::AuthHttpError>>,
    {
        self.pending.set(true);
        self.error.set(None);
        match call(client).await {
            Ok(session) => self.state.set(AuthState::SignedIn(session)),
            Err(error) => {
                self.state.set(AuthState::SignedOut);
                self.error.set(Some(friendly(&error)));
            }
        }
        self.pending.set(false);
    }

    pub async fn sign_out(&mut self) {
        // The client clears the local token even if the request fails,
        // so this is safe to treat as always succeeding locally.
        let _ = self.client.sign_out().await;
        self.state.set(AuthState::SignedOut);
        self.error.set(None);
    }
}

/// Turn a client error into something worth showing a person.
///
/// The taxonomy `code` is the stable thing to match on; the server's
/// own message is a fallback rather than the primary text, because it
/// is written for an API consumer.
fn friendly(error: &auth_http::AuthHttpError) -> String {
    use auth_http::AuthHttpError;
    match error {
        AuthHttpError::Api { code, .. } if code == "invalid_credentials" => {
            "That email and password do not match an account.".into()
        }
        AuthHttpError::Api { code, .. } if code == "user_already_exists" => {
            "There is already an account with that email.".into()
        }
        AuthHttpError::Api { code, .. } if code == "weak_password" => {
            "That password is too weak — try a longer one.".into()
        }
        AuthHttpError::Api { code, .. } if code == "verification_required" => {
            "Check your email to verify the address before signing in.".into()
        }
        AuthHttpError::Transport(_) | AuthHttpError::NoToken => {
            "Could not reach the account server. Check your connection.".into()
        }
        AuthHttpError::Api { message, .. } => message.clone(),
        AuthHttpError::Store(_) => {
            "Could not save the session in this browser. Private browsing?".into()
        }
    }
}

/// Install the account context and resolve any stored session.
///
/// Call once, at the top of the app. Everything below reaches it with
/// [`use_auth`].
pub fn use_auth_provider() -> Auth {
    let auth = use_context_provider(|| {
        // On wasm the token persists in localStorage; anywhere else
        // (a host build of the site, tests) it lives for the process.
        #[cfg(target_arch = "wasm32")]
        let store: Arc<dyn auth_client::TokenStore> =
            Arc::new(auth_http::LocalStorageTokenStore::new("keyflow.session"));
        #[cfg(not(target_arch = "wasm32"))]
        let store: Arc<dyn auth_client::TokenStore> = Arc::new(MemoryTokenStore::new());

        Auth {
            state: Signal::new(AuthState::Loading),
            error: Signal::new(None),
            pending: Signal::new(false),
            client: AuthHttpClient::new(auth_base_url()).with_store(store),
        }
    });

    // Resolve the stored token once on mount. A token that the server
    // no longer accepts (expired, revoked from another device) lands in
    // `SignedOut` like any other failure — the user simply signs in.
    use_future({
        let mut auth = auth.clone();
        move || {
            let client = auth.client.clone();
            async move {
                if !client.has_token() {
                    auth.state.set(AuthState::SignedOut);
                    return;
                }
                match client.session().await {
                    Ok(session) => auth.state.set(AuthState::SignedIn(session)),
                    Err(_) => auth.state.set(AuthState::SignedOut),
                }
            }
        }
    });

    auth
}

/// The account context installed by [`use_auth_provider`].
pub fn use_auth() -> Auth {
    use_context::<Auth>()
}
