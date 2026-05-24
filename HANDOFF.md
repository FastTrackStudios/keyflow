# Keyflow MusicXML import — handoff

Snapshot of the MusicXML-import → chart → engraver pipeline, focused on the
Lord of the Fight (LotF) reference fixture.

Goal: **content parity** with Tom Brooks' PNG Project Charts (not pixel
parity). LotF is the canonical fixture; we iterate against
`examples/png-project-charts/02 LORD OF THE FIGHT Master RS.musicxml` and
compare PNG output under `examples/png-project-charts/rendered/`.

Reference MuseScore source is cloned at
`/home/cody/Development/FastTrackStudio/reference/MuseScore` — consult it for
any engraving rule we're trying to mimic.

---

## Where things live

- `crates/keyflow-musicxml/` — MusicXML → `Chart` importer. Wraps
  `musicxml = 1.1`. Main work in `src/convert.rs`.
- `crates/keyflow-proto/` — chart domain model (`Chart`, `Measure`,
  `ChordInstance`, `MelodyNote`, notations, …).
- `crates/engraver-proto/src/engraver/layout/chart/` — chart engraver.
  - `mod.rs` — orchestration, section/system/page loops.
  - `measure_render.rs` — per-measure layout (builder wiring).
  - `rhythm_builder.rs` — rhythm extraction + auto-rhythm slash expansion.
  - `width_dist.rs` + `measure_layout.rs` + `spacing.rs` — spring-based
    width distribution.
  - `count_in_renderer.rs` + `page_rendering.rs` — header / count-in snippet.
- `crates/keyflow-cli/` — `kf musicxml`, `kf musicxml-gallery` subcommands.
- `examples/png-project-charts/` — source MusicXML + reference PDFs.
  Rendered output lives in `rendered/<name>/page.pN.png`.

Regenerate everything: `cargo run -p keyflow-cli -- musicxml-gallery`.

---

## Done

### Importer
- Metadata: title (largest credit-words on page 1), subtitle, composer,
  lyricist, arranger, copyright (extracted from staff text into metadata).
- Initial clef + key from first `<attributes>`.
- Per-measure time signature; `chart.initial_time_signature` set from the
  first measure.
- `<measure number="…">` preserved on `Measure.source_measure_number`
  (drives both the section header label and the count-in overlay labels).
- Chord-symbol carry-forward at beat 1: when the first `<harmony>` of a
  measure starts past tick 0, we insert the previous bar's last chord at
  beat 1. Snapshot is taken **before** ingesting the current measure's own
  harmonies so a bar doesn't self-source.
- Rest-only melodies are dropped (so `/. /.` slash fill or the
  harmony-with-`Slashes` rhythm wins over a literal dotted-whole rest).
- XML entity decoding (`&lt;` `&gt;` `&amp;` `&quot;` `&apos;` numeric refs)
  — the `musicxml` crate's hand-rolled parser does not unescape; we do it
  at every text boundary (words, rehearsal, credit-words, movement-title,
  creator, rights).
- `<tie type="start"/>` / `<tie type="stop"/>` → `MelodyNote.tie_start` /
  `tie_stop`.
- Sub-beat chord position: `position_from_tick` produces
  `MusicalDuration { beats, subdivisions }` where 1000 subdivisions = 1
  beat. 6/8 dotted-eighth boundary lands at beat 2.5 exactly.
- Polyphony / octave stacks via `<note><chord/>` → `extra_pitches` on the
  preceding melody note. Engraver renders extra noteheads on the shared
  stem (`BeamNote.top_line` / `bottom_line` extends the stem range).
- `kind_to_suffix`: when `use-symbols="yes"`, ignore the ambiguous text
  glyph (`text="7"` for a triangle-7) and fall through to KindValue, so
  `<kind text="7" use-symbols="yes">major-seventh</kind>` correctly produces
  `maj7` (not `7`). `min` / `min7` etc. still normalize to `m` / `m7`.

### Engraver
- 6/8 beam grouping: `MeasureBuilder.compute_beam_groups` reads
  `TimeSignature::beam_groups()`; ts metadata is set on every measure
  (not just the first of a system) via `time_signature_meta()` /
  `clef_meta()` setters that gate glyph rendering separately.
- Per-beam-group auto stem direction (sum of line indices → up/down).
- Empty / rest-only bars render as `/. /.` (2 dotted-quarter slashes) in
  compound meters. `extract_from_slash` detects when every chord declares
  `Slashes { dotted: true }`, caps `num_beats` to the sum of declared
  counts, and emits `DottedQuarter` entries (1440 ticks = full 6/8 bar).
- Filler slashes are stemless: `compute_auto_stemless` treats
  `Quarter { dots ≤ 1 }` the same as a plain quarter (rhythm slash, no
  stem) — only explicit-rhythm chords keep stems.
- Section measure number reads `measure.source_measure_number` first, so
  LotF's Opening reads "3", not "1".
- Count-in overlay shows per-measure labels (`["1","2"]` for LotF) above
  the snippet staff. Source: `measure.source_measure_number` from the
  CountIn section.
- Content-aware measure width: `estimate_measure_content_weight` now sums
  per-segment `duration_stretch(ticks)` (slope^log2(ticks/quarter)) plus a
  small per-visible-chord-symbol bonus weighted by rendered text width.
  The old `clamp(0.5, 2.5)` cap is gone so dense bars can exceed baseline.

