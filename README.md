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
  keyflow            the facade — the only public API surface
  keyflow-text       the .kf parser and exporter
  keyflow-chordpro   ChordPro parsing, merged into charts as lyric tracks
  keyflow-midi       MIDI import/export
  keyflow-musicxml   MusicXML import/export
  keyflow-musx       Finale .musx import
  keyflow-live       live chart following (position, cursor, section state)
  keyflow-sync       chart sync between processes and devices
  keyflow-annotate   performance annotations over a chart
  keyflow-orchestra  orchestral articulation vocabulary and realization
  keyflow-daw-analysis  reading harmony back out of a DAW project
  keyflow-ui         Dioxus components — editor, renderer mount, signals
  keyflow-lsp        the language server
  cli                the `keyflow` CLI
  tree-sitter-keyflow   the grammar (syntax highlighting, structural edits)

features/engraver/
  engraver           the facade — svg / wgpu / pdf behind cargo features
  proto              layout model and the layout engine itself
  score              score-level import/export and the inventory harness

apps/
  web                keyflow.fasttrackstudio.app — landing page, editor, guide
  mobile             Keyflow for iOS — chart library + the Keyflow keyboard
```

`keyflow` and `engraver` are the **facades**. Everything else is internal
to the domain — consumers outside this repo depend on those two, not on
`keyflow-text` or `engraver-proto` directly.

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

`keyflow-proto` and `keyflow-syntax` deliberately live in the **daw**
repo, not here: `expression-editor-core`, which `daw-reaper` hard-depends
on, needs them, which makes a wire contract plus a syntax parser
foundation-layer. That is accepted, not an accident.

## The site

`apps/web` is [keyflow.fasttrackstudio.app](https://keyflow.fasttrackstudio.app):
a landing page, a live editor, and the guide.

It has **no backend**. A chart is deflated and base64url-encoded into the
URL, so sharing a chart is sharing a link — no account, no database. A
full song fits: the "Messengers of Hope" example encodes to about 700
characters. Past `MAX_URL_CHART_LEN` the share control says so instead of
handing over a URL that will be truncated in transit.

The guide is `docs/guides/keyflow/*.md`, rendered to HTML at build time by
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
