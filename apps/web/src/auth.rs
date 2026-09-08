//! The FastTrackStudio account, on the Keyflow site.
//!
//! Deliberately additive. The editor has no backend and needs no
//! account — a chart travels in the URL, so sharing one is sharing a
//! link. Nothing here gates any of that. An account is for *keeping*
//! charts, and this is the groundwork: proof that the site can sign a
//! person in against `auth.fasttrackstudio.app`, and a place for saved
//! charts to hang off later.
//!
//! # Why the redirect, and not the password form it replaced
//!
//! This used to post an email and password straight to the issuer's
//! session API (`auth_http::AuthHttpClient::sign_in`) and keep the
//! session it returned. That worked, and cost three things:
//!
//! * **No social sign-in.** "Continue with GitHub" and "Continue with
//!   Google" live on the issuer's own hosted `/login` page and complete
//!   by redirect. A site that never sends anyone there cannot offer
//!   them — and for a tool whose users mostly arrive from a repository,
//!   GitHub is the account they already have.
//! * **No refresh token.** `offline_access` is granted through the
//!   redirect flow and not through the password API, so a session lapsed
//!   in an hour instead of lasting a week. Someone who came back to
//!   their charts the next morning was signed out.
//! * **Keyflow handling passwords.** The site has no use for one. Not
//!   seeing it is strictly better than seeing it carefully.
//!
//! So sign-in is now: build a PKCE verifier and a random `state`, park
//! them — and where we were, see [`crate::return_to`] — and hand the
//! browser to the issuer. It comes back to `/auth/callback` with a code,
//! which [`Auth::complete`] redeems. The password paths are gone; the
//! hosted page owns sign-up too.
//!
//! # What is kept
//!
//! The three-state shape — [`AuthState::Loading`] before the stored
//! session is resolved, so the header does not flash "Sign in" at
//! someone who turns out to be signed in — and `localStorage`
//! persistence, so a reload does not sign you out. The token still
//! travels as a bearer rather than a cookie: the issuer is a different
//! origin, and a bearer header avoids CORS credential rules entirely.
//!
//! # Sign-in goes over HTTP, not the issuer's vox lane
//!
//! Task documents this trap in `crates/ui/src/central_login.rs`, and it
//! applies here for the same reason. The issuer speaks vox, and dialling
//! it would look like the tidier choice. But a lane needs both ends on
//! the same vox wire version, and the issuer is a separately released
//! binary; when the two drift the handshake fails, and it fails as an
//! authentication error rather than as a version mismatch — which sends
//! you looking at credentials for a problem that is nothing to do with
//! them. `/oauth2/authorize` and `/oauth2/token` are plain HTTP with a
//! specification behind them, and they do not skew.

// Signing in happens in a browser, so most of this module is behind
// `cfg(target_arch = "wasm32")` and has no caller on the host. The site
// is still *checked* for the host — that is what keeps the shared half
// compiling and the tests below runnable under `just test` — and dead
// code there is the shape of the thing, not a warning worth acting on.
// See CLAUDE.md on `just web-check` for the other side of that split.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use dioxus::prelude::*;

use crate::oidc::{self, OidcError, UserInfo};
use crate::return_to;

/// Where the account server lives.
///
/// Overridable at build time so a developer can point the site at a
/// local issuer (`KEYFLOW_AUTH_URL=http://localhost:8099 dx serve`)
/// without editing code. Whatever it points at must have Keyflow's
/// redirect URI registered — the issuer refuses an unregistered one at
/// `/oauth2/authorize`, before a login page is ever drawn.
#[must_use]
pub fn auth_base_url() -> String {
    option_env!("KEYFLOW_AUTH_URL")
        .unwrap_or("https://auth.fasttrackstudio.app")
        .trim_end_matches('/')
        .to_owned()
}

