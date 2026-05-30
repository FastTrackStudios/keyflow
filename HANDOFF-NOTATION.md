# Keyflow — Notation & Parser Handoff

Running context for the `.kf` text → chord/notation track (parser, the three
notation systems, and the `docs/content/guide/` pages). Separate from
`HANDOFF.md`, which covers the MusicXML-import → engraver pipeline.

_Last updated: 2026-05-29 (rhythm groups + section-abbrev fix + rhythm doc page)_

---

## Where things stand

The chord-notation layer is in good shape. Recent arc (newest first):

- **`()` rhythm groups** (uncommitted) — `(a b c)` splits a target duration
  equally among its chords. Default target = one bar (`(C G)` = two half-bar
  chords); override with a trailing slash run (`(D Em)//` = two beats) or lily
  value (`(D Em G)_4` = eighth triplet over a quarter). N chords → even split, so
  `(D Em G)` is a whole-bar triplet. Implemented as a string-rewrite preprocessor
  `expand_chord_groups` in `chords.rs` (mirrors `apply_auto_durations`); rewrites
  to per-chord lily tokens (`_2`, `_2t`, …) so the main loop is untouched.
- **Single-letter section abbrevs removed** (uncommitted) — `c`/`b`/`v`/`i`/`o`
  no longer parse as section types; they shadowed chord roots (`C`, `B`) and
  numerals (`I`, `V`), so a content line `C G` was eaten as "Chorus, comment g".
  Sections now need ≥2 letters (`CH`, `VS`, `BR`, …). `section_type.rs`.
- **Diatonic quality for numbers** (`842ca93`) — bare Nashville numbers take the
  key's diatonic triad quality (`2` in C major = Dm). Overridable with `!`
  (literal, bare = major) or an explicit quality (`2M`, `2:maj`, `2m`, `2dim`).
- **`:` separator** (`5eedac9`) — optional readability colon between root and
  quality (`1:7` == `17`, `4:maj9` == `4maj9`). Works on all three systems.
- **Contextual `b7`** (`b637799`) — `b7` is the note B7 in a letter chart, the
  ♭7 degree in a number/Roman chart. Resolved line → section → chart, defaulting
  to note B7.
- **dim7 / Roman accidentals / `omit` alias** (`18d577b`) — dim7 → fully
  diminished; `bIII` keeps its accidental + lowercase; `omit5` aliases `no5`.

The three notation systems (letter / Nashville / Roman) are fully interchangeable
on the root; everything after the root parses identically across systems.

---

## Open work

### Docs (guide pages under `docs/content/guide/`)

Done (weights renumbered to put Sections after Structure): `_index.md`,
`structure.md` (w1), `sections.md` (w2), `chords.md` (w3),
`notation-systems.md` (w4), `rhythm.md` (w5), `melody.md` (w6). The sections page
covers section names + abbreviations, length in bars, replay-by-name, quoted
labels, custom `[…]` sections, and a key change on a header. The rhythm page
covers the measure-fill default, slashes, `()` groups + triplets, `_N` durations,
`%`, and `|`. The melody page covers `m{…}` blocks, letter/number pitch, relative
octaves + `'`/`,` nudges + `:` pin + `/octave`, the shared durations (rests `r` /
space `s` / tie `~`), stacked `<…>` notes, and pairing via `<< … ; … >>` +
sectioned lanes. Every example parse-verified.

- **The guide is now complete** — `lyrics.md` (w7) covers the `[lyrics]` track
  line, `{Chord}`-on-syllable markers, and hyphen melisma. All seven pages
  (Structure, Sections, Chords, Notation, Rhythm, Melody, Lyrics) are written and
  parse-verified.
