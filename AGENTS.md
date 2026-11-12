# keyflow — Repo Instructions

**This repo is the notation domain.** It was split out of
`FastTrackStudios/session` in August 2026, following the same pattern as
the `architect` / `daw` / `vendor` splits before it.

| repo | holds | consumed as |
|---|---|---|
| **keyflow** (here) | the chart language, its formats, the LSP + grammar, and Engraver | — |
| [daw](https://github.com/FastTrackStudios/daw) | the DAW platform and shared substrate | git dep, tag `v0.0.7` |
| [architect](https://github.com/FastTrackStudios/architect) | the framework (entity/RPC, atom, form, auth, permissions, crdt), `architect-ui` | git dep, tag `v0.12.4`, on the **`www.github.com`** spelling (Studio patches it to its own tag) |
| [session](https://github.com/FastTrackStudios/session) | the musical/production vocabulary and the Session app | consumes this repo |
| [editor](https://github.com/FastTrackStudios/editor) | the embeddable text/markdown editor — sits BELOW this repo | git dep, pinned by `rev` (`6817dc2`, architect v0.12.4's) |

One root Cargo workspace, one lockfile, one `target/`, one flake.
Intra-repo dependencies are path deps in root `[workspace.dependencies]`,
consumed as `x.workspace = true`. Cross-repo dependencies are **git deps
pinned to a tag** (the editor crates are pinned by `rev`).

**Co-developing across repos**: override the tag with a local checkout
rather than pushing a tag to test:

```toml
[patch."https://github.com/FastTrackStudios/daw"]
daw = { path = "../daw/crates/daw/daw" }
```

Never commit those overrides — the paths are machine-specific.

## Moving the Architect pin

Every Architect dependency here is spelled `https://www.github.com/FastTrackStudios/architect`,
on purpose. Cargo treats that spelling as a different source from the plain
URL, so Studio can patch exactly these pins to its own tag:
`[patch."https://www.github.com/FastTrackStudios/architect"]` in Studio's
Cargo.toml. In Studio's graph, keyflow's Architect tag is then not used.

- **An Architect patch release** (v0.12.4 → v0.12.5) needs **no keyflow
  release**. Studio takes it alone. The bot still bumps the pin here so
  keyflow's own CI builds on the newest Architect; no tag is needed.
- **An Architect minor** (v0.12 → v0.13) needs one: it may break this code,
  and so far every minor moved the editor, dioxus or blitz revisions this
  repo follows. Run `just bump-architect vX.Y.0 --version auto`, merge, then
  `just release-tag`. The bot sets `--version auto` on a minor by itself.
- **A new direct Architect dependency of a keyflow crate** must also go into
  Studio's www `[patch]` table, or Studio's lock gets a second Architect
  source. Studio's `just bump-architect` refuses a lock with one. Keep every
  Architect entry here on the www spelling.

`just bump-architect vX.Y.Z` moves every Architect pin (the
`[workspace.dependencies]` entries, the `[patch]` table, `features/ui`'s
`architect-workspace`, the comments and the table above) and `Cargo.lock`.
It exits 3 when Architect moved a pin this repo follows (editor, dioxus,
blitz): the rest is done, and a person moves that one.
`just bump-architect-test` replays keyflow#67 and #61.

When Architect releases a stable tag, `.github/workflows/bump-architect.yml`
runs the same command and opens `bump/architect-<tag>`. A mechanical bump
merges itself once `PR checks` and `CI passed` are green; a failing one
stays open with the failed jobs named. It is inert until the repo variable
`ARCHITECT_BUMP` is `on`. Its App token is minted for this repo only
(contents and pull-requests write). `just check-token-mints` (in `PR
checks`) fails any workflow that mints a wider one.

**Tagging** is `just release-tag`: it tags `origin/main` as
`v<workspace version>` once `CI passed` is green there. Only admins and the
fts-forge App may create `v*` tags once `scripts/tag-ruleset.sh --apply`
has been run (by the owner).

## Layout

One directory per crate under `features/`, named for the feature. A crate
is a *domain*, not a format: what used to be twenty small crates is a
handful of large ones, each old crate a module, and the optional parts
behind a Cargo feature of the module's name (the studio repo's pattern —
see `crates/frame` there). Split a crate only where the dependency graph
forces it: a layer boundary, a `no_std` consumer, a cross-repo cycle.

```
features/proto/       keyflow-proto — the domain model and the musical
                      primitives (chords, keys, sections, the time types),
                      and `syntax`: spans, tokens, the syntax AST, highlighting
features/lang/        keyflow-lang — the language: `text` (the parser and
                      the chart model) and `chordpro` always; `midi`, `live`,
                      `musicxml`, `musx`, `orchestra`, `chordsheet`
                      (+`manifest`), `sync` (+`sync-onnx`, `sync-whisper`, …)
                      and the `keyflow-lsp` bin (`lsp`) as features
features/engraver/    engraver — layout + render engine. Tiers `svg` →
                      `paint` → `wgpu`/`engraver`, plus `pdf`; `score`
                      (MusicXML / Finale parts, page layout) and
                      `pdfscore` (reading engraved music out of vector PDFs)
features/keyflow/     keyflow — the facade, the public API surface:
                      proto at the root, `keyflow::text`, `keyflow::engraver`
features/annotate/    keyflow-annotate — the articulation model; `no_std`,
                      dependency-free, shared with Signal's DSP
features/daw-analysis/ keyflow-daw-analysis — the chart service over the
                      `daw` client API (the one crate here that sees daw)
features/ui/          keyflow-ui — the Dioxus chart components
features/editor/      editor-keyflow — this repo's half of the editor
                      integration: the fence renderer (`render`) and `lang`
                      (decorations, hover, highlight)
features/tree-sitter/ the grammar
apps/cli/             the `kf` command line tool (Studio depends on it)
docs/guides/          the language guide
docs/spec/            tracey-tracked spec (score-engraving)
```

The layers, bottom up: proto → lang → engraver → keyflow → ui / editor /
cli. `annotate` sits beside proto; `daw-analysis` above the facade.

**There is no Keyflow app.** The standalone web site and iOS app were
removed: Keyflow's editor, chart library and guide ship inside Studio,
which builds these crates for desktop, iOS and the browser. So the
library crates keep their `wasm32` and `target_os = "ios"` support even
though nothing in this workspace builds an app for either target.

**Defaults and the root entry.** `keyflow-lang` and `engraver` default to
every native feature, so `cargo test -p <crate>` covers all of it. The
root `[workspace.dependencies]` entries turn defaults OFF, so a consumer
builds only the features it names — a wasm consumer must never pull
`sync`, `lsp` or `pdf`. Integration tests for a feature start with
`#![cfg(feature = "…")]`.

## Rules

- **`keyflow` and `engraver` are the public API surfaces**, with
  `keyflow-lang` for a consumer that needs a format the facade does not
  re-export (orchestra, sync). Inside the repo, prefer the facade too
  unless you are the crate directly beneath it.
- **Nothing here may depend upward on `session`, `signal`, or the app.**
  That edge is what the split removed: `keyflow-ui` used to carry
  `ChartView` / `ChartPreviewPanel`, which reached into `session_ui`
  playback signals and the dock. Panels that wire a chart to *app* state
  belong in the app. If you find yourself wanting `session::` in this
  repo, the component is in the wrong repo.
- **Keyflow owns the musical primitives; nothing here depends on `daw`
  to say what a bar is.** `keyflow-proto` (with its `syntax` module, once
  the `keyflow-syntax` crate) lives in `features/proto`. `TimeSignature`,
  `TimePosition`, `MusicalPosition`, `Tempo`, `Position` and `TimeRange`
  are defined in `features/proto/src/time/primitives/` — they used to be
  re-exported from `daw-proto`, which meant the notation domain asked the
  DAW platform what 4/4 meant. Keyflow is the authority on understanding
  a piece of music; the DAW reads these from us.
  `daw` still carries its own copies of `keyflow-proto` and
  `keyflow-syntax`, and
  `expression-editor-core` still builds against those. That duplication
  is deliberate and temporary — daw's copies come out once its consumers
  are repointed here. Until then, **a change to `features/proto` is not
  live for `daw` until it is made there too.**
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
- **`cargo check --workspace` does not check the wasm build.** It builds
  every crate for the *host*, where every `cfg(target_arch = "wasm32")`
  block and the whole WebGL surface are invisible. Run `just wasm-check`
  (CI does): the browser build of the surface Task's and Studio's web
  apps take. Two wasm traps, both of which *compile* and then fail at
  runtime or link time:
  - `tokio::time::sleep` needs a reactor and panics in the browser. Use
    `architect::platform::sleep`, or target-split onto `gloo-timers`.
  - `getrandom` 0.3 needs BOTH the `wasm_js` cfg (`.cargo/config.toml`)
    and the `wasm_js` feature, which only a direct dependency of the
    final binary can turn on. Either alone is a `compile_error!`.
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
  `features/engraver/tests/one_pipeline.rs` asserts the invariants
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
- **Engraving belongs in `use_memo`, not `use_effect`.** It is a pure
  function of the source. An effect that writes a signal the component
  also reads re-enters on every pass.

## Build

```bash
devenv shell         # or direnv: .envrc (`use devenv`)
just check
just test
just ci              # what CI runs, in CI's order
just grammar         # regenerate the tree-sitter C parser (gitignored)
```

### The corpus tests are `#[ignore]`d

`just test` is green on a clean clone. Twenty-nine tests that read
reference corpora are marked `#[ignore]`, because the corpora are not
where a clean clone's tests can read them:

- `features/examples/mxl` — the orchestral corpus the `orchestra`
  tests read, which is transcriptions of commercial scores. This repo is
  public; they are not ours to redistribute.
- `features/examples/png-project-charts` — the reference charts the
  MusicXML importer and the engraver layout tests measure against.
- `features/examples/chordsheet/source` (plus `manifest.json` beside it) —
  our chordsheet.com account backup, which the chordsheet corpus tests
  (keyflow-lang's, and the engraver's `chordsheet_engraving`) convert,
  round-trip and engrave. These charts are ours and ARE
  committed, at `examples/chordsheet-compat/` — but outside the crate, and a
  fixture lives inside the crate that reads it, so the tests look for a
  local copy: `cp -r examples/chordsheet-compat/chordsheet
  features/examples/chordsheet/source` and the manifest beside it.

Put a local copy at those paths and run `cargo test -- --ignored` to run
them. Each `#[ignore]` says which corpus it wants.

They used to just fail, all thirty-one of them, and CI had been red on
every commit for long enough that the job carried no signal — a real
regression could not be told from the standing noise. If you add a test
that needs data the repo does not ship, `#[ignore]` it with a reason.
Never weaken an assertion to make it pass, and never leave it failing.

## Logging & tracing — wide events, ALWAYS

- **The span IS the wide event.** Enrich it with
  `architect_telemetry::wide::set("namespace.field", value)` — one
  context-rich event per request, never scattered log lines.
- **Never `println!`/`eprintln!`/`dbg!` in library code** — not in
  committed code, and not as debug scaffolding. To chase a bug,
  reproduce it in a failing unit test.
- Record the **shape**, never the secret.
