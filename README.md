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
crates/keyflow/
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
