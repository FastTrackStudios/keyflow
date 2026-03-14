# Keyflow Vision

## What Keyflow Is

Keyflow is a **music-first document format and toolchain**. It treats a song the way a programmer treats source code: as structured, diffable, parseable text that compiles into something useful — a rendered lead sheet, a synced lyric display, or a data model a DAW can reason about.

The `.kf` format is the source. Everything else — PDFs, interactive previews, MIDI integration, RPC services — is derived from it.

## The Problem

Musicians think in chords, sections, and feel. Existing tools force them into one of two worlds:

1. **Notation software** (MuseScore, Finale, Sibelius) — powerful but heavy. Designed for classical engraving. Overkill for a worship leader who needs chord changes above lyrics, or a jazz player sketching a chart on a plane.

2. **Text formats** (ChordPro, Nashville Number System, hand-typed Google Docs) — lightweight but dumb. No structure, no rendering, no tooling. Copy-paste is the integration story.

There's a gap between "I just need chords and lyrics" and "I need a full score." Keyflow lives in that gap.

## Core Principles

### Text is the source of truth

Charts are plain text. They live in git. They diff cleanly. They can be typed in any editor, pasted into a chat, or generated from MIDI. The format is designed to be readable without tooling — you should be able to hand someone a `.kf` file and they can play from it as-is.

### Parse once, render anywhere

The data model (`keyflow-proto`) is format-agnostic and RPC-ready. A chart parsed from text, imported from MIDI, or received over the wire all become the same `Chart` struct. From there, any consumer — PDF renderer, live preview, karaoke display, DAW sync — works with the same data.

### Rhythm is a first-class citizen

Most chord chart formats treat timing as decoration. Keyflow encodes it structurally: slash notation for simple feels, Lilypond-style durations for precision, push/pull markers for groove, triplet groups. A chart knows *when* each chord lands, not just *what* chord it is.

### Lyrics belong with chords

The `{Chord}syllable` syntax binds chords to the exact syllable they fall on. This isn't just for display — it enables syllable-level timing, karaoke sync, and chord-aware lyric layout. Combined with Knuth-Liang hyphenation, Keyflow can automatically align chords to syllable boundaries.

### Separation of concerns

The workspace is intentionally split:

- **keyflow-proto** defines *what a chart is* — pure data, no I/O, no rendering.
- **keyflow-text** defines *how to read/write the .kf format* — parsing and serialization.
- **engraver-proto** defines *how to draw a chart* — layout, fonts, vector graphics.
- **keyflow-ui** defines *how to interact with a chart* — editing, preview, signals.

This means a server can parse and analyze charts without pulling in GPU dependencies. A CLI can render PDFs without a UI framework. A web app can use the same data model as a native desktop app.

## Where Keyflow Is Going

### Near-term: Complete the lead sheet pipeline

- **Chord-syllable alignment** — the in-progress work binding chords to syllable positions with bidirectional mapping, enabling synced display and interactive highlighting.
- **ChordPro interop** — import and export ChordPro format for compatibility with existing tools and song databases.
- **Voicing blocks** — represent specific chord voicings alongside the abstract chord symbols, so the chart carries both "what to play" and "how to voice it."
- **Multi-block documents** — a single `.kf` file can contain keyflow notation, ChordPro, voicings, and future block types, each parsed by its own handler.

### Medium-term: Live performance and collaboration

- **Session integration** — charts that sync with DAW transport, following playback position in real time.
- **Interactive UI** — click a chord to hear it, drag to reharmonize, pinch to zoom. The Dioxus/WGPU foundation is already in place.
- **Real-time collaboration** — the RPC layer (Facet + Roam) is designed for networked chart sharing. A bandleader updates the chart; everyone's screen updates.

### Long-term: The music document platform

- **Arrangement intelligence** — analyze chord progressions for common patterns, suggest substitutions, detect key centers.
- **Multi-instrument parts** — extend beyond lead sheets to include bass lines, drum patterns, horn hits, all derived from or annotated on the same chart.
- **Education tools** — scale degree overlays, roman numeral analysis, interval visualization. Make theory tangible by connecting it to real charts.
- **Ecosystem integrations** — export to MusicXML, import from Ultimate Guitar, sync with Planning Center or ProPresenter, generate backing tracks from chord timing.

## Design Decisions

### Why Rust?

Correctness, performance, and portability. Music timing is unforgiving — a parser that silently drops a beat or misaligns a chord is worse than one that fails loudly. Rust's type system catches entire categories of timing and state bugs at compile time. The performance ceiling means the engraver can run at 60fps for interactive preview. And the compilation targets mean the same codebase runs native, in the browser (WASM), and on servers.

### Why a custom format instead of MusicXML/ChordPro?

MusicXML is verbose and notation-centric — it encodes *how to draw* music, not *what the music is*. ChordPro is too simple — it can't express rhythm, melodies, or multi-track structure. Keyflow's `.kf` format is purpose-built: dense enough to type quickly, structured enough to parse unambiguously, and extensible via multi-block documents.

That said, interop matters. Keyflow imports ChordPro and MIDI today, and MusicXML export is on the roadmap.

### Why Facet/Roam instead of serde/tonic?

Facet provides reflection-based serialization that works across formats (JSON, binary, pretty-print) without per-type boilerplate. Roam builds on Facet to provide RPC services where the same types are used on both sides of the wire. This matters because Keyflow's data model needs to travel — between crates, between processes, between machines — and the type definitions shouldn't have to be duplicated or manually kept in sync.

## Who Keyflow Is For

- **Worship teams** who need clean chord charts with lyrics, transposable on the fly, shareable as PDFs or live displays.
- **Gigging musicians** who want a compact, readable chart format they can edit in a text editor and render when they need something pretty.
- **Music educators** who want to annotate progressions with theory — scale degrees, roman numerals, harmonic analysis — on top of real songs.
- **Developers** building music tools who need a structured, well-typed chord/lyric/timing data model they can integrate via library or RPC.
- **DAW users** who want to bridge the gap between MIDI sequences and human-readable charts.