- **Two section-parser bugs found while writing `sections.md` — now FIXED:**
  (1) a sub-labelled header (`CH 3A 10`) as the *first* section with a title line
  present mis-parsed to Intro — `looks_like_section_marker` (metadata.rs) had a
  drifted re-implementation of header detection that didn't know sub-labels; it
  now delegates to `SectionType::parse_with_measure_count` (also fixed latent
  first-section breakage for `CH 4 "Big"` / `BR 8 #G`). (2) `Pre-Chorus`/`PRE-CH`
  parsed to Intro — `parse_with_measure_count` now resolves `pre-`/`post-`
  prefixes via the new `base_section_type` helper. Both have regression tests.
- **Known remaining edge (not fixed):** a sub-label with *no* count (`CH 3A`,
  unusual — you normally write `CH 3A 4`) is read as count 3 because
  `MeasureExpression::parse("3a")` parses the leading digit. Consistent with/
  without a title now.

- **Melody supports letters + numbers only, not Roman numerals** (`melody.rs`
  parses scale-degree 1–7 or letter A–G; no numeral path). Corrected the old
  `notation-systems.md` "letter/number/numeral" claim to "letter or number".
- **Melody octave is `:` now, not `()`** (uncommitted). `C:4` = C in octave 4
  (mirrors the chord `root:quality` colon); single-digit octave, so `C:48` = oct
  4 eighth and `C:4_8` also works. Bare trailing number is still the *duration*
  (`C4` = quarter). The old `C(4)` parens are rejected with a guiding error, which
  frees `()` for future melody rhythm groups. Migrated the curated
  `04 PRESENCE Master RS.kf` chart (`(N)`→`:N`, parse output byte-identical to
  before) and fixed the long-standing `test_note_display` failure (display now
  canonicalises octaves to `:`). Wiring: `parse_explicit_octave`,
  `split_pitch_and_duration`, `melody_token_duration_start`, `melody_octave_colon`,
  and the two `Display` sites in `melody.rs`.
- **Melody notes default to a quarter when no duration is given** (and none to
  inherit) — `m{ C D E F }` is four quarter notes. Gate is the final `else` in
  `parse_with_defaults` (`melody.rs`); it appends `_4`. (Previously a bare first
  note errored and the whole melody was silently dropped.)
- **Sections / Lyrics pages** are the remaining guide gaps.
- The `docs/content/` folder will become a Dioxus app later — the dodeca SSG
  scaffolding was removed (`38b104c`), content kept. Doc page edits have been
  left uncommitted in the working tree by convention while content is in flux.

### Number-memory vs diatonic (design decision deferred)

For numbers, the diatonic quality always applies from the token — it does **not**
defer to chord-memory carryover the way letter chords do. So if you write `2maj`
then a later bare `2`, that bare `2` is still diatonic Dm, not a remembered D.
Arguably the right semantic for a number system, but revisit if users want
number-memory to win. Gate is the `infer_diatonic` block in `parse_chord_token`
(`crates/keyflow-text/src/chart/parser/chords.rs`).

---

## Landmines / gotchas

- **The real lexer is `keyflow-syntax`, not `keyflow-proto`.** `keyflow-proto`'s
  own `src/parsing/{token,lexer}.rs` are ORPHANED dead files — `crate::parsing`
  re-exports `keyflow_syntax::parsing::*`. Editing the proto copies does nothing.
  Burned a cycle on this with the `:` separator. Candidate for deletion.

- **Editions differ — no let-chains in the parser crates.** `keyflow-text` and
  `keyflow-proto` are edition 2021, so `if x && let Some(..)` fails to compile
  ("let chains only allowed in Rust 2024"). Use nested `if let` or `Option::zip`.
  `engraver-proto` is edition 2024 and allows them.

- **`cargo fix` over-prunes feature-gated glob imports.** It stripped imports
  only used under certain features (ast/metadata/parsing globs in
  `engraver-proto/lib.rs`, `TextSpan` in `ide/mod.rs`, `FiguredBassRow`). If you
  run it, check the diff and re-add with `#[allow(unused_imports)]`.

