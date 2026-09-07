//! The chart calls, spoken as MCP over HTTP.
//!
//! **This is the replaceable half, and it is the only file in the site
//! that knows MCP exists.** Nothing above it mentions a JSON-RPC id, a
//! `tools/call` envelope or a content block: [`super`] asks for a chart
//! and gets a [`super::StoredChart`], and the screens above that ask
//! [`super`]. When the transport changes, this file is what changes —
//! ideally, what is deleted.
//!
//! Read the module docs on [`super`] for *why* the transport is this
//! one, why it is temporary, and what replaces it.
//!
//! # The two layers, and why both have to be read
//!
//! An MCP tool call is a JSON-RPC 2.0 request whose `params` name a tool
//! and carry its arguments — not a REST body with the arguments at the
//! top. Posting the arguments bare gets `missing tool name`, which reads
//! like a bug in the arguments rather than in the framing.
//!
//! The answer is layered the same way, and each layer fails
//! differently:
//!
//! 1. A top-level JSON-RPC `error` means the request never reached a
//!    tool — an unknown method, a bad envelope, no org for the token.
//! 2. A tool that *ran* and failed answers a **successful** JSON-RPC
//!    response carrying `isError: true` and a human-readable message.
//!    This is MCP's convention, and a client that reads only the HTTP
//!    status or only the JSON-RPC `error` reports every one of those
//!    failures as a success.
//! 3. The tool's real answer is a JSON document *serialized into*
//!    `content[0].text`, because MCP content blocks carry text for a
//!    model to read. So the body is parsed twice, on purpose.
//!
//! All of that is why the parsing here is one tested function rather
//! than a chain of `?` at four call sites.
//!
//! # Tolerance about the payload shape
//!
//! The chart tools are new and this client was written against their
//! contract rather than against a running server. Where the contract is
//! ambiguous — a listing as a bare array or as `{"charts": [...]}`,
//! matching Task's other listing tools — both are accepted. The failure
//! this avoids is the quiet one: an empty library shown to someone whose
//! charts are plainly there.

use serde_json::{Value, json};

use super::{
    ChartEntry, Draft, LibraryError, SaveOutcome, StoredChart, http, mcp_url, task_base_url,
};

// ── Building requests ────────────────────────────────────────────────

/// The JSON-RPC id every request carries.
///
/// A constant, not a counter. Each `fetch` is its own request and its
/// own response — there is no multiplexed connection here to correlate
/// across — and a counter would be state this module otherwise does not
/// need, plus a reason the pure functions could not be tested by
/// comparing strings.
const RPC_ID: i64 = 1;

/// Wrap a tool call in MCP's `tools/call` envelope. See the module docs.
#[must_use]
pub fn call_body(tool: &str, arguments: Value) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": RPC_ID,
        "method": "tools/call",
        "params": { "name": tool, "arguments": arguments },
    })
    .to_string()
}

/// Add `org` to a tool's arguments when there is one to add.
///
/// Omitted rather than sent as `null`: on the account lane the server
/// reads an absent `org` as "use the caller's default", and a `null`
/// would have to be special-cased there to mean the same thing.
fn with_org(mut args: Value, org: Option<&str>) -> Value {
    if let (Some(slug), Some(map)) = (org, args.as_object_mut()) {
        map.insert("org".to_owned(), Value::String(slug.to_owned()));
    }
    args
}

#[must_use]
pub fn list_charts_body(org: Option<&str>) -> String {
    call_body("list_charts", with_org(json!({}), org))
}

#[must_use]
pub fn read_chart_body(slug: &str, org: Option<&str>) -> String {
    call_body("read_chart", with_org(json!({ "slug": slug }), org))
}

#[must_use]
pub fn delete_chart_body(slug: &str, org: Option<&str>) -> String {
    call_body("delete_chart", with_org(json!({ "slug": slug }), org))
}

