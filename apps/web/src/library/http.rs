//! `window.fetch`, and nothing else.
//!
//! Two verbs, shared by [`super::discovery`] (a GET) and
//! [`super::mcp`] (a POST). Both send the signed-in person's bearer
//! token, and neither knows what is in the body.
//!
//! # Why `fetch` and not an HTTP client
//!
//! For exactly the reason [`crate::auth`] uses it: this crate carries no
//! HTTP client crate and cannot afford one. reqwest 0.12 arrived here
//! once, with `auth-http`, alongside the reqwest 0.13 that iroh already
//! drags in through architect — and two reqwest majors in one wasm
//! binary is a hundred `duplicate symbol: intounderlyingsource_*` errors
//! out of rust-lld, because each compiles its own copy of the
//! wasm-streams glue. `apps/web/Cargo.toml` says so at length.
//!
//! Note that this constraint outlives the MCP transport above it. When
//! the architect pin moves and the chart calls become vox, discovery is
//! still a plain GET on a well-known path, and this is still how it is
//! sent.
//!
//! # The signatures are the same on both targets
//!
//! The host build gets stubs that fail. The site is checked for the host
//! as well as for wasm (CLAUDE.md), and a `cfg` on every call site would
//! be a second copy of the calling code to keep in step with the first.

use super::LibraryError;

/// POST a body, with a bearer token and a JSON content type.
///
/// # Errors
///
/// [`LibraryError::Transport`] if the request never got an answer, or
/// [`LibraryError::Refused`] carrying the body of a non-2xx response.
#[cfg(target_arch = "wasm32")]
pub async fn post_json(url: &str, body: &str, token: &str) -> Result<String, LibraryError> {
    let headers = web_sys::Headers::new().map_err(js)?;
    headers
        .set("content-type", "application/json")
        .map_err(js)?;
    // MCP's streamable-HTTP transport lets a server answer either as
    // JSON or as an event stream; Task always answers JSON, and saying
    // we accept both is what a compliant server expects to see.
    headers
        .set("accept", "application/json, text/event-stream")
        .map_err(js)?;
    headers
        .set("authorization", &format!("Bearer {token}"))
        .map_err(js)?;
    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    init.set_headers(&headers);
    init.set_body(&wasm_bindgen::JsValue::from_str(body));
    send(web_sys::Request::new_with_str_and_init(url, &init).map_err(js)?).await
}

/// GET, with a bearer token.
///
/// # Errors
///
/// As [`post_json`].
#[cfg(target_arch = "wasm32")]
pub async fn get(url: &str, token: &str) -> Result<String, LibraryError> {
    let headers = web_sys::Headers::new().map_err(js)?;
    headers
        .set("authorization", &format!("Bearer {token}"))
        .map_err(js)?;
    let init = web_sys::RequestInit::new();
    init.set_headers(&headers);
    send(web_sys::Request::new_with_str_and_init(url, &init).map_err(js)?).await
}

/// Send, keeping the body of a refusal.
///
/// A non-2xx answer keeps its body on purpose: an MCP tool error arrives
/// as a 200 with a JSON-RPC `error` inside it, but a proxy, a CORS
/// refusal or a dead deployment does not, and "502" alone explains
/// nothing to the person reading it.
#[cfg(target_arch = "wasm32")]
async fn send(request: web_sys::Request) -> Result<String, LibraryError> {
    use wasm_bindgen::JsCast as _;
    use wasm_bindgen_futures::JsFuture;

    let window =
        web_sys::window().ok_or_else(|| LibraryError::Transport("no browser window".to_owned()))?;
    let value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(js)?;
    let response: web_sys::Response = value
        .dyn_into()
        .map_err(|_| LibraryError::Transport("fetch returned a non-Response".to_owned()))?;
    let status = response.status();
    let text = JsFuture::from(response.text().map_err(js)?)
        .await
        .map_err(js)?
        .as_string()
        .unwrap_or_default();
    if (200..300).contains(&status) {
        Ok(text)
    } else {
        Err(LibraryError::Refused(format!("{status}: {text}")))
    }
}

#[cfg(target_arch = "wasm32")]
fn js(error: wasm_bindgen::JsValue) -> LibraryError {
    LibraryError::Transport(format!("{error:?}"))
}

/// There is no browser on the host, and no session to present to one.
#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::unused_async)]
pub async fn post_json(_url: &str, _body: &str, _token: &str) -> Result<String, LibraryError> {
    Err(LibraryError::Transport("not in a browser".to_owned()))
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::unused_async)]
pub async fn get(_url: &str, _token: &str) -> Result<String, LibraryError> {
    Err(LibraryError::Transport("not in a browser".to_owned()))
}
