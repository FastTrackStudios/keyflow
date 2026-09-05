//! The guide, re-rendered on save instead of on rebuild.
//!
//! The published guide is baked into the binary: `build.rs` renders every
//! note at compile time and `ssg::include_vault!()` includes the table.
//! That is what makes a published chapter arrive as finished HTML with no
//! markdown pass in the browser, and it is not something to give up.
//!
//! It also means editing one word of prose costs a full rebuild of this
//! crate — four to five minutes. This module is the dev-only escape: the
//! server keeps a *live* vault rendered from `docs/guides/keyflow` at
//! runtime, a filesystem watcher re-renders it on save, and the browser
//! polls for changes and swaps the new pages in. Editing a chapter
//! becomes a save and a second.
//!
//! Off unless the `dev-guide` feature is on, and the baked vault is what
//! every other build sees.
//!
//! ## Why the browser fetches by hand
//!
//! This crate deliberately does not enable `dioxus/fullstack` for wasm —
//! its server-function *client* is reqwest 0.12, the tree already has
//! 0.13, and two reqwest majors in one wasm binary is a wall of duplicate
//! symbols out of rust-lld. See the `web` feature in `Cargo.toml`.
//!
//! A server function is still an ordinary HTTP endpoint, though. So the
//! browser calls it with `fetch` through `web-sys` — which is already a
//! dependency — and parses the JSON itself. No client transport, no
//! second reqwest.

use ssg::{StaticHeading, StaticPage, StaticVault};

/// One rendered note, over the wire.
///
/// Mirrors [`StaticPage`] with owned strings. The baked table cannot be
/// sent as-is: its fields are `&'static str` pointing into the binary.
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct LivePage {
    pub slug: String,
    pub title: String,
    pub summary: String,
    pub order: u32,
    pub stage: String,
    pub kind: String,
    pub source: String,
    pub body: String,
    pub html: String,
    pub links: Vec<String>,
    pub headings: Vec<LiveHeading>,
    pub tags: Vec<String>,
    pub words: u32,
    pub updated: String,
}

/// One heading, over the wire. Mirrors [`StaticHeading`].
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct LiveHeading {
    pub level: u8,
    pub text: String,
    pub id: String,
}

/// A whole vault, over the wire, with the revision it was rendered at.
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct LiveVault {
    pub revision: u64,
    pub pages: Vec<LivePage>,
    /// `@font-face` rules for the typefaces these charts draw, subset to
    /// the glyphs they use.
    ///
    /// Carried with the pages rather than fetched separately because it
    /// belongs to *this* render: a chapter that gains a chart using a
    /// symbol no other chart uses needs a face the last build never
    /// subsetted for. The baked stylesheet cannot know about it, and a
    /// glyph that silently fails to draw is exactly the kind of
    /// difference a preview is supposed to catch.
    pub font_css: String,
}

