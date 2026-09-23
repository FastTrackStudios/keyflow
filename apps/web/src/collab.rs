//! Editing a library chart together — Keyflow's half of Task's per-file
//! collaboration.
//!
//! A chart in the library is not a file on one person's machine: it is a
//! document on the workspace's charts shelf (`assets:charts/<slug>.md`),
//! and Task keeps one live Loro document per such file
//! (`features/vault/vault-collab`). Everyone with the chart open edits
//! that one document; Task writes it back to the file about a second
//! after the typing stops, and merges anybody else's write (Save from an
//! older client, a person editing the file in Task) into it.
//!
//! # The one thing that makes this more than a port
//!
//! The shared document is the **whole file** — frontmatter, a heading, the
//! chart in a ```` ```keyflow ```` fence, notes under it — and this editor
//! edits **only the chart**. So every crossing goes through the fence:
//!
//! ```text
//! editor text ──splice into the fence──▶ file text ──LoroText::update──▶ doc
//! doc text ──extract the fence──▶ chart source ──diff──▶ editor ("remote")
//! ```
//!
//! Both directions use Task's own [`resources_proto::assets`] fence
//! helpers, so what this side calls "the chart" is byte-for-byte what the
//! server calls it. A note somebody types under `## Notes` in Task, or a
//! frontmatter key the server rewrites, is outside the fence and is left
//! alone here.
//!
//! # What is pure, and why
//!
//! The mapping ([`source_of`], [`splice`], [`fence_body_start`],
//! [`normalized`]) is ordinary functions tested on the host; the session
//! is a thin driver over architect's `crdt` hooks, the same ones Task's
//! note editor uses.

// The session half is browser-only, like the library's dial.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use dioxus::prelude::*;
use editor::{DecoratedRange, EditorState, TransactionSpec};
use resources_proto::assets::{CHART_FENCE, extract_fenced, replace_fenced};
use uuid::Uuid;
use vault_proto::COLLAB_TEXT_CONTAINER;

/// How long a quiet peer stays listed. Mirrors Task's cursor channel.
const PRESENCE_TIMEOUT_MS: i64 = 30_000;

/// Re-announce this often even when idle, so a person reading the chart
/// without typing is still shown as here.
const HEARTBEAT_MS: u64 = 10_000;

/// Caret moves are per keystroke; peers only need where it settled.
const CARET_DEBOUNCE_MS: u64 = 150;

// ── The fence ────────────────────────────────────────────────────────

/// A chart source as the fence stores it: ending in a newline. The
/// fence writer adds one to a source that lacks it, so comparing the
/// editor's text to the file's without this would see a difference on
/// every keystroke at the end of a chart and bounce a newline back in.
#[must_use]
pub fn normalized(source: &str) -> String {
    if source.is_empty() || source.ends_with('\n') {
        source.to_owned()
    } else {
        format!("{source}\n")
    }
}

/// The chart inside a chart document, or `None` when it has no fence.
#[must_use]
pub fn source_of(file: &str) -> Option<String> {
    extract_fenced(file, CHART_FENCE)
}

/// The document with its chart replaced by `source` — every byte outside
/// the fence kept as it was.
#[must_use]
pub fn splice(file: &str, source: &str) -> String {
    replace_fenced(file, CHART_FENCE, &normalized(source))
}

/// Where the chart starts inside the document, in characters (Loro's
/// unit): the first character after the opening fence line. Matches the
/// rule Task's fence reader uses to find the block.
#[must_use]
pub fn fence_body_start(file: &str) -> Option<usize> {
    let mut offset = 0usize;
    for line in file.split_inclusive('\n') {
        offset += line.len();
        let trimmed = line.trim_end_matches(['\n', '\r']);
        let ticks = trimmed.chars().take_while(|c| *c == '`').count();
        if ticks >= 3 && trimmed[ticks..].trim() == CHART_FENCE {
            return Some(file[..offset].chars().count());
        }
    }
    None
}

// ── The session ──────────────────────────────────────────────────────

/// Everything the editor screen reads about the shared chart. `Copy`, and
/// every signal in it is owned by the screen ([`use_chart_collab`]), so
/// the editor's long-lived closures can hold it across a session
/// remount — the lesson Task's note editor paid for.
#[derive(Clone, Copy, PartialEq)]
pub struct ChartCollab {
    pub doc: crdt::DocHandle,
    pub presence: crdt::Presence,
    /// `true` once the server's copy has merged in and the shared
    /// document, not Save, is what keeps the chart.
    pub live: Signal<bool>,
    /// This tab's presence key. A tab, not a person: one person in two
    /// tabs is two carets.
    key: Signal<String>,
    /// The name shown on this tab's caret.
    name: Signal<String>,
    caret_seq: Signal<u64>,
    /// Where this tab's caret last was, in chart characters, so every
    /// re-announcement carries it — a heartbeat without it would wipe
    /// the caret peers are looking at.
    caret: Signal<Option<(usize, usize)>>,
}

