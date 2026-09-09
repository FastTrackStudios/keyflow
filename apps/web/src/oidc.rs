//! The redirect sign-in, minus the plumbing.
//!
//! Every FastTrackStudio app — Task, Session, Signal, Keyflow, Ignition
//! — signs people in the same way: send the browser to the issuer, get
//! an authorization code back, redeem it for a token. Doing that once
//! per app is one chance per app to build the challenge wrong, and the
//! failure mode is unusually cruel: the issuer answers a bad PKCE
//! exchange with a generic refusal that names no field, so an app with a
//! subtly wrong base64 alphabet looks exactly like an app with a wrong
//! client id.
//!
//! So the parts that are easy to get wrong and impossible to debug live
//! here, in one module, with tests. It is a port of
//! `auth_client::oidc` in architect — the same shape Task's
//! `central_login` builds on. It began as a *copy* rather than a
//! dependency because the site was pinned to an architect that predated
//! that module, and the pin could not move on its own.
//!
//! **That pin has moved**: the site and Task are both on v0.8.2, so the
//! crate is now reachable and this file is redundant. Replacing it is a
//! deliberate follow-up rather than a drive-by — the tests below are the
//! contract to keep passing when it happens.
//!
//! # What this module does not do
//!
//! No HTTP, no browser, no randomness. That is what keeps it testable on
//! the host, where `just test` runs and where there is no browser to
//! stub — every one of the ways to get PKCE wrong is checked below on a
//! target that never loads a page.
//!
//! The caller supplies:
//!
//! * **entropy** — [`Pkce::from_entropy`] takes the bytes rather than
//!   generating them, because the right source is platform-specific
//!   (`crypto.getRandomValues` in a browser) and the wrong one silently
//!   defeats PKCE. Making it an argument means nothing can reach for
//!   `Math.random` without someone noticing.
//! * **the two requests** — build them with [`authorize_url`] and
//!   [`token_request_body`] / [`refresh_request_body`], send them
//!   however the app already sends things, then read the answer with
//!   [`tokens_from`].
//! * **somewhere to park the verifier** while the browser is away. See
//!   [`crate::auth`], which parks it in `sessionStorage`.

// Every caller of this module is in the browser half of [`crate::auth`],
// which the host build compiles out. The tests below are then the only
// host-side use — which is the point of keeping it pure, not a reason to
// hear about it on every `cargo check`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};

/// The OIDC client id Keyflow is registered under at the issuer.
///
/// Matches the `oidcClients` entry in the auth server's deployment; a
/// mismatch is refused at `/oauth2/authorize` before anything else, with
/// a message that does not say which side is wrong.
pub const CLIENT_ID: &str = "keyflow";

/// What the site asks the issuer for.
///
/// `offline_access` is the one that is not boilerplate, and it is half
/// the reason this flow replaced the password form: it is what makes the
/// issuer mint a refresh token, and a refresh token is the difference
/// between a session that lapses in an hour and one that lasts a week.
/// The password API grants no such thing.
///
/// `forge:github` is registered for this client and asked for here so
/// that a later "open this chart from a repo" does not need a second
/// trip through consent. It grants nothing the site does not use.
pub const SCOPE: &str = "openid email profile offline_access forge:github";

/// What went wrong reading the issuer's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OidcError {
    /// The response was not JSON.
    Malformed(String),
    /// The response parsed but lacked something a session needs.
    Missing(&'static str),
    /// The `state` coming back is not the one that went out.
    ///
    /// Either a stale tab finishing an abandoned attempt or a forged
    /// callback. Both are refused: the difference is not knowable from
    /// here, and treating a forgery as staleness is the dangerous half.
    StateMismatch,
}

impl std::fmt::Display for OidcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(why) => write!(f, "the issuer's response was not JSON: {why}"),
            Self::Missing(what) => write!(f, "no {what} in the issuer's response"),
            Self::StateMismatch => f.write_str("sign-in state did not match"),
        }
    }
}

impl std::error::Error for OidcError {}

/// One sign-in attempt's secrets.
///
/// Created before leaving for the issuer, consumed on the way back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkce {
    verifier: String,
    challenge: String,
    state: String,
}

