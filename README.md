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
  web            keyflow.fasttrackstudio.app — landing page, editor, guide
  mobile         Keyflow for iOS — chart library + the Keyflow keyboard
```

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

## The site

`apps/web` is [keyflow.fasttrackstudio.app](https://keyflow.fasttrackstudio.app):
a landing page, a live editor, and the guide.

It has **no backend**. A chart is deflated and base64url-encoded into the
URL, so sharing a chart is sharing a link — no account, no database. A
full song fits: the "Messengers of Hope" example encodes to about 700
characters. Past `MAX_URL_CHART_LEN` the share control says so instead of
handing over a URL that will be truncated in transit.

The guide is `docs/guides/*.md`, rendered to HTML at build time by
`apps/web/build.rs` — no markdown parser reaches the browser. The guides'
own fence convention is honoured: ` ```kf- ` is a syntax illustration shown
as source, ` ```kf+ ` is a real chart, engraved on the page with a link
that opens it in the editor. A test asserts every `kf+` fence parses *and*
engraves, so the guide cannot teach something the parser rejects.

Charts render as SVG, not on a GPU canvas. Layout costs about 10 ms and the
result is static until the source changes, so it is serialised once and the
browser scales, prints and selects it for free. The WebGL surface in
`keyflow-ui` remains for what it was built for — a cursor tracking playback
at 120 Hz in the desktop app.

```bash
just web            # dx serve, hot reload
just web-build      # the shipping bundle
just web-check      # compile for wasm32 — `just check` does NOT
```

Deployment needs SPA fallback (unknown paths serve `index.html`);
`nix/modules/packages/static-site.nix` does this.

## The iOS app

`apps/mobile` is the chart library and editor, plus the reason it exists: a
**custom keyboard** for Keyflow syntax, in the spirit of Musician Keyboard.
`|`, `♭`, `𝄆` and section headers are not on the stock keyboard, which is
what makes writing a chart on a phone unpleasant today.

The keyboard's layout, key semantics and suggestions live in Rust
(`src/keyboard.rs`) and are tested against the real parser; the Swift
extension that draws them is not yet written. See
`apps/mobile/ios/README.md` for why the split falls there and what the
extension's sandbox forbids.

## Build

```bash
nix develop          # or direnv: `use flake`
just check           # cargo check --workspace
just test            # cargo nextest run --workspace
just ci              # fmt + check + test, the order CI runs them
just kf --help       # the keyflow CLI
```

The tree-sitter grammar's generated `src/parser.c` is gitignored. A fresh
clone compiles a NULL stub and warns until you run `just grammar` once.

Some tests read reference corpora that are not in the repo (large
third-party scores). Those fail on a clean clone; that is expected and
predates this repo.

## Licence

GPL-3.0-or-later.