/// A person on the chart besides you.
#[derive(Clone, Debug, PartialEq)]
pub struct Peer {
    pub key: String,
    pub name: String,
    pub hue: u32,
}

pub fn use_chart_collab(name: String) -> ChartCollab {
    ChartCollab {
        doc: crdt::use_doc_slot(),
        presence: crdt::use_presence_slot(),
        live: use_signal(|| false),
        key: use_signal(|| format!("cursor-{}", Uuid::new_v4())),
        name: use_signal(|| name),
        caret_seq: use_signal(|| 0),
        caret: use_signal(|| None),
    }
}

impl ChartCollab {
    /// Live right now: taken over, and the sync session is up.
    pub fn is_live(&self) -> bool {
        (self.live)() && self.doc.status() == crdt::SyncStatus::Live
    }

    /// The document's text, when a replica is attached.
    fn file(&self) -> Option<String> {
        let doc = self.doc.doc()?;
        Some(doc.loro().get_text(COLLAB_TEXT_CONTAINER).to_string())
    }

    /// A local edit: put the editor's chart into the shared document.
    /// Loro's own diff turns the whole-text update into the minimal ops,
    /// so a keystroke is a keystroke on the wire.
    pub fn push_local(&self, source: &str) {
        if !*self.live.peek() {
            return; // the takeover folds pre-live typing in
        }
        let Some(doc) = self.doc.doc() else { return };
        let text = doc.loro().get_text(COLLAB_TEXT_CONTAINER);
        let file = text.to_string();
        let next = splice(&file, source);
        if next == file {
            return;
        }
        if let Err(error) = text.update(&next, crdt::loro::UpdateOptions::default()) {
            tracing::warn!("collab: text update failed: {error}");
            return;
        }
        doc.loro().commit();
    }

    /// The editor's transaction sink: every local edit goes into the
    /// shared document **synchronously**, before any effect runs, so the
    /// document never lags the buffer. (Pushing from an effect instead let
    /// a keystroke typed between two effects look like a remote change
    /// and be reverted — the second letter of " feel" vanished that way.)
    ///
    /// The fast path turns the editor's own changes into text ops at the
    /// chart's offset in the file: exact, no diffing. When a remote
    /// update landed between the keystroke and this sink, the document
    /// no longer matches `doc_before`, and a diff of the whole chart is
    /// the always-convergent fallback.
    pub fn on_transaction(
        &self,
        event: &editor::editor_view::TransactionEvent,
        state: &EditorState,
    ) {
        if event.is_remote() || !*self.live.peek() {
            return; // an echo, or pre-live typing the takeover folds in
        }
        if event.is_edit() {
            let Some(doc) = self.doc.doc() else { return };
            let text = doc.loro().get_text(COLLAB_TEXT_CONTAINER);
            let file = text.to_string();
            let before = event.doc_before.to_string();
            match (fence_body_start(&file), source_of(&file)) {
                (Some(start), Some(shared)) if shared == normalized(&before) => {
                    let ops =
                        editor_crdt::changes_to_text_ops(event.doc_before.rope(), &event.changes);
                    apply_ops_at(&text, &ops, start);
                    doc.loro().commit();
                }
                _ => self.push_local(&event.doc_after.to_string()),
            }
        }
        self.publish_caret(state);
    }

    /// Everyone else here, from the presence channel. Reactive.
    pub fn peers(&self) -> Vec<Peer> {
        let own = self.key.read().clone();
        let mut peers: Vec<Peer> = self
            .presence
            .states()
            .into_iter()
            .filter(|(key, _)| key != &own && key.starts_with("cursor-"))
            .map(|(key, value)| Peer {
                name: name_in(&value).unwrap_or_else(|| "Someone".to_owned()),
                hue: hue_of(&key),
                key,
            })
            .collect();
        peers.sort_by(|a, b| a.name.cmp(&b.name).then(a.key.cmp(&b.key)));
        peers
    }