impl Pkce {
    /// Build from caller-supplied entropy.
    ///
    /// 32 bytes of verifier is the RFC 7636 floor once base64url'd (43
    /// characters) and exactly the 256 bits an `S256` challenge hashes.
    ///
    /// The bytes must come from a cryptographically secure source. A
    /// guessable verifier is not a weaker PKCE, it is no PKCE: the whole
    /// mechanism is "only the app that asked for this code knows the
    /// value behind the challenge".
    #[must_use]
    pub fn from_entropy(verifier_bytes: [u8; 32], state_bytes: [u8; 16]) -> Self {
        let verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);
        Self {
            challenge: challenge_for(&verifier),
            state: URL_SAFE_NO_PAD.encode(state_bytes),
            verifier,
        }
    }

    /// Reconstruct from a parked verifier and state, on the way back.
    #[must_use]
    pub fn resume(verifier: impl Into<String>, state: impl Into<String>) -> Self {
        let verifier = verifier.into();
        Self {
            challenge: challenge_for(&verifier),
            verifier,
            state: state.into(),
        }
    }

    /// Park this before navigating away — a verifier that did not reach
    /// storage first is a sign-in that can never complete.
    #[must_use]
    pub fn verifier(&self) -> &str {
        &self.verifier
    }

    #[must_use]
    pub fn challenge(&self) -> &str {
        &self.challenge
    }

    #[must_use]
    pub fn state(&self) -> &str {
        &self.state
    }

    /// Check the `state` that came back against the one that went out.
    ///
    /// # Errors
    ///
    /// [`OidcError::StateMismatch`] if they differ. An empty `returned`
    /// — a callback with no `state` at all — differs from every real
    /// state and is refused here rather than special-cased.
    pub fn check_state(&self, returned: &str) -> Result<(), OidcError> {
        if self.state == returned {
            Ok(())
        } else {
            Err(OidcError::StateMismatch)
        }
    }
}

/// `base64url(sha256(verifier))` — the `S256` challenge.
///
/// The base64 *url* alphabet, with no padding, over the ASCII bytes of
/// the verifier. Each of those three is a way to be wrong that the
/// issuer reports identically.
#[must_use]
pub fn challenge_for(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// The URL that starts a sign-in.
#[must_use]
pub fn authorize_url(issuer: &str, redirect_uri: &str, pkce: &Pkce) -> String {
    format!(
        "{}/oauth2/authorize\
         ?client_id={}\
         &redirect_uri={}\
         &response_type=code\
         &scope={}\
         &code_challenge={}\
         &code_challenge_method=S256\
         &state={}",
        issuer.trim_end_matches('/'),
        encode(CLIENT_ID),
        encode(redirect_uri),
        encode(SCOPE),
        encode(pkce.challenge()),
        encode(pkce.state()),
    )
}

/// The issuer's hosted sign-up page, told where to come back to.
///
/// The same authorize redirect would do — the hosted login page offers
/// "create an account" itself — but someone who clicked *Create
/// account* asked for the sign-up form, and landing them on a page
/// asking for a password they do not have yet is a worse answer than
/// the one they asked for.
///
/// Two things about the parameter, both learned the hard way:
///
/// * It is **`return_to`**. The issuer reads that name and no other, so
///   a `redirect_to` is not a redirect that goes to the wrong place —
///   it is silently ignored, and someone who finishes signing up lands
///   on the issuer's own page having never been given a code.
/// * Its value is a **same-origin path**, not a full URL. The issuer
///   refuses anything that does not begin with a single `/`, which is
///   what stops an open redirect, and falls back to its default page.
///   So this passes `/oauth2/authorize?…` rather than
///   `https://issuer/oauth2/authorize?…`.
#[must_use]
pub fn sign_up_url(issuer: &str, redirect_uri: &str, pkce: &Pkce) -> String {
    let issuer = issuer.trim_end_matches('/');
    let authorize = authorize_url(issuer, redirect_uri, pkce);
    // The path-and-query half of the authorize URL, which is the only
    // form the issuer's `return_to` accepts.
    let path = authorize.strip_prefix(issuer).unwrap_or(&authorize);
    format!("{issuer}/sign-up?return_to={}", encode(path))
}

/// The form-encoded body that redeems a code.
///
/// `content-type: application/x-www-form-urlencoded`.
#[must_use]
pub fn token_request_body(redirect_uri: &str, code: &str, pkce: &Pkce) -> String {
    format!(
        "grant_type=authorization_code\
         &code={}\
         &redirect_uri={}\
         &client_id={}\
         &code_verifier={}",
        encode(code),
        encode(redirect_uri),
        encode(CLIENT_ID),
        encode(pkce.verifier()),
    )
}

/// The form-encoded body that trades a refresh token for a fresh access
/// token.
///
/// No PKCE and no redirect: the verifier belonged to the authorization
/// code, which was spent. What authorises this exchange is possession of
/// the refresh token itself, which is why it lives in `localStorage`
/// under the same rules as the access token and is dropped the moment
/// the issuer refuses it.
#[must_use]
pub fn refresh_request_body(refresh_token: &str) -> String {
    format!(
        "grant_type=refresh_token&refresh_token={}&client_id={}",
        encode(refresh_token),
        encode(CLIENT_ID),
    )
}

/// Percent-encode a query-string value.
///
/// Unreserved characters per RFC 3986 pass through; everything else is
/// escaped. It matters most for `redirect_uri`, whose `:` and `/` would
/// otherwise read as structure in the URL being built.
fn encode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + raw.len() / 2);
    for byte in raw.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char);
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// What `/oauth2/token` hands back, for either grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tokens {
    pub access_token: String,
    /// Absent when the issuer chose not to mint one — which it does not
    /// do for `offline_access`, but a refresh response is allowed to
    /// omit it and mean "keep the one you have".
    pub refresh_token: Option<String>,
    /// Lifetime of the access token in seconds, as the issuer reported
    /// it. Turned into a wall-clock deadline by the caller, which is the
    /// only side that knows what time it is.
    pub expires_in: Option<i64>,
}