/// The callback URL to send as `redirect_uri`.
///
/// Derived from the page's own origin, so localhost development and the
/// deployed site both work with no build flag and no code edit. It must
/// still match a `redirect_uris` entry registered for the `keyflow`
/// client at the issuer, character for character; the registered set is
/// `https://keyflow.fasttrackstudio.app/auth/callback`,
/// `http://localhost:8080/auth/callback`,
/// `http://localhost:8766/auth/callback`, and the iOS app's
/// `app.fasttrackstudio.keyflow://auth/callback`. A dev server on some
/// other port is refused at authorize — serve on 8080 or 8766.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn redirect_uri() -> Option<String> {
    let origin = web_sys::window()?.location().origin().ok()?;
    Some(format!("{origin}{}", return_to::CALLBACK_PATH))
}

/// The host build has no origin to read. It also never signs in — the
/// site is checked for the host so the `cfg(wasm32)` code around it
/// cannot rot, not run there.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn redirect_uri() -> Option<String> {
    None
}

/// Who is signed in, as far as the browser knows.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum AuthState {
    /// Startup, before the stored session has been checked. Distinct
    /// from `SignedOut` so the header does not flash "Sign in" at
    /// someone who is in fact signed in.
    #[default]
    Loading,
    SignedOut,
    SignedIn(Account),
}

/// The person, as the issuer describes them at `/oauth2/userinfo`.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Account {
    /// The issuer's user id — the principal a saved chart would
    /// eventually be keyed on.
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
}

impl Account {
    /// What to call them in the header. A name, then an address, then a
    /// word: an account with neither claim set is still an account.
    #[must_use]
    pub fn label(&self) -> String {
        self.name
            .clone()
            .or_else(|| self.email.clone())
            .unwrap_or_else(|| "Account".to_owned())
    }
}

impl From<UserInfo> for Account {
    fn from(user: UserInfo) -> Self {
        Self {
            sub: user.sub,
            email: user.email,
            name: user.name,
        }
    }
}

/// What is kept in `localStorage` between visits.
///
/// The refresh token sits here beside the access token, and that is a
/// deliberate acceptance rather than an oversight: any script on this
/// origin can read it, which is the standing tradeoff for a browser SPA
/// with no backend of its own to hold a cookie in. What it buys is a
/// returning visitor who is still signed in instead of being bounced
/// through the issuer. The issuer's own rotation and revocation are what
/// bound the exposure.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct StoredSession {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    /// When the access token stops being accepted, in seconds since the
    /// epoch. Absent when the issuer did not say, which reads as "no
    /// idea — present it and find out".
    #[serde(default)]
    expires_at_unix: Option<i64>,
    #[serde(default)]
    account: Option<Account>,
}

impl StoredSession {
    /// Whether the access token is past its stated life.
    ///
    /// Thirty seconds of slack, because the alternative is presenting a
    /// token that expires in flight and reading the resulting 401 as
    /// "signed out".
    fn access_expired(&self) -> bool {
        self.expires_at_unix
            .is_some_and(|deadline| now_unix() + 30 >= deadline)
    }
}

/// The key the session is stored under. Namespaced per app: two
/// FastTrackStudio sites on one origin must not overwrite each other.
const SESSION_KEY: &str = "keyflow.session";

/// Where the verifier waits while the browser is away at the issuer.
///
/// `sessionStorage`, not `localStorage`: it is meaningful for one
/// attempt in one tab, and `localStorage` would leave it readable by
/// every later page load in the origin.
const VERIFIER_KEY: &str = "keyflow.auth.pkce.verifier";
const STATE_KEY: &str = "keyflow.auth.pkce.state";

/// Why a sign-in did not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// The browser API we need is missing, or we are not in a browser.
    NoBrowser,
    /// No sign-in was started in this tab, so there is no verifier to
    /// redeem with.
    NoAttemptInProgress,
    /// The issuer refused, or answered with something unusable.
    Exchange(String),
}

impl From<OidcError> for AuthError {
    fn from(error: OidcError) -> Self {
        Self::Exchange(error.to_string())
    }
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBrowser => f.write_str("this browser will not let the site sign you in."),
            Self::NoAttemptInProgress => {
                f.write_str("no sign-in was started in this tab — try again.")
            }
            Self::Exchange(why) => write!(f, "the account server refused the sign-in: {why}"),
        }
    }
}