    /// The names of everyone else here, once each: one person in two
    /// tabs is two carets but one name.
    pub fn peer_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.peers().into_iter().map(|p| p.name).collect();
        names.dedup();
        names
    }

    /// Announce where this tab's caret is, debounced. Positions go up as
    /// Loro stable cursors minted against the *file*, so a peer resolves
    /// them against their own copy and our caret moves correctly as they
    /// type ahead of it.
    pub fn publish_caret(&self, state: &EditorState) {
        let primary = state.selection.primary();
        let rope = state.doc.rope();
        let char_at = |byte: usize| rope.byte_to_char(byte.min(rope.len_bytes()));
        let (anchor, head) = (char_at(primary.anchor), char_at(primary.head));

        let seq = self.caret_seq.peek().wrapping_add(1);
        let mut caret_seq = self.caret_seq;
        caret_seq.set(seq);
        let this = *self;
        spawn(async move {
            architect::sleep(architect::Duration::from_millis(CARET_DEBOUNCE_MS)).await;
            if *this.caret_seq.peek() != seq {
                return;
            }
            let mut caret = this.caret;
            caret.set(Some((anchor, head)));
            this.announce();
        });
    }

    /// Put this tab's presence up: its name, and its caret when known.
    fn announce(&self) {
        let caret = *self.caret.peek();
        let name = self.name.peek().clone();
        let mut entries = vec![("name".to_owned(), crdt::loro::LoroValue::from(name))];
        if let (Some((anchor, head)), Some(doc)) = (caret, self.doc.doc()) {
            let text = doc.loro().get_text(COLLAB_TEXT_CONTAINER);
            let file = text.to_string();
            if let Some(start) = fence_body_start(&file) {
                let len = text.len_unicode();
                for (label, at) in [("anchor_c", anchor), ("head_c", head)] {
                    let pos = (start + at).min(len);
                    if let Some(cursor) = text.get_cursor(pos, crdt::loro::cursor::Side::Left) {
                        entries.push((
                            label.to_owned(),
                            crdt::loro::LoroValue::Binary(cursor.encode().into()),
                        ));
                    }
                }
            }
        }
        let key = self.key.peek().clone();
        self.presence.set(
            &key,
            crdt::loro::LoroValue::Map(entries.into_iter().collect()),
        );
    }

    /// Other people's carets and selections, as editor decorations.
    /// Resolved against this tab's own copy of the document, then moved
    /// from file positions to chart positions; one outside the chart
    /// (someone in the notes, in Task) is not drawn.
    pub fn carets(&self, state: &EditorState) -> Vec<DecoratedRange> {
        let Some(doc) = self.doc.doc() else {
            return Vec::new();
        };
        let file = doc.loro().get_text(COLLAB_TEXT_CONTAINER).to_string();
        let Some(start) = fence_body_start(&file) else {
            return Vec::new();
        };
        let own = self.key.read().clone();
        let rope = state.doc.rope();
        let max = rope.len_chars();
        let mut out = Vec::new();
        for (key, value) in self.presence.states() {
            if key == own || !key.starts_with("cursor-") {
                continue;
            }
            let crdt::loro::LoroValue::Map(map) = &value else {
                continue;
            };
            let resolve = |label: &str| -> Option<usize> {
                let crdt::loro::LoroValue::Binary(bytes) = map.get(label)? else {
                    return None;
                };
                let cursor = crdt::loro::cursor::Cursor::decode(bytes).ok()?;
                let pos = doc.loro().get_cursor_pos(&cursor).ok()?.current.pos;
                let at = pos.checked_sub(start)?;
                (at <= max).then_some(at)
            };
            let (Some(anchor), Some(head)) = (resolve("anchor_c"), resolve("head_c")) else {
                continue;
            };
            let name = name_in(&value).unwrap_or_else(|| "Someone".to_owned());
            let hue = hue_of(&key);
            let (a, h) = (rope.char_to_byte(anchor), rope.char_to_byte(head));
            if a != h {
                out.push(DecoratedRange::mark_with_attrs(
                    a.min(h)..a.max(h),
                    "kf-collab-selection",
                    vec![(
                        "style".to_owned(),
                        format!("background-color: hsla({hue}, 80%, 60%, 0.22);"),
                    )],
                ));
            }
            out.push(DecoratedRange::widget(
                h,
                format!(
                    "<span class=\"kf-collab-caret\" style=\"border-color: hsl({hue}, 80%, 60%);\">\
                     <span class=\"kf-collab-caret-name\" style=\"background: hsl({hue}, 70%, 42%);\">{}</span>\
                     </span>",
                    escape_html(&name)
                ),
            ));
        }
        out
    }
}