/// Read the tokens out of a `/oauth2/token` response.
///
/// # Errors
///
/// [`OidcError::Malformed`] if it is not JSON, [`OidcError::Missing`] if
/// there is no usable `access_token` — including one present but empty,
/// which would otherwise pass for a signed-in session.
pub fn tokens_from(body: &str) -> Result<Tokens, OidcError> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|e| OidcError::Malformed(e.to_string()))?;
    let text = |key: &str| {
        value
            .get(key)
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(std::borrow::ToOwned::to_owned)
    };
    Ok(Tokens {
        access_token: text("access_token").ok_or(OidcError::Missing("access_token"))?,
        refresh_token: text("refresh_token"),
        expires_in: value.get("expires_in").and_then(serde_json::Value::as_i64),
    })
}

/// Who a token belongs to, per `/oauth2/userinfo`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UserInfo {
    /// The issuer's user id — the principal a saved chart would
    /// eventually be keyed on.
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
}

/// Parse a `/oauth2/userinfo` response.
///
/// `sub` is the only claim required. Email and name are absent for an
/// account that has not set them, and a sign-in has to survive that
/// rather than failing on a missing display name.
///
/// # Errors
///
/// [`OidcError::Malformed`] if it is not JSON, [`OidcError::Missing`] if
/// there is no `sub`.
pub fn user_from(body: &str) -> Result<UserInfo, OidcError> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|e| OidcError::Malformed(e.to_string()))?;
    let text = |key: &str| {
        value
            .get(key)
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(std::borrow::ToOwned::to_owned)
    };
    Ok(UserInfo {
        sub: text("sub").ok_or(OidcError::Missing("sub"))?,
        email: text("email"),
        name: text("name"),
    })
}