impl std::error::Error for AuthError {}

/// The account context: state, plus the things that change it.
#[derive(Clone, Copy)]
pub struct Auth {
    pub state: Signal<AuthState>,
    /// Last failure, for the header to show. Cleared on every attempt.
    pub error: Signal<Option<String>>,
    /// True while the browser is on its way somewhere, or a token
    /// exchange is in flight, so a button can disable itself rather than
    /// let someone start two sign-ins.
    pub pending: Signal<bool>,
}

impl Auth {
    /// Send the browser to the issuer to sign in.
    ///
    /// Returns only on failure: on success the page is already
    /// navigating away.
    pub fn begin_sign_in(&mut self) {
        self.leave_for_issuer(oidc::authorize_url);
    }

    /// Send the browser to the issuer's hosted sign-up form.
    ///
    /// There is no local form to fall back to, and that is the point:
    /// the issuer owns account creation, including which social
    /// providers are on offer this month, and a second form here would
    /// be a second password policy to keep in step with the first.
    pub fn begin_sign_up(&mut self) {
        self.leave_for_issuer(oidc::sign_up_url);
    }

    /// The shared half of both: park the attempt, then leave.
    fn leave_for_issuer(&mut self, url_for: fn(&str, &str, &oidc::Pkce) -> String) {
        self.error.set(None);
        self.pending.set(true);
        if let Err(error) = start_redirect(url_for) {
            self.pending.set(false);
            self.error.set(Some(error.to_string()));
        }
    }

    /// Redeem the authorization code the issuer sent us back with, and
    /// land in [`AuthState::SignedIn`] or [`AuthState::SignedOut`].
    ///
    /// Returns where to send the person next: wherever they were when
    /// they left, which on this site may be a whole chart. See
    /// [`crate::return_to`].
    pub async fn complete(&mut self, code: String, state: String) -> String {
        self.pending.set(true);
        self.error.set(None);
        match redeem(&code, &state).await {
            Ok(session) => {
                let account = session.account.clone();
                remember(&session);
                self.state.set(match account {
                    Some(account) => AuthState::SignedIn(account),
                    None => AuthState::SignedOut,
                });
            }
            Err(error) => {
                self.state.set(AuthState::SignedOut);
                self.error.set(Some(error.to_string()));
            }
        }
        self.pending.set(false);
        return_to::take()
    }

    /// Forget the session locally.
    ///
    /// Local only, on purpose. The token lives at the issuer and the
    /// site has no session of its own to end; what "sign out" means here
    /// is that this browser stops holding one. Someone who wants the
    /// issuer-side session ended goes to the issuer, which is the only
    /// place that can end it for every app at once.
    pub fn sign_out(&mut self) {
        forget();
        self.state.set(AuthState::SignedOut);
        self.error.set(None);
        self.pending.set(false);
    }
}

/// Install the account context and resolve any stored session.
///
/// Call once, at the top of the app. Everything below reaches it with
/// [`use_auth`].
pub fn use_auth_provider() -> Auth {
    let auth = use_context_provider(|| Auth {
        state: Signal::new(AuthState::Loading),
        error: Signal::new(None),
        pending: Signal::new(false),
    });

    // Resolve the stored session once on mount. A token the issuer no
    // longer accepts — expired past its refresh, revoked from another
    // device — lands in `SignedOut` like any other failure, and the
    // person simply signs in.
    use_future(move || {
        let mut auth = auth;
        async move {
            let resolved = resolve().await;
            auth.state.set(resolved);
        }
    });

    auth
}

/// The account context installed by [`use_auth_provider`].
#[must_use]
pub fn use_auth() -> Auth {
    use_context::<Auth>()
}

// ── Starting ─────────────────────────────────────────────────────────

