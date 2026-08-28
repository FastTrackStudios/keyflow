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
| [task](https://github.com/FastTrackStudios/task) | the Task product, the Editor stack, the vault/wiki layer | git dep (site only) |

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

## Build

```bash
nix develop          # or direnv: `use flake`
just check
just test
just ci              # what CI runs, in CI's order
just grammar         # regenerate the tree-sitter C parser (gitignored)
```

### Known-failing tests on a clean clone

~30 tests fail on a fresh checkout because they read reference corpora
(`lord_of_the_fight`, the orchestra corpus) that are not in the repo.
These failed identically in `session` before the split — they are not
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
