//! The editor pane's own stylesheet.
//!
//! Emitted by `KeyflowEditor` as a `<style>` element (`stylesheet: true`,
//! the default) so the pane lays out with nothing else loaded — Blitz
//! does not load `<link>`ed sheets, and Studio has no keyflow site sheet.
//! Written for Blitz: no `position: fixed`, no `overflow: auto` (the pane
//! scrolls with `overflow-y: scroll`), no `:has()`.
//!
//! The values are the keyflow site's (`apps/web/assets/site.css`), behind
//! the same custom properties, each with the site's dark value as its
//! fallback — a host themes the pane by setting `--ink`, `--line`,
//! `--accent`, … on an ancestor. The site itself ships these rules in its
//! own sheet and turns this one off.

/// The `kf-*` pane chrome: the header row, its buttons, the scrolling
/// code frame.
pub const PANE_CSS: &str = r#"
.kf-code-editor {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  min-width: 0;
  min-height: 0;
  height: 100%;
  color: var(--ink, #f2f2f7);
}
.kf-pane-head {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex: 0 0 auto;
  height: 2rem;
  padding: 0 0.15rem;
  margin-bottom: 0.5rem;
}
.kf-pane-name {
  font-family: var(--font-mono, ui-monospace, Menlo, monospace);
  font-size: 0.7rem;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--ink-soft, #9997a6);
  opacity: 0.7;
  margin-right: 0.25rem;
}
.kf-pane-spacer { flex: 1; }
.kf-code-editor .kf-note {
  font-size: 0.85rem;
  color: var(--ink-soft, #9997a6);
}
.kf-code-editor .kf-button {
  display: inline-block;
  padding: 0.15rem 0.55rem;
  border: 1px solid var(--line, #26262f);
  border-radius: 0.5rem;
  background: transparent;
  color: var(--ink, #f2f2f7);
  font-size: 0.8rem;
  font-weight: 550;
  line-height: 1.35;
  cursor: pointer;
}
.kf-code-editor .kf-button:hover { border-color: var(--ink-soft, #9997a6); }
.kf-code-editor .kf-button-on {
  background: var(--accent, #6ea8fe);
  border-color: var(--accent, #6ea8fe);
  color: var(--accent-ink, #0b1220);
}
.kf-code-editor-pane {
  min-height: 0;
  overflow-y: scroll;
  border: 1px solid var(--line, #26262f);
  border-radius: 0.75rem;
  background: var(--code-bg, #131319);
}
.kf-code-editor-frame {
  position: relative;
  height: 100%;
  min-height: 100%;
  padding: 1rem;
  font-family: var(--font-mono, ui-monospace, Menlo, monospace);
  font-size: 14px;
  line-height: 21px;
}
"#;
