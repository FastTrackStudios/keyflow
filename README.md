# Keyflow

A text format for musical charts, and the engine that renders them.

Keyflow is what you write when you need a chart rather than a score: the
harmonic and rhythmic skeleton of a song, in Nashville numbers or Roman
numerals or letter names, with lyrics, sections, repeats and annotations
layered on top. It reads like something a musician would scribble on a
chart, and parses like something a computer can follow in real time.

```
--- keyflow ---
Build My Life - Housefires
72bpm 4/4 #G

Intro: | 1 4 |

VS 1: | 1 4 | 5 6- |

--- chordpro ---
{sov: Verse 1 sync=lines}
[G]Worthy of every so[C/G]ng we could ever sing
{eov}
```

**Engraver** is the other half: the layout and render engine that turns a
parsed chart into a page — system breaking, width distribution, chord
symbol placement, section cards — rendered to Vello scenes (screen), SVG,
or PDF.

## Layout

```
features/
  proto          keyflow-proto — the domain model, musical primitives, syntax
  lang           keyflow-lang — the .kf parser and exporter, ChordPro; and as
                 features: midi, live, musicxml, musx (Finale), orchestra,
                 chordsheet, sync (synced lyrics), lsp (the language server)
  engraver       engraver — layout + render; svg / paint / wgpu / pdf tiers,
                 score (part import + page layout), pdfscore (PDF reading)
  keyflow        keyflow — the facade: proto, `keyflow::text`, `keyflow::engraver`
  annotate       keyflow-annotate — articulation model (no_std, shared with Signal)
  daw-analysis   keyflow-daw-analysis — reading harmony back out of a DAW project
  ui             keyflow-ui — Dioxus components: renderer mount, signals
  editor         editor-keyflow — .kf fences and language support in the Editor
  tree-sitter    tree-sitter-keyflow — the grammar

apps/
  cli            the `kf` command line tool
```

Keyflow has no app of its own: its editor, chart library and guide ship
inside [Studio](https://github.com/FastTrackStudios/studio), which builds
these crates for desktop, iOS and the browser.

A crate is a domain, not a format: each crate's parts are modules, and
the optional ones sit behind a Cargo feature of the same name.
`keyflow` and `engraver` are the **facades**; reach for `keyflow-lang`
only for a format the facade does not re-export.

## Where this sits

```
vendor ── architect ── task
             │
            daw
             │
         ▶ keyflow ◀        this repo
             │
          session
             │
          signal
             │
     FastTrackStudio
```

Cross-repo dependencies are git deps pinned to a tag. To co-develop
against a sibling repo, override the tag with a local checkout rather
than pushing a tag to test:

```toml
[patch."https://github.com/FastTrackStudios/daw"]
daw = { path = "../daw/crates/daw/daw" }
```

Never commit those overrides — the paths are machine-specific.

`daw` still carries its own copies of `keyflow-proto` and `keyflow-syntax`:
`expression-editor-core`, which `daw-reaper` hard-depends on, builds
against them. Keyflow's own `keyflow-proto` (with `syntax` folded in) is
the authority; daw's copies come out once its consumers are repointed.

## Build

```bash
nix develop          # or direnv: `use flake`
just check           # cargo check --workspace
just test            # cargo nextest run --workspace
just wasm-check      # the library crates for wasm32 — `just check` does NOT
just ci              # fmt + check + wasm-check + test, the order CI runs them
just kf --help       # the keyflow CLI
```

The tree-sitter grammar's generated `src/parser.c` is gitignored. A fresh
clone compiles a NULL stub and warns until you run `just grammar` once.

Some tests read reference corpora that are not in the repo (large
third-party scores). Those fail on a clean clone; that is expected and
predates this repo.

## Licence

GPL-3.0-or-later.