/// Apply editor text ops to the document, shifted to where the chart
/// starts in the file and clamped to its length — positions can drift
/// when streams interleave, and Task's conflict policy is "may
/// interleave, never loses a whole edit".
fn apply_ops_at(text: &crdt::loro::LoroText, ops: &[editor_crdt::TextOp], start: usize) {
    for op in ops {
        let len = text.len_unicode();
        let result = match op {
            editor_crdt::TextOp::Insert { pos, text: t } => {
                text.insert((start + *pos as usize).min(len), t)
            }
            editor_crdt::TextOp::Delete { pos, len: n } => {
                let from = (start + *pos as usize).min(len);
                let n = (*n as usize).min(len - from);
                if n == 0 {
                    continue;
                }
                text.delete(from, n)
            }
        };
        if let Err(error) = result {
            tracing::warn!("collab: text op failed: {error}");
        }
    }
}

fn name_in(value: &crdt::loro::LoroValue) -> Option<String> {
    let crdt::loro::LoroValue::Map(map) = value else {
        return None;
    };
    match map.get("name") {
        Some(crdt::loro::LoroValue::String(s)) => Some(s.to_string()),
        _ => None,
    }
}

/// A stable hue per peer, so a person is the same colour in every tab.
#[must_use]
pub fn hue_of(key: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in key.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h % 360
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Bring the editor to `source` as a change tagged `"remote"`, so the
/// caret stays where the person left it and the edit is not sent back.
fn apply_remote(state: Signal<EditorState>, source: &str) {
    let current = state.peek().clone();
    let changes = editor_crdt::remote_text_to_changes(current.doc.rope(), source);
    if changes.is_empty() {
        return;
    }
    let mut state = state;
    state.set(current.update(TransactionSpec::new().changes(changes).user_event("remote")));
}

/// Drives one chart's session **into** the screen's [`ChartCollab`].
/// Mount keyed by the doc id: a remount is a fresh replica.
///
/// `opened` is the chart source the editor was opened with. Until the
/// server's copy arrives the editor is ordinary; when it does, the two
/// are reconciled once — if nobody typed, the shared copy (which may
/// already hold a collaborator's edits) wins; if somebody did, their
/// typing is folded into it.
#[component]
pub fn ChartCollabSession(
    doc_id: Uuid,
    collab: ChartCollab,
    state: Signal<EditorState>,
    opened: String,
) -> Element {
    crdt::use_synced_doc_into(collab.doc, doc_id);
    crdt::use_presence_channel_into(collab.presence, doc_id, PRESENCE_TIMEOUT_MS);
    let handle = collab.doc;
    let mut live = collab.live;

    use_effect(move || {
        let revision = handle.revision(); // the one reactive read
        if revision == 0 {
            return; // nothing merged yet
        }
        let Some(file) = collab.file() else { return };
        let Some(remote) = source_of(&file) else {
            return; // a chart with no fence has nothing to edit here
        };
        let buffer = state.peek().doc.to_string();

        if !*live.peek() {
            live.set(true);
            if normalized(&buffer) == remote {
                return;
            }
            if normalized(&buffer) == normalized(&opened) {
                apply_remote(state, &remote);
            } else {
                collab.push_local(&buffer);
            }
            collab.announce();
            return;
        }
        if normalized(&buffer) != remote {
            apply_remote(state, &remote);
        }
    });

    // Stay listed while reading, not only while typing.
    use_future(move || async move {
        loop {
            architect::sleep(architect::Duration::from_millis(HEARTBEAT_MS)).await;
            if *collab.live.peek() {
                collab.announce();
            }
        }
    });

    rsx! {}
}

/// Provide the org connection the `crdt` hooks sync over, fed from the
/// library's own dial — the same socket every other library call on this
/// chart uses, not a second one. Re-dials when it drops.
pub fn use_org_connection(org: Option<String>) {
    let conn = architect::use_connection_root::<vox::Caller>();
    use_future(move || {
        let org = org.clone();
        async move {
            let Some(org) = org else { return };
            #[cfg(target_arch = "wasm32")]
            loop {
                match crate::library::org_caller(&org).await {
                    Ok(caller) => {
                        conn.set_ready(caller.clone());
                        while caller.is_connected() {
                            architect::sleep(architect::Duration::from_millis(1_000)).await;
                        }
                        conn.set_connecting();
                    }
                    Err(error) => {
                        conn.set_failed(error.to_string());
                        architect::sleep(architect::Duration::from_millis(3_000)).await;
                    }
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let _ = (org, conn);
            }
        }
    });
}

/// Whether the chart is live, and who else is on it. Its own component so
/// that presence — which changes on every caret move — re-renders this
/// label and nothing else.
#[component]
pub fn CollabStatus(collab: ChartCollab) -> Element {
    let live = collab.is_live();
    let names = collab.peer_names();
    let (class, text, title) = match (live, names.is_empty()) {
        (false, _) => (
            "kf-collab-status",
            "Connecting…".to_owned(),
            "Joining the shared copy of this chart".to_owned(),
        ),
        (true, true) => (
            "kf-collab-status kf-collab-live",
            "Live".to_owned(),
            "Saved as you type. Anyone who opens this chart edits it with you.".to_owned(),
        ),
        (true, false) => (
            "kf-collab-status kf-collab-live",
            format!("Live · {}", names.join(", ")),
            format!("Editing now with {}", names.join(", ")),
        ),
    };
    rsx! {
        span { class, title, role: "status", "{text}" }
    }
}

/// Styles for peers' carets. Colours are per-person hues, like avatars,
/// not theme tokens.
pub const COLLAB_STYLE: &str = "
.kf-collab-selection { border-radius: 2px; }
.kf-collab-status {
  display: inline-flex; align-items: center; gap: 0.35rem;
  padding: 0.2rem 0.55rem; border-radius: 999px;
  border: 1px solid var(--line); color: var(--ink-soft);
  font-size: 0.75rem; white-space: nowrap;
}
.kf-collab-live { color: var(--ink); border-color: color-mix(in srgb, var(--syn-section) 55%, transparent); }
.kf-collab-live::before {
  content: \"\"; width: 0.45rem; height: 0.45rem; border-radius: 50%;
  background: var(--syn-section);
}
.kf-collab-caret { position: relative; border-left: 2px solid; margin-left: -1px; pointer-events: none; }
.kf-collab-caret-name {
  position: absolute; top: -1.2em; left: -2px;
  padding: 0 4px; border-radius: 3px 3px 3px 0;
  font-size: 0.65rem; line-height: 1.25; color: #fff;
  white-space: nowrap; user-select: none; pointer-events: none; z-index: 30;
}
";

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "---\ntype: asset\nasset_kind: chart\nslug: africa\n---\n# AFRICA\n\n```keyflow\nAFRICA - TOTO\n| A |\n```\n\n## Notes\n\nplay it slow\n";

    #[test]
    fn the_chart_is_the_fence_and_nothing_else() {
        assert_eq!(source_of(DOC).as_deref(), Some("AFRICA - TOTO\n| A |\n"));
        assert_eq!(source_of("# no chart\n"), None);
    }

    /// Splicing replaces the chart and keeps every other byte — the
    /// notes a collaborator is typing in Task are not the editor's.
    #[test]
    fn a_splice_touches_only_the_chart() {
        let next = splice(DOC, "AFRICA - TOTO\n| A | E |");
        assert_eq!(
            source_of(&next).as_deref(),
            Some("AFRICA - TOTO\n| A | E |\n")
        );
        assert!(next.starts_with("---\ntype: asset"));
        assert!(next.ends_with("## Notes\n\nplay it slow\n"));
        assert_eq!(
            splice(DOC, "AFRICA - TOTO\n| A |\n"),
            DOC,
            "no change is no change"
        );
    }

    /// The fence stores a trailing newline; a buffer without one is the
    /// same chart, not an edit to send.
    #[test]
    fn a_missing_trailing_newline_is_not_a_difference() {
        assert_eq!(normalized("| A |"), "| A |\n");
        assert_eq!(normalized("| A |\n"), "| A |\n");
        assert_eq!(normalized(""), "");
        assert_eq!(splice(DOC, "AFRICA - TOTO\n| A |"), DOC);
    }

    /// A caret at chart position `n` is at document position
    /// `start + n`, and `start` is where the chart's text begins.
    #[test]
    fn the_chart_starts_after_the_opening_fence_line() {
        let start = fence_body_start(DOC).expect("the document has a chart");
        let rest: String = DOC.chars().skip(start).collect();
        assert!(rest.starts_with("AFRICA - TOTO\n"));
        assert_eq!(fence_body_start("no fence\n"), None);
        // Non-ASCII before the fence is counted in characters, not bytes.
        let accented = DOC.replace("# AFRICA", "# Café Olé");
        let start = fence_body_start(&accented).unwrap();
        let rest: String = accented.chars().skip(start).collect();
        assert!(rest.starts_with("AFRICA - TOTO\n"));
    }

    #[test]
    fn a_peer_keeps_their_colour() {
        assert_eq!(hue_of("cursor-a"), hue_of("cursor-a"));
        assert!(hue_of("cursor-b") < 360);
    }
}