/// Park the attempt and hand the browser to the issuer.
///
/// Order matters here and is not obvious: the verifier, the state and
/// the destination are all written to storage BEFORE `assign` is called.
/// Once the browser starts navigating, this context is on its way out —
/// and a verifier that did not reach storage first is a sign-in that can
/// never complete, while a chart that did not reach storage first is a
/// chart that is simply gone.
#[cfg(target_arch = "wasm32")]
fn start_redirect(url_for: fn(&str, &str, &oidc::Pkce) -> String) -> Result<(), AuthError> {
    let redirect_uri = redirect_uri().ok_or(AuthError::NoBrowser)?;
    let pkce = oidc::Pkce::from_entropy(random_bytes::<32>()?, random_bytes::<16>()?);

    let storage = session_storage().ok_or(AuthError::NoBrowser)?;
    storage
        .set_item(VERIFIER_KEY, pkce.verifier())
        .map_err(|_| AuthError::NoBrowser)?;
    storage
        .set_item(STATE_KEY, pkce.state())
        .map_err(|_| AuthError::NoBrowser)?;
    return_to::stash_current();

    let url = url_for(&auth_base_url(), &redirect_uri, &pkce);
    web_sys::window()
        .ok_or(AuthError::NoBrowser)?
        .location()
        .assign(&url)
        .map_err(|_| AuthError::NoBrowser)
}

#[cfg(not(target_arch = "wasm32"))]
fn start_redirect(_url_for: fn(&str, &str, &oidc::Pkce) -> String) -> Result<(), AuthError> {
    Err(AuthError::NoBrowser)
}

// ── Finishing ────────────────────────────────────────────────────────

/// Check the state, spend the code, and ask who it belongs to.
#[cfg(target_arch = "wasm32")]
async fn redeem(code: &str, state: &str) -> Result<StoredSession, AuthError> {
    let redirect_uri = redirect_uri().ok_or(AuthError::NoBrowser)?;
    let storage = session_storage().ok_or(AuthError::NoBrowser)?;
    let parked = |key: &str| {
        storage
            .get_item(key)
            .ok()
            .flatten()
            .filter(|value| !value.is_empty())
            .ok_or(AuthError::NoAttemptInProgress)
    };
    let pkce = oidc::Pkce::resume(parked(VERIFIER_KEY)?, parked(STATE_KEY)?);
    pkce.check_state(state)?;

    // Cleared before the exchange, not after: an authorization code is
    // single-use, so a verifier left behind past this point can only be
    // replayed, never legitimately reused.
    let _ = storage.remove_item(VERIFIER_KEY);
    let _ = storage.remove_item(STATE_KEY);

    let body = oidc::token_request_body(&redirect_uri, code, &pkce);
    let tokens = oidc::tokens_from(&post_form(&token_url(), &body).await?)?;
    let account = fetch_user(&tokens.access_token).await?;
    Ok(session_from(tokens, Some(account), None))
}

#[cfg(not(target_arch = "wasm32"))]
async fn redeem(_code: &str, _state: &str) -> Result<StoredSession, AuthError> {
    Err(AuthError::NoBrowser)
}

/// What the browser knows about the account on a fresh page load.
///
/// The refresh token is the reason this is more than "do we have a
/// token": an access token good for an hour is expired on nearly every
/// return visit, and without this step someone who signed in yesterday
/// would be sent back to the issuer today. With it, the site quietly
/// trades the refresh token for a new access token and they are simply
/// still signed in.
///
/// A refresh the issuer refuses — revoked, rotated out, past its own
/// week — clears the stored session rather than leaving a dead token in
/// place to fail again on the next page.
#[cfg(target_arch = "wasm32")]
async fn resolve() -> AuthState {
    let Some(stored) = load() else {
        return AuthState::SignedOut;
    };

    let session = if stored.access_expired() {
        match refresh(&stored).await {
            Ok(session) => session,
            Err(_) => {
                forget();
                return AuthState::SignedOut;
            }
        }
    } else {
        stored
    };

    // Asking who this is also confirms the token is live: an access
    // token can be revoked well before it expires, and its stated
    // deadline says nothing about that.
    match adopt(session).await {
        Some(state) => state,
        None => {
            forget();
            AuthState::SignedOut
        }
    }
}