/// `code` and `state` (or the issuer's refusal) out of a callback query
/// string — the part after `?`, with or without the leading `?`.
///
/// Written by hand rather than through a URL crate because the shape is
/// fixed by us on both ends and the site has no URL parser in its
/// dependency graph. Percent-decoding is applied because the issuer
/// encodes the values it puts in the query — an authorization code
/// containing a `+` redeems as a different code if it is not decoded.
///
/// # Errors
///
/// [`OidcError::Malformed`] carrying the issuer's own `error` value when
/// it refused, and [`OidcError::Missing`] when there is no `code` or no
/// `state`.
pub fn parse_callback_query(query: &str) -> Result<(String, String), OidcError> {
    let query = query.strip_prefix('?').unwrap_or(query);
    let query = query.split('#').next().unwrap_or_default();
    let (mut code, mut state, mut error) = (None, None, None);
    for pair in query.split('&') {
        let (key, raw) = pair.split_once('=').unwrap_or((pair, ""));
        let value = percent_decode(raw);
        match key {
            "code" => code = Some(value),
            "state" => state = Some(value),
            "error" => error = Some(value),
            _ => {}
        }
    }
    if let Some(error) = error {
        return Err(OidcError::Malformed(error));
    }
    match (code, state) {
        (Some(code), Some(state)) if !code.is_empty() && !state.is_empty() => Ok((code, state)),
        (None, _) => Err(OidcError::Missing("code")),
        _ => Err(OidcError::Missing("state")),
    }
}