### Tests (`crates/keyflow-musicxml/src/lib.rs`)
- `imports_lord_of_the_fight_metadata`
- `imports_lord_of_the_fight_measures`
- `lord_of_the_fight_structure` — the big one. Asserts:
  - count-in length=2, xml numbers 1 & 2 on CountIn section measures;
  - measure 6: F#m7@1, G#m7@2.5, Amaj7@4, B@5.5 (sub-beat exact);
  - m7-m10 vertical-slash chord progression
    (C#m / B/C# / A/C# / G#m7/C#);
  - m7-m9 dotted-half melody with `tie_start`;
  - m10: C# dotted-quarter (`tie_stop`) + G# dotted-quarter no outgoing tie;
  - m3, m4, m11..=m16 all carry `ChordRhythm::Slashes { count:2, dotted:true }`
    and zero rendered melody notes.

Engraver builder unit tests of note:
- `test_auto_stemless_dotted_quarter_is_filler_slash` — pins the
  filler-slash-is-stemless rule.
- existing `test_auto_stemless_*` family — updated to match.

---

## Known gaps / next up

### Spacing (in progress; step 1 of 3 shipped)
Plan from the MuseScore-spacing research:
1. **Done** — duration-stretch-sum + chord-symbol bonus in
   `estimate_measure_content_weight`.
2. **Next** — sum per-segment `min_widths` into a per-measure minimum and
   feed that as the spring lower bound in
   `measure_layout::distribute_measure_widths_spring`. Today only the
   index-matched value is used as a floor; long chord-symbol clusters
   under-reserve width.
3. **Bigger refactor** — flatten to one spring per ChordRest segment
   across the whole system (mirrors MuseScore's `justifySystem` /
   `stretchSegmentsToWidth`). Solve once, sum widths back per measure.
   Defer until 1+2 are measured.

### Importer
- Honor explicit MusicXML `<stem>` direction per note (task #31). User
  explicitly deferred — auto stem from beam-group line sum is currently
  preferred.
- Direction `<offset>` is intentionally ignored on harmonies right now;
  Tom Brooks uses tiny offsets purely as visual nudges. If we ever support
  real-time-anchored chord positions for playback, revisit.
- Measure-style `<slash type="start">` / `<stop>` regions (LotF m11–m17)
  are handled implicitly via rest-drop + harmony-with-Slashes rhythm. We
  don't yet *parse* the directive — if a chart ever uses non-default
  `use-stems` / `use-dots`, we'd miss it. Add explicit handling when needed.
- Multi-measure / cross-system hairpins and voltas: opening spanners are
  tracked but we don't carry them across system breaks.
- Chord parsing: `Chord::parse` doesn't handle every Finale/Sibelius
  suffix (e.g. m12 has a Dolet warning about "4-3" being exported as
  text). We currently store the text as figured bass — works for LotF.

### Engraver
- Beat-3-vs-beat-1 stem balancing inside a single beam group could be
  smarter — right now we just sum line indices and pick up/down.
- The skyline / autoplace pass works for chord-vs-text collisions but
  doesn't yet handle figured-bass-vs-melody collisions cleanly when
  melody notes are above the staff.
- PDF export is wired (`kf pdf`, `kf midi-pdf`) but `musicxml-gallery`
  intentionally only outputs PNG — the PDF export had visual drift the
  user wanted to leave aside for now. Don't add PDF output to the gallery
  command without checking.

### Test coverage holes
- No engraver-level test pinning the rendered measure widths (Step 2/3
  of the spacing plan should add one — e.g. "dense bar > empty filler bar
  by at least 1.5×").
- Other six PNG-project-charts fixtures only get smoke tests; LotF is the
  only one with structural assertions. Pick the next worst-looking one
  (`07 I KNOW A NAME`?) and add a similar `*_structure` test.

---

## Commands

```bash
# Regenerate every chart fixture's PNG (use this after any change)
cargo run -p keyflow-cli -- musicxml-gallery

# Single chart
cargo run -p keyflow-cli -- musicxml \
  "examples/png-project-charts/02 LORD OF THE FIGHT Master RS.musicxml" \
  --output-dir /tmp/lotf

# Test
cargo test -p keyflow-musicxml lord_of_the_fight_structure
cargo test -p engraver-proto auto_stemless

# Debug-log a single render (beam grouping, etc.)
RUST_LOG=engraver_proto=debug cargo run -p keyflow-cli -- musicxml ... 2>&1 | grep beam
```

Rendered output: `examples/png-project-charts/rendered/<name>/page.p{1,2,3}.png`.

---

## Active tasks (TaskList)

- **#31** (pending) Honor explicit MusicXML `<stem>` direction per note —
  deferred per user.
- **#32–#36** (completed) Importer + test fixes for LotF.
- **#37** (completed) Step 1 of MuseScore-style content-aware spacing.

Open follow-ups not yet ticketed:
- Spacing plan steps 2 + 3 (per-measure min sum, system-wide segment spring).
- Add `*_structure` test for at least one more fixture.
- Parse `<measure-style><slash>` directive explicitly if/when a chart needs it.