/// Confirm a session at `/oauth2/userinfo` and remember it, refreshing
/// once if the token turns out to be dead.
///
/// One retry, and only one: a token that was fine a moment ago may have
/// just been revoked, and the refresh answers that. A second attempt
/// would answer nothing new, and a loop against an issuer that is simply
/// down is worse than a sign-in button.
#[cfg(target_arch = "wasm32")]
async fn adopt(session: StoredSession) -> Option<AuthState> {
    if let Ok(account) = fetch_user(&session.access_token).await {
        let session = StoredSession {
            account: Some(account.clone()),
            ..session
        };
        remember(&session);
        return Some(AuthState::SignedIn(account));
    }

    let refreshed = refresh(&session).await.ok()?;
    let account = fetch_user(&refreshed.access_token).await.ok()?;
    let session = StoredSession {
        account: Some(account.clone()),
        ..refreshed
    };
    remember(&session);
    Some(AuthState::SignedIn(account))
}

#[cfg(not(target_arch = "wasm32"))]
async fn resolve() -> AuthState {
    AuthState::SignedOut
}

/// Trade the refresh token for a fresh access token.
///
/// The issuer may or may not hand back a new refresh token; when it does
/// not, the old one stays valid and is kept — see
/// [`oidc::refresh_request_body`].
#[cfg(target_arch = "wasm32")]
async fn refresh(session: &StoredSession) -> Result<StoredSession, AuthError> {
    let refresh_token = session
        .refresh_token
        .clone()
        .ok_or(AuthError::NoAttemptInProgress)?;
    let body = oidc::refresh_request_body(&refresh_token);
    let tokens = oidc::tokens_from(&post_form(&token_url(), &body).await?)?;
    Ok(session_from(
        tokens,
        session.account.clone(),
        Some(refresh_token),
    ))
}

/// Fold a token response into a session, keeping what it did not
/// replace.
fn session_from(
    tokens: oidc::Tokens,
    account: Option<Account>,
    previous_refresh: Option<String>,
) -> StoredSession {
    StoredSession {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token.or(previous_refresh),
        expires_at_unix: tokens.expires_in.map(|seconds| now_unix() + seconds),
        account,
    }
}

#[cfg(target_arch = "wasm32")]
async fn fetch_user(access_token: &str) -> Result<Account, AuthError> {
    let url = format!("{}/oauth2/userinfo", auth_base_url());
    let text = get_with_bearer(&url, access_token).await?;
    Ok(oidc::user_from(&text)?.into())
}

fn token_url() -> String {
    format!("{}/oauth2/token", auth_base_url())
}

// ── Persistence ──────────────────────────────────────────────────────
//
// One `localStorage` key holding one JSON object, rather than a key per
// field. A half-written session — an access token stored, its deadline
// not — is a session that behaves as though it never expires, and one
// key cannot be half-written.

#[cfg(target_arch = "wasm32")]
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

#[cfg(target_arch = "wasm32")]
fn load() -> Option<StoredSession> {
    let raw = local_storage()?.get_item(SESSION_KEY).ok().flatten()?;
    // A value we cannot parse is one from an older format or a corrupt
    // write — including the shape the password flow used to store here.
    // That is "no session", not an error path: the person signs in
    // again, and this time through the issuer.
    serde_json::from_str(&raw).ok()
}

#[cfg(target_arch = "wasm32")]
fn remember(session: &StoredSession) {
    if let (Some(storage), Ok(raw)) = (local_storage(), serde_json::to_string(session)) {
        let _ = storage.set_item(SESSION_KEY, &raw);
    }
}

#[cfg(target_arch = "wasm32")]
fn forget() {
    if let Some(storage) = local_storage() {
        let _ = storage.remove_item(SESSION_KEY);
    }
}

// The site is checked for the host as well as for wasm, where there is
// no browser to remember anything.
#[cfg(not(target_arch = "wasm32"))]
fn remember(_session: &StoredSession) {}

#[cfg(not(target_arch = "wasm32"))]
fn forget() {}