fn percent_decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => match u8::from_str_radix(&raw[i + 1..i + 3], 16) {
                Ok(byte) => {
                    out.push(byte);
                    i += 3;
                }
                Err(_) => {
                    out.push(b'%');
                    i += 1;
                }
            },
            // A form-encoded query spells a space `+`; the issuer does
            // so for `scope`, and would for any claim it echoed back.
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::{
        CLIENT_ID, OidcError, Pkce, SCOPE, authorize_url, challenge_for, encode,
        parse_callback_query, refresh_request_body, sign_up_url, token_request_body, tokens_from,
        user_from,
    };

    const ISSUER: &str = "https://auth.fasttrackstudio.app";
    const REDIRECT: &str = "https://keyflow.fasttrackstudio.app/auth/callback";

    fn pkce() -> Pkce {
        Pkce::from_entropy([7u8; 32], [9u8; 16])
    }

    /// The RFC 7636 appendix B vector. If this drifts, every exchange
    /// fails with a refusal that names nothing.
    #[test]
    fn the_challenge_matches_the_rfc_vector() {
        assert_eq!(
            challenge_for("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    /// base64url, not base64. `+` and `/` would be re-encoded in the
    /// query string and reach the issuer as something else.
    #[test]
    fn nothing_produces_a_non_url_safe_challenge() {
        for seed in ["a", "ab", "abc", "hello world", "////++++", ""] {
            let challenge = challenge_for(seed);
            assert!(
                !challenge.contains(['+', '/', '=']),
                "{seed:?} produced {challenge}"
            );
        }
    }

    /// 32 bytes base64url'd is 43 characters — the RFC's minimum.
    #[test]
    fn the_verifier_is_long_enough_to_be_legal() {
        let p = pkce();
        assert_eq!(p.verifier().len(), 43);
        assert!(!p.verifier().contains(['+', '/', '=']));
    }

    /// Resuming has to derive the SAME challenge, or the redemption
    /// presents a verifier that does not match what was sent.
    #[test]
    fn resuming_reproduces_the_challenge() {
        let started = pkce();
        let resumed = Pkce::resume(started.verifier(), started.state());
        assert_eq!(started.challenge(), resumed.challenge());
        assert_eq!(started, resumed);
    }

    #[test]
    fn state_is_checked_both_ways() {
        let p = pkce();
        assert!(p.check_state(p.state()).is_ok());
        assert_eq!(
            p.check_state("something else"),
            Err(OidcError::StateMismatch)
        );
        assert_eq!(p.check_state(""), Err(OidcError::StateMismatch));
    }

    #[test]
    fn the_authorize_url_carries_every_required_parameter() {
        let url = authorize_url(ISSUER, REDIRECT, &pkce());
        for required in [
            "client_id=keyflow",
            "response_type=code",
            "code_challenge_method=S256",
        ] {
            assert!(url.contains(required), "{required} missing from {url}");
        }
        // The redirect must be escaped or its :// reads as structure.
        assert!(
            url.contains(
                "redirect_uri=https%3A%2F%2Fkeyflow.fasttrackstudio.app%2Fauth%2Fcallback"
            )
        );
    }

    /// The two scopes that are not boilerplate. `offline_access` is what
    /// mints the refresh token; without it the site is back to a session
    /// that lapses in an hour, which is the thing this flow replaced.
    #[test]
    fn the_scope_asks_for_a_refresh_token() {
        assert!(SCOPE.contains("offline_access"), "{SCOPE}");
        assert!(SCOPE.contains("forge:github"), "{SCOPE}");
        let url = authorize_url(ISSUER, REDIRECT, &pkce());
        assert!(
            url.contains("scope=openid%20email%20profile%20offline_access%20forge%3Agithub"),
            "{url}"
        );
    }

    /// The client id is what the issuer matches against its registered
    /// `oidcClients`; a rename here is a sign-in that fails at authorize
    /// with no indication which side is wrong.
    #[test]
    fn keyflow_is_registered_under_this_client_id() {
        assert_eq!(CLIENT_ID, "keyflow");
    }

    /// A trailing slash would make the path `//oauth2/authorize`, which
    /// some gateways route differently and none route better.
    #[test]
    fn a_trailing_slash_on_the_issuer_does_not_double_up() {
        let url = authorize_url("https://a.test/", "https://b.test/cb", &pkce());
        assert!(url.starts_with("https://a.test/oauth2/authorize?"), "{url}");
    }

    /// Sign-up lands on the hosted form and comes back through the same
    /// authorize URL, so creating an account signs you in.
    ///
    /// This test used to assert `redirect_to=<absolute URL>`, and so
    /// pinned a sign-up that could never come back: the issuer reads
    /// `return_to`, and accepts only a same-origin path. It is asserted
    /// here in the shape the issuer actually honours — see
    /// [`super::sign_up_url`].
    #[test]
    fn sign_up_returns_through_authorize() {
        let url = sign_up_url(ISSUER, REDIRECT, &pkce());
        assert!(
            url.starts_with("https://auth.fasttrackstudio.app/sign-up?"),
            "{url}"
        );
        assert!(url.contains("return_to=%2Foauth2%2Fauthorize"), "{url}");
    }

    #[test]
    fn the_token_body_sends_the_verifier_and_grant_type() {
        let p = pkce();
        let body = token_request_body("https://b.test/cb", "the+code", &p);
        assert!(body.contains("grant_type=authorization_code"));
        assert!(body.contains(&format!("code_verifier={}", p.verifier())));
        // `+` in a code decodes to a space in a form body, so an
        // unescaped one redeems a different code than the app holds.
        assert!(body.contains("code=the%2Bcode"), "got {body}");
    }

    /// The refresh grant carries no verifier and no redirect — the
    /// verifier belonged to a code that has been spent.
    #[test]
    fn the_refresh_body_is_the_token_and_nothing_else() {
        let body = refresh_request_body("r/t+ok");
        assert!(body.contains("grant_type=refresh_token"));
        assert!(body.contains("refresh_token=r%2Ft%2Bok"), "got {body}");
        assert!(!body.contains("code_verifier"));
        assert!(!body.contains("redirect_uri"));
    }

    #[test]
    fn unreserved_characters_survive_encoding_untouched() {
        assert_eq!(encode("aZ09-_.~"), "aZ09-_.~");
        assert_eq!(encode("a b"), "a%20b");
        assert_eq!(encode("&=?/#"), "%26%3D%3F%2F%23");
    }

    #[test]
    fn the_tokens_are_read_out() {
        let tokens = tokens_from(
            r#"{"access_token":"tok","refresh_token":"ref","expires_in":3600,"token_type":"Bearer"}"#,
        )
        .unwrap();
        assert_eq!(tokens.access_token, "tok");
        assert_eq!(tokens.refresh_token.as_deref(), Some("ref"));
        assert_eq!(tokens.expires_in, Some(3600));
    }

    /// A refresh response is allowed to omit the refresh token and mean
    /// "keep the one you have"; that must not read as an error.
    #[test]
    fn a_response_without_a_refresh_token_still_signs_in() {
        let tokens = tokens_from(r#"{"access_token":"tok"}"#).unwrap();
        assert_eq!(tokens.refresh_token, None);
        assert_eq!(tokens.expires_in, None);
    }

    /// A response that parses but carries nothing usable must fail, not
    /// hand back an empty token that looks like a session.
    #[test]
    fn a_response_without_a_usable_token_is_an_error() {
        for body in [
            r#"{"error":"invalid_grant"}"#,
            r#"{"access_token":""}"#,
            r#"{"access_token":"   "}"#,
            "{}",
        ] {
            assert!(tokens_from(body).is_err(), "{body} should not pass");
        }
        assert!(matches!(
            tokens_from("not json"),
            Err(OidcError::Malformed(_))
        ));
    }

    /// A person with no display name still signs in.
    #[test]
    fn userinfo_needs_only_a_subject() {
        let user = user_from(r#"{"sub":"abc-123"}"#).unwrap();
        assert_eq!(user.sub, "abc-123");
        assert_eq!(user.email, None);
        assert_eq!(user.name, None);

        let full = user_from(r#"{"sub":"a","email":"e@x.test","name":"Nom"}"#).unwrap();
        assert_eq!(full.email.as_deref(), Some("e@x.test"));
        assert_eq!(full.name.as_deref(), Some("Nom"));

        assert!(matches!(
            user_from(r#"{"email":"e@x.test"}"#),
            Err(OidcError::Missing("sub"))
        ));
    }

    #[test]
    fn a_callback_yields_its_code_and_state() {
        let (code, state) =
            parse_callback_query("?code=abc%2F1&state=xyz&iss=https%3A%2F%2Fa.test").unwrap();
        assert_eq!(code, "abc/1");
        assert_eq!(state, "xyz");
    }

    /// A refusal is the issuer's word, kept — "it didn't work" is not
    /// something a person can act on.
    #[test]
    fn a_refusal_carries_the_issuers_reason() {
        assert_eq!(
            parse_callback_query("error=access_denied&state=xyz"),
            Err(OidcError::Malformed("access_denied".to_owned()))
        );
    }

    /// A callback with no `state` is refused rather than trusted: the
    /// state check is the whole defence against a forged callback, and
    /// "absent" must not be a way around it.
    #[test]
    fn a_callback_without_a_state_is_not_a_sign_in() {
        assert_eq!(
            parse_callback_query("code=abc"),
            Err(OidcError::Missing("state"))
        );
        assert_eq!(
            parse_callback_query("state=xyz"),
            Err(OidcError::Missing("code"))
        );
        assert_eq!(parse_callback_query(""), Err(OidcError::Missing("code")));
    }
}

#[cfg(test)]
mod sign_up_return_to_tests {
    use super::{Pkce, sign_up_url};

    const ISSUER: &str = "https://auth.fasttrackstudio.app";
    const REDIRECT: &str = "https://keyflow.fasttrackstudio.app/auth/callback";

    fn pkce() -> Pkce {
        Pkce::from_entropy([7u8; 32], [9u8; 16])
    }

    /// The issuer reads `return_to` and nothing else.
    ///
    /// `redirect_to` was not a redirect to the wrong place — it was a
    /// parameter the issuer never looks at, so signing up succeeded and
    /// then dropped the person on the issuer's own page with no code
    /// and no way back.
    #[test]
    fn the_parameter_is_return_to() {
        let url = sign_up_url(ISSUER, REDIRECT, &pkce());
        assert!(url.contains("return_to="), "{url}");
        assert!(!url.contains("redirect_to="), "{url}");
    }

    /// And its value is a same-origin PATH. The issuer refuses anything
    /// that does not start with a single `/` — that refusal is what
    /// stops an open redirect — and silently falls back to its own
    /// default page, which is the same failure by a different route.
    #[test]
    fn the_destination_is_a_path_not_an_absolute_url() {
        let url = sign_up_url(ISSUER, REDIRECT, &pkce());
        let value = url
            .split_once("return_to=")
            .expect("a return_to parameter")
            .1;
        // Encoded, a path begins `%2Foauth2` and an absolute URL would
        // begin `https%3A%2F%2F`.
        assert!(value.starts_with("%2Foauth2%2Fauthorize"), "{value}");
        assert!(!value.contains("https%3A"), "absolute URL in {value}");
    }

    /// The authorize request still has to survive the round trip whole:
    /// a sign-up that comes back without PKCE or state cannot complete.
    #[test]
    fn the_authorize_request_survives_the_detour() {
        let url = sign_up_url(ISSUER, REDIRECT, &pkce());
        for part in [
            "client_id%3Dkeyflow",
            "code_challenge_method%3DS256",
            "state%3D",
            "response_type%3Dcode",
        ] {
            assert!(url.contains(part), "{part} missing from {url}");
        }
    }
}