- **`b` always means flat in degree/numeral position.** Deliberate — most people
  can't type a real ♭. The only genuine ambiguity is `b5`/`b6`/`b7` (note B vs
  flat degree); `b9`+ is always the note B (degrees stop at 7), and any
  `#`-prefixed number is always a degree.

- **Pre-existing test failures (not regressions):** keyflow-proto ~1 failing,
  integration ~2 failing — red before this work; don't chase them as if you
  broke them. keyflow-text is fully green (117/0).

---

## How the recent features are wired (for future edits)

All in `crates/keyflow-proto/src/` + `crates/keyflow-text/src/chart/parser/chords.rs`.

**Diatonic quality** — reuses the existing scale/harmonization engine, nothing
new built:
- `Key::diatonic_quality(degree)` — `key/definition.rs`. Thin wrapper over
  `harmonize_scale(mode, root, Triads)`.
- `Chord::set_triad_quality(q)` — `chord/definition.rs`. Sets quality + re-runs
  `compute_intervals` so sevenths/extensions restack on the new triad.
- Application — `parse_chord_token` in `chords.rs`, gated by `infer_diatonic`
  (bare number, quality still Major, not an explicit-major token, has a diatonic
  scale degree). Display stays terse: chart shows `2`, only the underlying
  quality/intervals change.

**`:` separator** — added `TokenType::Colon` to `keyflow-syntax`
(`parsing/token.rs` + `lexer.rs` + `highlighting/highlighter.rs`). The chord
parser skips a `Colon` right after the root.

**`()` rhythm groups** — pure string-rewrite preprocessor, no main-loop change.
- `expand_chord_groups(line, time_sig)` in `chords.rs` runs just before
  `apply_auto_durations_between_separators` in `parse_chord_line_inner`. It finds
  `(…)[target]`, divides the target evenly, and rewrites each inner chord with a
  lily token via `beats_to_lily_suffix` (the inverse of `parse_lily_duration_beats`,
  extended with triplets). Target resolved by `group_target_beats` (default = one
  bar; `_N` lily; or a slash run `//`). Skips `"…"` and `m{…}` regions.
- Identity when a line has no `(`, so existing lines/tests/spans are unaffected.
- Limits: no nesting; only powers-of-two + triplet divisions notate (others
  error — quintuplets need wider `_Nt` token support); source spans on a grouped
  line are approximate (the rewrite changes token lengths). A bare dotted-slash
  chord placed right after a sub-beat group can mis-split a bar — a pre-existing
  slash quirk, reproducible with hand-written `_8t … /.`, not group-specific.

**Section abbreviations** — `SectionType::parse` and `parse_with_measure_count`
in `keyflow-proto/src/sections/section_type.rs` no longer accept single letters
(`c`/`b`/`v`/`i`/`o`); they collided with chord roots/numerals so `C G` parsed as
a Chorus header. Regression test: `test_single_letter_abbrevs_are_not_sections`.

**Notation system detection** — `NotationSystem { Auto, Letter, Degree }` in
`chord/root.rs`; `parse_root_with_system` / `detect_parser_order` route
accidental+Roman-letter → Roman first, and `b`+digit → ScaleDegree when the
system is Degree. The `b7` line→section→chart cascade uses vote counters on
`ChartParser` (`chart/mod.rs`: `chart_letter_votes` / `chart_degree_votes`).

---

## Related repo (editor)

`editor-keyflow` lives in `../../Task/features/editor/crates/editor-keyflow/`
(next to editor-mermaid/editor-typst). Renders ```` ```kf ```` fenced blocks to
SVG via `render_svg(source) -> Result<String, RenderError>` (the editor-mermaid
shape). Fence dispatch: `editor-state/src/markdown.rs` + `markdown/keyflow.rs`.
The editor facade no longer re-exports editor-typst. SVG works without GPU deps
because the engraver was split (`4dd670f`) — external crates use the `keyflow`
facade and never need the `engraver`/`wgpu` feature.