/// Turn a wire vault into the `&'static StaticVault` the whole site is
/// written against.
///
/// This leaks, deliberately. Every consumer — `VaultArticle`, the table
/// of contents, the graph — takes `&'static`, and threading a lifetime
/// through all of them to support a dev-only feature would be a large
/// change to shipped code for no benefit to a reader. A vault is a few
/// hundred kilobytes and a save leaks one; a long editing session costs a
/// few megabytes in a process that exists to be restarted.
#[must_use]
pub fn leak(vault: LiveVault) -> &'static StaticVault {
    fn s(v: String) -> &'static str {
        Box::leak(v.into_boxed_str())
    }
    let pages: Vec<StaticPage> = vault
        .pages
        .into_iter()
        .map(|p| StaticPage {
            slug: s(p.slug),
            title: s(p.title),
            summary: s(p.summary),
            order: p.order,
            stage: s(p.stage),
            kind: s(p.kind),
            source: s(p.source),
            body: s(p.body),
            html: s(p.html),
            links: Box::leak(
                p.links
                    .into_iter()
                    .map(s)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ),
            headings: Box::leak(
                p.headings
                    .into_iter()
                    .map(|h| StaticHeading {
                        level: h.level,
                        text: s(h.text),
                        id: s(h.id),
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ),
            tags: Box::leak(
                p.tags
                    .into_iter()
                    .map(s)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ),
            words: p.words,
            updated: s(p.updated),
        })
        .collect();
    Box::leak(Box::new(StaticVault::new(Box::leak(
        pages.into_boxed_slice(),
    ))))
}

// ── The server's live vault ─────────────────────────────────────────

#[cfg(all(feature = "dev-guide", feature = "server"))]
mod server {
    use super::{LiveHeading, LivePage, LiveVault};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{OnceLock, RwLock};

    /// Where the notes live, relative to this crate.
    ///
    /// The same path `build.rs` reads. A dev server that rendered a
    /// different directory from the build would be previewing something
    /// that never ships.
    const VAULT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/guides/keyflow");

    /// The last render, and a counter the browser can compare against.
    ///
    /// The counter is what makes polling cheap: the browser asks for a
    /// number, and only asks for the pages when the number moved.
    static LIVE: OnceLock<RwLock<LiveVault>> = OnceLock::new();
    static REVISION: AtomicU64 = AtomicU64::new(0);
    /// Keeps the watcher alive. `notify` stops watching when its handle
    /// drops, and a watch nobody holds silently does nothing.
    static WATCH: OnceLock<ssg_build::watch::Watch> = OnceLock::new();

    /// The chart renderer for the dev preview.
    ///
    /// `editor_keyflow::Fences` — what `main` registers for the editor —
    /// embeds the typefaces in every chart it draws, because a chart in
    /// the editor has to be self-contained. A *page* of charts must not:
    /// the guide links one subsetted stylesheet and every chart on it
    /// resolves against that, which is what `build.rs` does and why the
    /// published site ships charts without shipping the faces sixty
    /// times over.
    ///
    /// Rendering the guide through the embedding variant produced a
    /// hundred megabytes of HTML for six chapters — 485 KB of typeface
    /// per chart, exactly the trap the repo instructions warn about. This
    /// is the linked variant, which is also what ships.
    struct LinkedCharts {
        /// What the charts drew, for subsetting afterwards.
        used: std::sync::Arc<editor_keyflow::font_subset::FontUsage>,
    }

    impl editor_state::fence_renderer::FenceRenderer for LinkedCharts {
        fn render_svg(&self, source: &str) -> Option<String> {
            let svg = editor_keyflow::render_svg_live(source).ok()?;
            // Read the families back out of the finished SVG, the same
            // way `build.rs` does — the serializer decides which family
            // a run of text lands in, and it is the authority.
            self.used.record(&svg);
            Some(svg)
        }

        fn highlight_html(&self, source: &str) -> String {
            editor_keyflow::highlight_html(source)
        }
    }

    /// Render the vault the way `build.rs` does.
    ///
    /// The body renderer is the editor's markdown pass, and `kf` fences
    /// go through the editor's fence registry — so a chapter previewed
    /// here is the chapter that ships, callouts, charts and all.
    fn render() -> LiveVault {
        // Replace the editor's self-contained chart renderer with the
        // linked one for the rest of this process. Global, because the
        // registry is: the dev server also serves the workbench, which
        // will draw its charts against the guide's font stylesheet
        // instead of its own copies. In a dev build that is a cosmetic
        // difference; the hundred megabytes was not.
        let used = std::sync::Arc::new(editor_keyflow::font_subset::FontUsage::new());
        editor_state::fence_renderer::register_fence_renderer(
            "kf",
            std::sync::Arc::new(LinkedCharts {
                used: std::sync::Arc::clone(&used),
            }),
        );
        let vault = ssg_build::Vault::at(VAULT_DIR)
            .link_base(crate::guide::BASE)
            .body_renderer(|markdown| editor_state::html::render_markdown_html(markdown))
            .allow_broken_links()
            .render();
        let pages = vault
            .pages
            .into_iter()
            .map(|p| LivePage {
                slug: p.slug,
                title: p.title,
                summary: p.summary,
                order: p.order,
                stage: p.stage,
                kind: p.kind,
                source: p.source,
                body: p.body,
                html: p.html,
                links: p.links,
                headings: p
                    .headings
                    .into_iter()
                    .map(|h| LiveHeading {
                        level: h.level,
                        text: h.text,
                        id: h.id,
                    })
                    .collect(),
                tags: p.tags,
                words: p.words,
                updated: p.updated,
            })
            .collect();
        // After the render, so it covers every chart on every page.
        let font_css = used.font_face_css().unwrap_or_default();
        LiveVault {
            revision: REVISION.load(Ordering::Relaxed),
            pages,
            font_css,
        }
    }

    /// Render once and start watching. Idempotent.
    fn ensure_started() -> &'static RwLock<LiveVault> {
        let live = LIVE.get_or_init(|| RwLock::new(render()));
        WATCH.get_or_init(|| {
            ssg_build::watch::on_change(VAULT_DIR, || {
                // A render failure must not take the dev server with it:
                // a half-typed `[[wikilink]]` is a normal state for a
                // file being edited, and the preview should keep showing
                // the last good version until the note parses again.
                let rendered = std::panic::catch_unwind(render).map(|mut v| {
                    v.revision = REVISION.fetch_add(1, Ordering::Relaxed) + 1;
                    v
                });
                match rendered {
                    Ok(v) => {
                        if let Ok(mut slot) = LIVE
                            .get()
                            .expect("the vault is rendered before the watcher starts")
                            .write()
                        {
                            tracing::info!(revision = v.revision, "guide re-rendered");
                            *slot = v;
                        }
                    }
                    Err(_) => tracing::warn!("guide re-render failed; keeping the last good one"),
                }
            })
            .unwrap_or_else(|e| panic!("cannot watch {VAULT_DIR}: {e}"))
        });
        live
    }

    /// The current revision.
    pub fn revision() -> u64 {
        ensure_started();
        REVISION.load(Ordering::Relaxed)
    }

    /// The current vault.
    pub fn snapshot() -> LiveVault {
        ensure_started()
            .read()
            .map(|v| v.clone())
            .unwrap_or_else(|e| e.into_inner().clone())
    }
}

