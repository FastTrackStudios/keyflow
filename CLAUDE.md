# keyflow — Repo Instructions

**This repo is the notation domain.** It was split out of
`FastTrackStudios/session` in August 2026, following the same pattern as
the `architect` / `daw` / `vendor` splits before it.

| repo | holds | consumed as |
|---|---|---|
| **keyflow** (here) | the chart language, its formats, the LSP + grammar, Engraver, and the keyflow site | — |
| [daw](https://github.com/FastTrackStudios/daw) | the DAW platform and shared substrate — including `keyflow-proto` and `keyflow-syntax` | git dep, tag `v0.0.2` |
| [architect](https://github.com/FastTrackStudios/architect) | the framework (entity/RPC, atom, form, auth, permissions, crdt), `architect-ui` | git dep, tag `v0.0.2` |
| [session](https://github.com/FastTrackStudios/session) | the musical/production vocabulary and the Session app | consumes this repo |
| [editor](https://github.com/FastTrackStudios/editor) | the embeddable text/markdown editor — sits BELOW this repo | git dep, tag `v0.1.0` |
| [task](https://github.com/FastTrackStudios/task) | the Task product and the vault/wiki layer | git dep (`view-knowledge-graph`, site only) |

One root Cargo workspace, one lockfile, one `target/`, one flake.
Intra-repo dependencies are path deps in root `[workspace.dependencies]`,
consumed as `x.workspace = true`. Cross-repo dependencies are **git deps
pinned to a tag**.

**Co-developing across repos**: override the tag with a local checkout
rather than pushing a tag to test:

```toml
[patch."https://github.com/FastTrackStudios/daw"]
daw = { path = "../daw/crates/daw/daw" }
```

Never commit those overrides — the paths are machine-specific.

## Layout

```
crates/keyflow/       the language: facade + text/chordpro/midi/musicxml/
                      musx/live/sync/annotate/orchestra/daw-analysis/ui,
                      the LSP, the CLI, the tree-sitter grammar
features/engraver/    the layout + render engine: facade, proto (the
                      layout model and engine), score (import/export)
features/editor/      this repo's half of the editor integration:
                      editor-keyflow (the fence renderer) and
                      editor-keyflow-lang (decorations, hover, highlight)
apps/web/             keyflow.fasttrackstudio.app — landing page, editor,
                      guide. Wasm; charts render as SVG.
apps/mobile/          Keyflow for iOS — chart library + the Keyflow
                      keyboard extension
docs/guides/keyflow/  the language guide — also the source content for
                      the site's embedded tutorial
docs/spec/            tracey-tracked spec (score-engraving)
```

## Rules

- **`keyflow` and `engraver` are the only public API surfaces.** Nothing
  outside this repo depends on `keyflow-text`, `keyflow-midi`,
  `engraver-proto` and so on. Inside the repo, prefer the facade too
  unless you are the crate directly beneath it.
- **Nothing here may depend upward on `session`, `signal`, or the app.**
  That edge is what the split removed: `keyflow-ui` used to carry
  `ChartView` / `ChartPreviewPanel`, which reached into `session_ui`
  playback signals and the dock. Panels that wire a chart to *app* state
  belong in the app. If you find yourself wanting `session::` in this
  repo, the component is in the wrong repo.
- **`keyflow-proto` and `keyflow-syntax` live in `daw`, not here.**
  `expression-editor-core` (which `daw-reaper` hard-depends on) needs
  them, so they are foundation-layer. Do not try to move them back
  without also breaking `daw-reaper → expression-editor-*`.
- **`default-features = false` cannot be applied to a workspace-inherited
  dep.** Put it on the `[workspace.dependencies]` entry, not the consumer.
- **`include_str!` across a repo boundary does not work.** A git dep has
  no stable path on disk, and these are invisible to cargo's dependency
  graph, so they fail at compile time rather than resolution time. Export
  the bytes from the owning crate instead. (This broke the last split
  three times — and a fixture reference that escaped its crate,
  `../../../../examples/build_my_life.kf`, is exactly how one test
  arrived here already broken.)
- **Test fixtures live inside the crate that reads them.** Under
  `<crate>/tests/fixtures/`, never up and out of the crate directory.
- Async: `tokio::sync::*` for locks/channels; `architect::platform::{spawn,
  sleep, timeout}` for tasks/timers — the wasm-cfg-split seam.
- **`cargo check --workspace` does not check the site.** It builds
  `keyflow-web` for the *host*, where every `cfg(target_arch = "wasm32")`
  block and the whole WebGL surface are invisible. Run `just web-check`
  (CI does). Two wasm traps already caught this way, both of which
  *compile* and then fail at runtime or link time:
  - `tokio::time::sleep` needs a reactor and panics in the browser. The
    editor debounce is target-split onto `gloo-timers`.
  - `getrandom` 0.3 needs BOTH the `wasm_js` cfg (`.cargo/config.toml`)
    and the `wasm_js` feature (a direct dep in `apps/web`). Either alone
    is a `compile_error!`.
- **Never enable `keyflow-ui`'s `web` feature from a wasm consumer.** It
  turns on `dioxus/desktop`, which drags dioxus-desktop → tungstenite →
  native-tls → openssl-sys, and openssl does not build for wasm32. That is
  why the root's `keyflow-ui` entry sets `default-features = false`.
- **`ChartPipeline` is the only way to engrave a chart.** Fonts, layout
  engine, presets and every SVG/PDF export live on
  `engraver::api::pipeline::ChartPipeline` — take one with
  `ChartPipeline::shared()` (or `with_style` if you genuinely need a
  different style) rather than calling `ChartFontBundle::new()` and
  wiring an engine by hand. Three places used to do all four steps
  independently, and they drifted: chord symbols are emitted as
  `MuseJazz Text`, *with a space*, and one of the three declared
  `MuseJazzText`, so every chart it exported fell back to a system sans
  and the `maj7` triangles came out blank. A `font-family` nothing
  declares does not error — it silently substitutes, which is why
  `features/engraver/proto/tests/one_pipeline.rs` asserts the invariants
  instead of trusting review. `ChartFontBundle::new()` outside
  `ChartFontBundle::shared()` is the smell; the bundle costs seven font
  files parsed per call.
- **A WebGL context is a scarce resource.** `keyflow-ui::ChartGraphics` is
  for one live surface, not one per chart on a page: browsers cap contexts
  at around sixteen, and re-creating one per render wedges the renderer.
  Static charts go through `export_svg_snippet` /
  `export_svg_pages_linked` instead. Pair the linked exports with
  `font_face_css` emitted once per document — the embedding variants cost
  ~485 KB of font data *per chart*.
- **The editor sits BELOW this repo, and must stay there.** `editor-state`
  knows nothing about keyflow; `editor-keyflow` implements its
  `fence_renderer` seam and registers itself. If you find yourself adding
  `keyflow` to a crate in the editor repo, the code belongs here instead.
- **The site's Tailwind sheet is build output.** `apps/web/assets/tailwind.css`
  is gitignored and `asset!()` demands it at compile time, so a fresh
  clone must `just tailwind` before `cargo check` passes (CI does). It
  scans two GIT DEPS — architect-ui and view-knowledge-graph — through
  symlinks that `just _tw-link` resolves via `cargo metadata`, because a
  git dep has no stable path to glob. **A `@source` that matches nothing
  is silent**: no error, just missing classes and an unstyled component.
  `just tailwind-check` exists to make that loud. architect-ui's prebuilt
  `UTILITIES_CSS` is not a substitute — it lacks `cursor-grab`,
  `cursor-grabbing` and `backdrop-blur-sm`, which the graph view needs.
- **Engraving belongs in `use_memo`, not `use_effect`.** It is a pure
  function of the source. An effect that writes a signal the component
  also reads re-enters on every pass.

## Build

```bash
nix develop          # or direnv: `use flake`
just check
just test
just ci              # what CI runs, in CI's order
just grammar         # regenerate the tree-sitter C parser (gitignored)
```

### Known-failing tests on a clean clone

~31 tests fail on a fresh checkout. Thirty read reference corpora
(`lord_of_the_fight`, the orchestra corpus) that are not in the repo and
failed identically in `session` before the split; the thirty-first,
`editor-keyflow-lang … section_headers_get_resolved_name_badges`, fails
identically in `task` at the commit those crates moved from. None is
split damage. Fix them by moving the corpus in or marking them
`#[ignore]`; do not "fix" them by weakening assertions.

## Logging & tracing — wide events, ALWAYS

- **The span IS the wide event.** Enrich it with
  `architect_telemetry::wide::set("namespace.field", value)` — one
  context-rich event per request, never scattered log lines.
- **Never `println!`/`eprintln!`/`dbg!` in library code** — not in
  committed code, and not as debug scaffolding. To chase a bug,
  reproduce it in a failing unit test.
- Record the **shape**, never the secret.