// ── Browser bits ─────────────────────────────────────────────────────

/// PKCE needs unguessable bytes. `Math.random` is not that — it is
/// seeded per context and predictable, and a guessable verifier is not a
/// weaker PKCE but no PKCE at all.
#[cfg(target_arch = "wasm32")]
fn random_bytes<const N: usize>() -> Result<[u8; N], AuthError> {
    let crypto = web_sys::window()
        .ok_or(AuthError::NoBrowser)?
        .crypto()
        .map_err(|_| AuthError::NoBrowser)?;
    let mut buf = [0u8; N];
    crypto
        .get_random_values_with_u8_array(&mut buf)
        .map_err(|_| AuthError::NoBrowser)?;
    Ok(buf)
}

#[cfg(target_arch = "wasm32")]
fn session_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.session_storage().ok().flatten()
}

/// Seconds since the epoch.
///
/// `Date::now` is milliseconds and a float; the truncation is the point,
/// and a token deadline does not care about the fraction.
#[cfg(target_arch = "wasm32")]
#[allow(clippy::cast_possible_truncation)]
fn now_unix() -> i64 {
    (js_sys::Date::now() / 1000.0) as i64
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::cast_possible_wrap)]
fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs() as i64)
}

// `window.fetch`, not reqwest. The site deliberately carries no HTTP
// client crate: reqwest 0.12 arrived here with `auth-http`, and this
// tree already has reqwest 0.13 through iroh — two majors in one wasm
// binary is a hundred `duplicate symbol: intounderlyingsource_*` errors
// out of rust-lld, because each compiles its own copy of the wasm-streams
// glue. Two form posts and one GET do not need a client crate; dropping
// `auth-http` with the password form took one of the two majors out.

#[cfg(target_arch = "wasm32")]
async fn post_form(url: &str, body: &str) -> Result<String, AuthError> {
    let headers = web_sys::Headers::new().map_err(js)?;
    headers
        .set("content-type", "application/x-www-form-urlencoded")
        .map_err(js)?;
    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    init.set_headers(&headers);
    init.set_body(&wasm_bindgen::JsValue::from_str(body));
    send(web_sys::Request::new_with_str_and_init(url, &init).map_err(js)?).await
}

#[cfg(target_arch = "wasm32")]
async fn get_with_bearer(url: &str, token: &str) -> Result<String, AuthError> {
    let headers = web_sys::Headers::new().map_err(js)?;
    headers
        .set("authorization", &format!("Bearer {token}"))
        .map_err(js)?;
    let init = web_sys::RequestInit::new();
    init.set_headers(&headers);
    send(web_sys::Request::new_with_str_and_init(url, &init).map_err(js)?).await
}

/// Send, and keep the body on failure — an issuer's refusal explains
/// itself there, and dropping it leaves only "it didn't work".
#[cfg(target_arch = "wasm32")]
async fn send(request: web_sys::Request) -> Result<String, AuthError> {
    use wasm_bindgen::JsCast as _;
    use wasm_bindgen_futures::JsFuture;

    let window = web_sys::window().ok_or(AuthError::NoBrowser)?;
    let value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(js)?;
    let response: web_sys::Response = value
        .dyn_into()
        .map_err(|_| AuthError::Exchange("fetch returned a non-Response".to_owned()))?;
    let status = response.status();
    let text = JsFuture::from(response.text().map_err(js)?)
        .await
        .map_err(js)?
        .as_string()
        .unwrap_or_default();
    if (200..300).contains(&status) {
        Ok(text)
    } else {
        Err(AuthError::Exchange(format!("{status}: {text}")))
    }
}

#[cfg(target_arch = "wasm32")]
fn js(error: wasm_bindgen::JsValue) -> AuthError {
    AuthError::Exchange(format!("{error:?}"))
}

#[cfg(test)]
mod tests {
    use super::{Account, AuthState, StoredSession, auth_base_url, now_unix, session_from};
    use crate::oidc;

    #[test]
    fn the_default_issuer_is_the_one_keyflow_is_registered_at() {
        assert_eq!(auth_base_url(), "https://auth.fasttrackstudio.app");
    }