/// The `save_chart` call.
///
/// Optional fields are omitted when empty rather than sent as `null` or
/// `""`. An empty `slug` in particular would defeat the derive-from-
/// title behaviour that makes re-saving a chart a new *version* instead
/// of a second chart called the same thing.
#[must_use]
pub fn save_chart_body(draft: &Draft) -> String {
    let mut args = json!({ "title": draft.title, "source": draft.source });
    let map = args
        .as_object_mut()
        .expect("a json! object literal is an object");
    if let Some(key) = draft.key.as_ref().filter(|k| !k.trim().is_empty()) {
        map.insert("key".to_owned(), Value::String(key.clone()));
    }
    if !draft.sections.is_empty() {
        map.insert("sections".to_owned(), json!(draft.sections));
    }
    if let Some(slug) = draft.slug.as_ref().filter(|s| !s.trim().is_empty()) {
        map.insert("slug".to_owned(), Value::String(slug.clone()));
    }
    call_body("save_chart", with_org(args, draft.org.as_deref()))
}

// ── Reading answers ──────────────────────────────────────────────────

/// Unwrap an MCP tool response down to the JSON the tool returned. The
/// three layers, and what each can mean, are in the module docs.
///
/// # Errors
///
/// One of [`LibraryError`]'s server-side variants, per those layers.
pub fn tool_payload(body: &str) -> Result<Value, LibraryError> {
    let response: Value =
        serde_json::from_str(body).map_err(|e| LibraryError::Malformed(e.to_string()))?;

    if let Some(error) = response.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("no reason given")
            .to_owned();
        let code = error.get("code").and_then(Value::as_i64);
        // -32601 is JSON-RPC's METHOD_NOT_FOUND, which Task also uses
        // for a tool it does not have.
        if code == Some(-32601) || looks_like_missing_tool(&message) {
            return Err(LibraryError::Unsupported);
        }
        if looks_like_no_org(&message) {
            return Err(LibraryError::NoOrg);
        }
        return Err(LibraryError::Refused(message));
    }

    let result = response
        .get("result")
        .ok_or_else(|| LibraryError::Malformed("no `result` in the response".to_owned()))?;

    let text = result
        .get("content")
        .and_then(Value::as_array)
        .and_then(|blocks| blocks.first())
        .and_then(|block| block.get("text"))
        .and_then(Value::as_str)
        .unwrap_or_default();

    if result
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        // The message here is prose for a model to read, not JSON.
        if looks_like_missing_tool(text) {
            return Err(LibraryError::Unsupported);
        }
        if looks_like_no_org(text) {
            return Err(LibraryError::NoOrg);
        }
        return Err(LibraryError::Refused(text.to_owned()));
    }

    // A tool with nothing to say (`delete_chart`) is allowed an empty
    // block; that is success, not a malformed answer.
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(text).map_err(|e| LibraryError::Malformed(e.to_string()))
}

/// Whether a server's refusal is "I do not have that tool".
///
/// Matched on the message as well as the code because the same
/// condition arrives two ways: as JSON-RPC `-32601` from the dispatcher,
/// and as an `isError` tool result from the plugin gate. Both mean the
/// person's library is not on this deployment yet, and both should read
/// as that rather than as a failure they can do something about.
fn looks_like_missing_tool(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("unknown tool")
        || lower.contains("method not found")
        || lower.contains("no such tool")
}

fn looks_like_no_org(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("no reachable org") || lower.contains("not hosted")
}

/// Pull an array out of a payload that may or may not be wrapped.
/// Anything with exactly one array in it is that array; see the module
/// docs on tolerance.
fn array_in(payload: &Value, name: &str) -> Option<Vec<Value>> {
    if let Some(list) = payload.as_array() {
        return Some(list.clone());
    }
    let object = payload.as_object()?;
    if let Some(list) = object.get(name).and_then(Value::as_array) {
        return Some(list.clone());
    }
    let mut arrays = object.values().filter_map(Value::as_array);
    let only = arrays.next()?;
    arrays.next().is_none().then(|| only.clone())
}

/// Pull an object out of a payload that may or may not be wrapped in a
/// named field. Same tolerance, same reason as [`array_in`].
fn object_in<'a>(payload: &'a Value, name: &str) -> &'a Value {
    payload.get(name).unwrap_or(payload)
}

fn text_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

/// Read a listing.
///
/// # Errors
///
/// Whatever [`tool_payload`] found, or [`LibraryError::Malformed`] if
/// the payload holds no recognisable list.
pub fn charts_from(body: &str) -> Result<Vec<ChartEntry>, LibraryError> {
    let payload = tool_payload(body)?;
    let rows = array_in(&payload, "charts")
        .ok_or_else(|| LibraryError::Malformed("no chart list in the answer".to_owned()))?;
    Ok(rows
        .iter()
        .filter_map(|row| {
            let slug = text_field(row, "slug")?;
            Some(ChartEntry {
                // A chart with no title is shown by its slug rather
                // than as a blank row.
                title: text_field(row, "title").unwrap_or_else(|| slug.clone()),
                key: text_field(row, "key"),
                notation: text_field(row, "notation"),
                updated_at: text_field(row, "updated_at"),
                slug,
            })
        })
        .collect())
}