// The `#[server]` macro expands to paths that assume the fullstack
// prelude is in scope, the way `main.rs` has it.
#[cfg(all(feature = "dev-guide", feature = "server"))]
use dioxus::prelude::*;

/// The guide's current revision, as a number the browser can compare.
///
/// Polled rather than pushed. A push would want a WebSocket, and this
/// crate cannot carry the server-function client that would ride on —
/// see the module docs. A `u64` every half second in a dev server is not
/// worth a protocol.
#[cfg(all(feature = "dev-guide", feature = "server"))]
#[server(endpoint = "guide_revision")]
pub async fn guide_revision() -> ServerFnResult<u64> {
    Ok(server::revision())
}

/// The whole guide as it stands on disk.
#[cfg(all(feature = "dev-guide", feature = "server"))]
#[server(endpoint = "guide_snapshot")]
pub async fn guide_snapshot() -> ServerFnResult<LiveVault> {
    Ok(server::snapshot())
}

// ── The browser's half ──────────────────────────────────────────────

/// The live vault, once the browser has fetched one.
///
/// `None` until the first poll answers, and every read falls back to the
/// baked table until then — so the first paint is the ordinary
/// server-rendered page and nothing waits on the network.
#[cfg(feature = "dev-guide")]
pub static LIVE_VAULT: dioxus::prelude::GlobalSignal<Option<&'static StaticVault>> =
    dioxus::prelude::Signal::global(|| None);

/// The `@font-face` rules for the live render, once one has arrived.
///
/// Read by the guide page, which links the baked stylesheet until this
/// is populated and then declares these instead.
#[cfg(feature = "dev-guide")]
pub static LIVE_FONT_CSS: dioxus::prelude::GlobalSignal<Option<String>> =
    dioxus::prelude::Signal::global(|| None);

/// How often the browser asks whether the guide changed.
///
/// Fast enough that a save feels immediate, slow enough that the request
/// log stays readable. Only the revision is fetched at this rate; the
/// pages follow only when it moved.
#[cfg(all(feature = "dev-guide", target_arch = "wasm32"))]
const POLL: std::time::Duration = std::time::Duration::from_millis(400);

/// Poll the dev server for guide changes, forever.
///
/// Spawned once from `App`. Calls the server functions by URL with
/// `fetch` rather than through a generated client — see the module docs
/// for why this crate has no server-function client in wasm.
#[cfg(all(feature = "dev-guide", target_arch = "wasm32"))]
pub fn start_polling() {
    use dioxus::prelude::*;

    spawn(async move {
        let mut seen = u64::MAX;
        loop {
            gloo_timers::future::sleep(POLL).await;
            let Some(revision) = post_json::<u64>("/api/guide_revision").await else {
                continue;
            };
            if revision == seen {
                continue;
            }
            let Some(vault) = post_json::<LiveVault>("/api/guide_snapshot").await else {
                continue;
            };
            seen = revision;
            tracing::info!(revision, "guide updated");
            // Fonts before pages: the charts in the new pages reference
            // families by name, and a face that arrives second is a
            // frame of blank glyphs.
            if !vault.font_css.is_empty() {
                *LIVE_FONT_CSS.write() = Some(vault.font_css.clone());
            }
            *LIVE_VAULT.write() = Some(leak(vault));
        }
    });
}

/// POST to a server-function endpoint and deserialise its JSON.
///
/// Server functions with no arguments still expect a POST with a JSON
/// body — an empty object is the encoding of a unit argument list.
/// Returns `None` on any failure: this is a dev convenience, and a dev
/// server that is restarting should make the page stale, not broken.
#[cfg(all(feature = "dev-guide", target_arch = "wasm32"))]
async fn post_json<T: serde::de::DeserializeOwned>(url: &str) -> Option<T> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_body(&wasm_bindgen::JsValue::from_str("{}"));
    let request = web_sys::Request::new_with_str_and_init(url, &opts).ok()?;
    request
        .headers()
        .set("Content-Type", "application/json")
        .ok()?;

    let window = web_sys::window()?;
    let response: web_sys::Response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .ok()?
        .dyn_into()
        .ok()?;
    if !response.ok() {
        return None;
    }
    let text = JsFuture::from(response.text().ok()?)
        .await
        .ok()?
        .as_string()?;
    serde_json::from_str(&text).ok()
}