    /// The header shows a name if there is one, an address if there is
    /// not, and a word if there is neither. An account with no display
    /// name is still an account.
    #[test]
    fn a_person_is_labelled_by_the_best_claim_they_have() {
        let named = Account {
            sub: "u1".to_owned(),
            email: Some("e@x.test".to_owned()),
            name: Some("Nom".to_owned()),
        };
        assert_eq!(named.label(), "Nom");
        assert_eq!(
            Account {
                name: None,
                ..named.clone()
            }
            .label(),
            "e@x.test"
        );
        assert_eq!(
            Account {
                name: None,
                email: None,
                ..named
            }
            .label(),
            "Account"
        );
    }

    /// `Loading` is a state, not a flavour of `SignedOut` — the header
    /// must not flash "Sign in" at someone who is signed in, and it is
    /// the default precisely so that it cannot.
    #[test]
    fn loading_is_the_default_and_is_not_signed_out() {
        assert_eq!(AuthState::default(), AuthState::Loading);
        assert_ne!(AuthState::Loading, AuthState::SignedOut);
    }

    /// A refresh response that omits `refresh_token` means "keep the one
    /// you have". Dropping it there would sign the person out a week
    /// early, on the *successful* path.
    #[test]
    fn a_refresh_keeps_the_token_it_was_not_given_a_replacement_for() {
        let tokens = oidc::tokens_from(r#"{"access_token":"new","expires_in":3600}"#).unwrap();
        let session = session_from(tokens, None, Some("old-refresh".to_owned()));
        assert_eq!(session.access_token, "new");
        assert_eq!(session.refresh_token.as_deref(), Some("old-refresh"));
        assert!(!session.access_expired(), "a fresh hour is not expired");
    }

    /// And a response that *does* carry one replaces it — rotation is
    /// the issuer's to do, and holding the spent token would fail the
    /// next refresh.
    #[test]
    fn a_rotated_refresh_token_replaces_the_old_one() {
        let tokens =
            oidc::tokens_from(r#"{"access_token":"new","refresh_token":"rotated"}"#).unwrap();
        let session = session_from(tokens, None, Some("old-refresh".to_owned()));
        assert_eq!(session.refresh_token.as_deref(), Some("rotated"));
    }

    /// The slack is what keeps a token that expires in flight from
    /// reading as "signed out".
    #[test]
    fn a_token_about_to_expire_counts_as_expired() {
        let about_to = StoredSession {
            access_token: "t".to_owned(),
            refresh_token: None,
            expires_at_unix: Some(now_unix() + 5),
            account: None,
        };
        assert!(about_to.access_expired());

        let comfortable = StoredSession {
            expires_at_unix: Some(now_unix() + 600),
            ..about_to.clone()
        };
        assert!(!comfortable.access_expired());

        // No stated deadline is not "expired" — it is "the issuer did
        // not say", and the only way to find out is to present it.
        let undated = StoredSession {
            expires_at_unix: None,
            ..about_to
        };
        assert!(!undated.access_expired());
    }

    /// The session survives a page reload as JSON under one key, and a
    /// stored shape from before this flow existed reads as "no session"
    /// rather than as an error.
    #[test]
    fn a_session_round_trips_through_storage_and_junk_does_not() {
        let session = StoredSession {
            access_token: "a".to_owned(),
            refresh_token: Some("r".to_owned()),
            expires_at_unix: Some(now_unix() + 3600),
            account: Some(Account {
                sub: "u1".to_owned(),
                email: None,
                name: Some("Nom".to_owned()),
            }),
        };
        let raw = serde_json::to_string(&session).unwrap();
        assert_eq!(
            serde_json::from_str::<StoredSession>(&raw).unwrap(),
            session
        );
        // What the password flow used to leave behind.
        assert!(
            serde_json::from_str::<StoredSession>(r#"{"token":"t","user_id":"u"}"#).is_err(),
            "an old session shape must not be adopted as a new one"
        );
    }
}