/// Read one chart back.
///
/// # Errors
///
/// Whatever [`tool_payload`] found, or [`LibraryError::Malformed`] if
/// there is no `source` — a chart with no text is not a chart, and
/// opening the editor on an empty buffer would look like the save had
/// silently lost it.
pub fn chart_from(body: &str) -> Result<StoredChart, LibraryError> {
    let payload = tool_payload(body)?;
    let chart = object_in(&payload, "chart");
    let source = chart
        .get("source")
        .and_then(Value::as_str)
        .ok_or_else(|| LibraryError::Malformed("the chart came back without its text".to_owned()))?
        // Not trimmed. The contract is byte-identical: leading blank
        // lines are the writer's, and so is the trailing newline.
        .to_owned();
    let slug = text_field(chart, "slug").unwrap_or_default();
    Ok(StoredChart {
        title: text_field(chart, "title").unwrap_or_else(|| slug.clone()),
        key: text_field(chart, "key"),
        notation: text_field(chart, "notation"),
        slug,
        source,
    })
}

/// Read the answer to a save.
///
/// # Errors
///
/// Whatever [`tool_payload`] found, or [`LibraryError::Malformed`] if
/// the server did not name the slug it wrote — without one there is
/// nothing to link to and no way to save over it next time.
pub fn save_outcome_from(body: &str) -> Result<SaveOutcome, LibraryError> {
    let payload = tool_payload(body)?;
    let saved = object_in(&payload, "chart");
    Ok(SaveOutcome {
        slug: text_field(saved, "slug")
            .ok_or_else(|| LibraryError::Malformed("the save named no chart".to_owned()))?,
        rel_path: text_field(saved, "rel_path"),
        created: saved
            .get("created")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// Read the answer to a delete. There is nothing to read — this is
/// [`tool_payload`]'s error handling and no payload.
///
/// # Errors
///
/// Whatever [`tool_payload`] found.
pub fn delete_ack_from(body: &str) -> Result<(), LibraryError> {
    tool_payload(body).map(|_| ())
}

// ── The four operations ──────────────────────────────────────────────
//
// Each is a body this module built, posted, and handed to a parser this
// module wrote. Everything that can be got wrong is above; these
// deliberately contain no decisions, because they are the part no host
// test can reach.

/// # Errors
///
/// [`LibraryError::SignedOut`], or whatever the server said.
pub async fn list_charts(org: Option<&str>) -> Result<Vec<ChartEntry>, LibraryError> {
    charts_from(&call(&list_charts_body(org)).await?)
}

/// # Errors
///
/// [`LibraryError::SignedOut`], or whatever the server said.
pub async fn read_chart(slug: &str, org: Option<&str>) -> Result<StoredChart, LibraryError> {
    chart_from(&call(&read_chart_body(slug, org)).await?)
}

/// # Errors
///
/// [`LibraryError::SignedOut`], or whatever the server said.
pub async fn save_chart(draft: &Draft) -> Result<SaveOutcome, LibraryError> {
    save_outcome_from(&call(&save_chart_body(draft)).await?)
}

/// # Errors
///
/// [`LibraryError::SignedOut`], or whatever the server said.
pub async fn delete_chart(slug: &str, org: Option<&str>) -> Result<(), LibraryError> {
    delete_ack_from(&call(&delete_chart_body(slug, org)).await?)
}

/// Post one JSON-RPC body to the MCP endpoint as the signed-in person.
async fn call(body: &str) -> Result<String, LibraryError> {
    let token = crate::auth::access_token()
        .await
        .ok_or(LibraryError::SignedOut)?;
    http::post_json(&mcp_url(&task_base_url()), body, &token).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The response shape the server actually sends: a JSON-RPC result
    /// whose `content[0].text` is the tool's JSON.
    fn tool_response(payload: Value) -> String {
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "content": [{ "type": "text", "text": payload.to_string() }],
                "isError": false,
            },
        })
        .to_string()
    }

    fn tool_failure(message: &str) -> String {
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "content": [{ "type": "text", "text": message }],
                "isError": true,
            },
        })
        .to_string()
    }

    /// The framing is the thing this module exists to get right. An MCP
    /// tool call is a JSON-RPC request with the arguments *nested*, not
    /// a REST body with the arguments at the top.
    #[test]
    fn a_tool_call_is_wrapped_in_the_json_rpc_envelope() {
        let body: Value = serde_json::from_str(&list_charts_body(Some("acme"))).unwrap();
        assert_eq!(body["jsonrpc"], "2.0");
        assert_eq!(body["method"], "tools/call");
        assert_eq!(body["params"]["name"], "list_charts");
        assert_eq!(body["params"]["arguments"]["org"], "acme");
        assert!(
            body.get("name").is_none(),
            "the tool name belongs under params, not at the top level"
        );
    }

    /// An absent org means "the caller's default org", which the server
    /// resolves. A `null` would have to be special-cased there to mean
    /// the same thing.
    #[test]
    fn no_org_means_no_org_field() {
        let body: Value = serde_json::from_str(&list_charts_body(None)).unwrap();
        assert_eq!(body["params"]["arguments"], json!({}));
    }

    /// Each tool is named exactly once, here, and a typo in one of them
    /// is a feature that silently does not exist.
    #[test]
    fn every_tool_is_called_by_the_name_the_server_registered() {
        let name = |body: &str| {
            serde_json::from_str::<Value>(body).unwrap()["params"]["name"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        assert_eq!(name(&list_charts_body(None)), "list_charts");
        assert_eq!(name(&read_chart_body("s", None)), "read_chart");
        assert_eq!(name(&delete_chart_body("s", None)), "delete_chart");
        assert_eq!(
            name(&save_chart_body(&Draft {
                title: "T".to_owned(),
                source: String::new(),
                key: None,
                sections: Vec::new(),
                slug: None,
                org: None,
            })),
            "save_chart"
        );
    }

    /// The chart text is what is being kept. It has to survive JSON
    /// encoding exactly — newlines, unicode, trailing blank line and
    /// all — or the library quietly returns a different document from
    /// the one that was saved.
    #[test]
    fn the_source_survives_the_envelope_byte_for_byte() {
        let source = "Café — 4/4 #G\n\nVS 1: | 1 4 |\n\t| 5 6m |\n\n";
        let draft = Draft {
            title: "Café".to_owned(),
            source: source.to_owned(),
            key: Some("G".to_owned()),
            sections: vec!["VS 1".to_owned()],
            slug: None,
            org: None,
        };
        let body: Value = serde_json::from_str(&save_chart_body(&draft)).unwrap();
        assert_eq!(body["params"]["arguments"]["source"].as_str(), Some(source));
    }

    /// Empty optionals are omitted, not sent blank. An empty `slug` in
    /// particular would stop the server deriving one from the title,
    /// which is what makes a re-save a new version instead of a second
    /// chart.
    #[test]
    fn empty_optionals_are_left_out_rather_than_sent_blank() {
        let draft = Draft {
            title: "T".to_owned(),
            source: "T\n".to_owned(),
            key: Some("  ".to_owned()),
            sections: Vec::new(),
            slug: Some(String::new()),
            org: None,
        };
        let sent: Value = serde_json::from_str(&save_chart_body(&draft)).unwrap();
        let args = sent["params"]["arguments"].clone();
        assert_eq!(args, json!({ "title": "T", "source": "T\n" }));
    }

    /// The payload is JSON *inside* a text content block — two parses,
    /// on purpose.
    #[test]
    fn a_tool_result_is_unwrapped_through_both_layers() {
        let body = tool_response(json!({ "charts": [] }));
        assert_eq!(tool_payload(&body).unwrap(), json!({ "charts": [] }));
    }

    /// MCP models tool failure as a *successful* response with
    /// `isError`. Reading only the HTTP status or only the JSON-RPC
    /// `error` reports those as successes.
    #[test]
    fn an_is_error_result_is_a_failure_and_not_an_empty_success() {
        assert_eq!(
            tool_payload(&tool_failure("that chart is not yours")),
            Err(LibraryError::Refused("that chart is not yours".to_owned()))
        );
    }

    /// The server may not carry the chart tools yet. That is its own
    /// state, not a generic refusal, because it is the difference
    /// between "not yet" and "no" — and it arrives BOTH ways.
    #[test]
    fn a_server_without_the_chart_tools_is_unsupported_however_it_says_so() {
        let by_code = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": { "code": -32601, "message": "unknown tool `save_chart`" },
        })
        .to_string();
        assert_eq!(tool_payload(&by_code), Err(LibraryError::Unsupported));
        assert_eq!(
            tool_payload(&tool_failure("unknown tool `save_chart`")),
            Err(LibraryError::Unsupported)
        );
    }

    /// An account with nowhere to write says so plainly rather than
    /// reading as a server error.
    #[test]
    fn no_reachable_org_is_its_own_state() {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": { "code": -32600, "message": "no reachable org for this token" },
        })
        .to_string();
        assert_eq!(tool_payload(&body), Err(LibraryError::NoOrg));
    }

    #[test]
    fn junk_is_malformed_rather_than_a_panic() {
        assert!(matches!(
            tool_payload("<html>502 Bad Gateway</html>"),
            Err(LibraryError::Malformed(_))
        ));
        assert!(matches!(
            charts_from(&tool_response(json!("not an object"))),
            Err(LibraryError::Malformed(_))
        ));
    }

    /// Whether the listing comes back bare or wrapped is not worth a
    /// version skew that shows an empty library to someone whose charts
    /// are plainly there.
    #[test]
    fn a_listing_reads_wrapped_or_bare() {
        let row = json!({
            "slug": "build-my-life",
            "title": "Build My Life",
            "key": "G",
            "notation": "numbers",
            "updated_at": "2026-09-06T12:00:00Z",
        });
        let expected = vec![ChartEntry {
            slug: "build-my-life".to_owned(),
            title: "Build My Life".to_owned(),
            key: Some("G".to_owned()),
            notation: Some("numbers".to_owned()),
            updated_at: Some("2026-09-06T12:00:00Z".to_owned()),
        }];
        for payload in [
            json!([row.clone()]),
            json!({ "charts": [row.clone()] }),
            json!({ "count": 1, "charts": [row] }),
        ] {
            assert_eq!(charts_from(&tool_response(payload)).unwrap(), expected);
        }
    }

    /// A chart with no title is listed by its slug, not as a blank row.
    #[test]
    fn an_untitled_row_still_shows_something() {
        let body = tool_response(json!({ "charts": [{ "slug": "untitled-chart" }] }));
        let charts = charts_from(&body).unwrap();
        assert_eq!(charts[0].title, "untitled-chart");
        assert_eq!(charts[0].key, None);
    }

    /// The whole promise of the library: what comes back is what went
    /// in.
    #[test]
    fn a_chart_reads_back_byte_identical() {
        let source = "Build My Life - Housefires\n4/4 #G\n\nVS 1: | 1 4 | 5 6m |\n";
        let body = tool_response(json!({
            "slug": "build-my-life",
            "title": "Build My Life",
            "source": source,
        }));
        let chart = chart_from(&body).unwrap();
        assert_eq!(chart.source, source);
        assert_eq!(chart.slug, "build-my-life");
    }

    /// A chart with no text is not a chart. Opening the editor on an
    /// empty buffer would look exactly like a save that silently lost
    /// the document.
    #[test]
    fn a_chart_without_text_is_malformed_not_empty() {
        let body = tool_response(json!({ "slug": "s", "title": "T" }));
        assert!(matches!(chart_from(&body), Err(LibraryError::Malformed(_))));
    }

    #[test]
    fn a_save_reports_the_slug_and_whether_it_was_new() {
        let body = tool_response(json!({
            "slug": "build-my-life",
            "rel_path": "Charts/build-my-life.kf",
            "created": true,
        }));
        assert_eq!(
            save_outcome_from(&body).unwrap(),
            SaveOutcome {
                slug: "build-my-life".to_owned(),
                rel_path: Some("Charts/build-my-life.kf".to_owned()),
                created: true,
            }
        );
    }

    /// A delete has nothing to say, and an empty content block is
    /// success rather than an unreadable answer.
    #[test]
    fn a_delete_with_no_payload_is_still_a_success() {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "content": [{ "type": "text", "text": "" }], "isError": false },
        })
        .to_string();
        assert_eq!(delete_ack_from(&body), Ok(()));
    }
}
