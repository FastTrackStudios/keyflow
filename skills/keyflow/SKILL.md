---
name: keyflow
description: Use when writing, editing, reviewing, or debugging Keyflow .kf chart files, especially section lengths, 6/8 rhythm syntax, parallel chord/melody blocks, figured bass, staff text, aliases, melody octave notation, and MusicXML-to-Keyflow chart equivalence.
---

# Keyflow

Use this skill whenever touching `.kf` files or Keyflow chart syntax. Prefer editing the `.kf` by hand once it has become a curated chart; do not regenerate over manual chart decisions unless explicitly asked.

## Core Workflow

1. Preserve the authorial chart shape first: section names, line breaks, aliases, comments, and manually chosen annotations matter.
2. After edits, run `cargo run -p keyflow-cli -- parse <path-to.kf>` and fix section length errors before trusting visual or MusicXML comparisons.
3. For MusicXML equivalence, use `cargo run -p keyflow-cli -- musicxml-compare <source.musicxml> <chart.kf> --max-diffs 40`. Do **not** pass `--include-source` for equivalence — it folds MusicXML-only `source_measure_number`/`width` into the signature, and `.kf` charts never carry those, so every measure reports as different.
4. If compare output is noisy, fix measure-count alignment first. Melody/chord differences after the first shifted measure may be false positives.

Useful commands:

```bash
cargo run -p keyflow-cli -- parse "examples/png-project-charts/02 LORD OF THE FIGHT Master RS.kf"
cargo run -p keyflow-cli -- musicxml-compare \
  "examples/png-project-charts/02 LORD OF THE FIGHT Master RS.musicxml" \
  "examples/png-project-charts/02 LORD OF THE FIGHT Master RS.kf" \
  --max-diffs 80
```

`musicxml-compare` parses the MusicXML and `.kf` into `Chart` objects and compares their expanded measure structures on a **musical-equivalence** signature: time signature, repeat/volta structure, chord-symbol sequence, dynamics, hairpins, and normalized melody (pitch, resolved octave, duration, articulation, stacked pitches). It deliberately ignores encoding that the two importers populate differently for identical music:

- Chord rhythm storage (`Slashes { count }` vs `Default`) and `MusicalPosition` — a whole-measure chord is the same chord either way.
- The `rhythm` element list — it only echoes the chords plus silence markers.
- Melody tie markers and rest/space-only ("silent") voices — an empty MusicXML bar and a `.kf` `s`/rest bar are equal.
- Free staff text, instrument cues (`@Inst`), and figured bass — engraving annotations the importers format differently (merged `<words>`, dropped accidentals, case/`*` variation). A remaining diff here is annotation drift, not a wrong note.

So a clean run means the **notes, chords, dynamics, and structure match**; remaining diffs are real musical differences worth investigating (e.g. a missing `dyn`, a transposed octave, a one-measure melody offset). Use `--include-source` only when manually debugging source measure alignment/widths — it is a debug aid, not an equivalence check.

`musicxml-kf` converts a MusicXML file to Keyflow text and prints it to **stdout** by default (capture with `> out.kf` or in tests); pass `--output <path>` to write a file. It never overwrites the input-adjacent `.kf`, so a curated chart next to its source is safe.

Round-trip self-validation (import → export → re-parse → compare) confirms the importer/exporter/parser agree:

```bash
cargo run -p keyflow-cli -- musicxml-kf "song.musicxml" > /tmp/rt.kf
cargo run -p keyflow-cli -- musicxml-compare "song.musicxml" /tmp/rt.kf
```

Residual round-trip diffs point at exporter/parser fidelity gaps (e.g. a chord like `B2` re-parses as `B` because keyflow durations use `_`, so a bare trailing digit on a chord symbol is lossy), not at the comparator.

## MusicXML Repeats

MusicXML may encode repeat barlines and first/second endings instead of writing every playback measure out. Hand-authored `.kf` files are often expanded, so comparisons must use the expanded playback form, not raw source measure numbers.

When accounting for repeats:

1. Count all normal measures before the repeat.
2. On the first pass, include the first ending.
3. On the repeated pass, replay from the forward repeat through the measure before the first ending.
4. Continue into the second ending through normal source order.
5. Compare against the hand-expanded `.kf` measure count.

For Lord of the Fight, the expanded MusicXML target is `111` measures:

```text
count 2
opening 4
intro 4
vs 1a 8
vs 1b 8
ch 1 main body 15 + first ending 4 = 19
vs 2 8
ch 2 main body 15 + second ending / The Climb 2 = 17
bridge 16
ch 3 12
tags 7
outro 6
```

If `musicxml-compare` reports late-song missing melodies or wrong chords, first check whether the expanded MusicXML measure count and parsed `.kf` measure count match. If they do not match, fix section parsing and repeat expansion before editing melodies.

## Section Lengths

Section headers use the section duration in measures, not the source measure number:

```kf
vs 8
ch 19
[The Climb] 2
outro 6
```

The parser should error if explicit section content parses to more measures than the header says. Treat that as a syntax bug or chart bug, not a warning to ignore.

For Lord of the Fight, the expanded target is 111 measures:

```text
count 2, opening 4, intro 4, vs 8, vs 8,
ch 19, vs 8, ch 15, The Climb 2,
br 16, ch 12, tags 7, outro 6
```

## Rhythm Conventions

In 6/8, `/.` is a half-measure dotted-quarter continuation unit. Two `/.` units fill one measure:

```kf
Amaj7 /. /.
F#m7 /. G#m7 /.
C#m /. B/C# /.
```

A full-measure chord does not need slashes:

```kf
C#m
```

For long continuation runs, separate slash groups for readability. Prefer this:

```kf
//// ////
```

over this:

```kf
////////
```

Use `/Duration` for a section-wide default duration for both chords and melody where repeated duration tokens are expected. Prefer this over `/ChordLength`:

```kf
intro 4
/Duration 8.
<<
  C#m B/C# A/C# G#m7/C# ;
  m { C# D# E F# }
>>
```

Durations can also stick from the first explicit melody or chord token. When a phrase has four or fewer repeated durations, put the duration on the first token and let it propagate:

```kf
A/D_8. B/C# Amaj7/B B/E
m { Dn8. C# B E }
```

Use `/Duration` only when it affects more than four items. A `!` prefix makes the whole token one-time and prevents it from updating duration memory.

A `/Duration` placed **before any section** (top-level) sets a chart-wide default that every section inherits. A section's own `/Duration` overrides the global default for that section only:

```kf
Song Title
64 BPM #E 4/4

/Duration 2

intro 4
/Duration 4   ; overrides the global default for this section
C#m /// /B / A // E/G# D B4-3

vs 8           ; inherits the global half-note default
E B/D# A/C# E/B ...
```

## Inline Time Signature Changes

Mid-chart meter changes use a `T` prefix on the meter (`T2/4`, `T6/8`), parallel to how key changes use `#` (`#E`, `#G`). The leading `T` is required so a meter change never collides with a number/Nashville slash chord like `4/6`. The change applies from that point until the next `T...`:

```kf
G D/F# Em G/D C D G D/C T2/4 Am7 T4/4 #A Esus ////
```

For a meter change that lasts exactly **one measure** and then reverts, prefix with `!` (the same "one-time" sigil used for durations): `!T2/4` means one measure of 2/4, then back to the prevailing meter — no closing `T4/4` needed:

```kf
G D/F# Em G/D C D G D/C !T2/4 Am7 #A Esus ////
!T2/4 D Esus // E // E5 ////
```

A bare `N/D` fraction is only honored on the header metadata line (`64 BPM #E 4/4`); mid-chart it would be read as a chord, so always use the `T`/`!T` prefix there.

## Parallel Chords And Melody

Use `<< ... ; ... >>` for parallel content. Keep the chord structure in one branch and melody in another branch when the melody spans across the same measures:

```kf
<<
  A/D_8. B/C# Amaj7/B B/E ;
  m { Dn(3)8. C# B,(2) E(3) }
>>
```

Multiline parallel blocks are preferred when they are easier to read.

For dense full-song charts, prefer sectioned lanes. The top-level section list is the table of contents and owns section lengths; lane sections repeat only the section name:

```kf
count 2
opening 4
intro 4
vs 8

let chords = {
  count
  s1 %

  opening
  r1 r1 s1 F#m7_8. G#m7 Amaj7 B
}

let melody = {
  count

  opening
  s1 .
  /octave 4
  C#8. D# E F#
  <,,F# 'C#>8. <G# 'D#> <A 'E> <B 'F#>
}

<< <chords> ; <melody> >>
```

The reserved `melody` lane is melody mode: bare content lines are melody syntax, while section names, `/Duration`, `/octave`, quoted staff text, instrument cues, `dyn`, and `hairpin` remain chart syntax.

## Melody Pitch Syntax

Melody notes use relative pitch unless an absolute octave is supplied with `(N)`:

```kf
m { C#(4)8. D# E F# }
```

Chord-note groups use `<...>` and are interpreted lowest-to-highest. Inside a group, apostrophe raises after the low-to-high baseline is chosen; comma lowers:

```kf
/octave 2 m { <F# 'C#>8. <G# 'D#> <A 'E> <B 'F#> }
```

Use `/octave N` to set the starting melody octave for a block or line when testing relative behavior.

Section-level octave:

```kf
/octave 2
m { <F# 'C#>8. <G# 'D#> <A 'E> <B 'F#> }
```

Inline octave for one chord/melody line:

```kf
<<
  F#m7_8. G#m7 Amaj7 B ;
  /octave 2 m { <F# 'C#>8. <G# 'D#> <A 'E> <B 'F#> }
>>
```

## Text, Instruments, And Figured Bass

Generic staff text uses quoted strings. Bare quoted text defaults below the staff. `_` is still allowed for below-staff text, but is optional. Use `^` when text must be above the staff:

```kf
"below staff by default"
_"below staff"
^"above staff"
```

Instrument-specific comments use `@Instrument`:

```kf
@Drums "Full Groove Now"
@Bass "Bass loco"
```

Figured bass can be attached to a chord with quoted text:

```kf
A"#4-3 2-1"
```

Aliases keep dense repeated annotations readable:

```kf
let fb = ^"4-3 2-1"
let #fb = ^"#4-3 2-1"

A<#fb>  C#m <fb> /. <fb> /.
```

## Dynamics And Hairpins

Classical dynamics use the `dyn` keyword so single-letter dynamics do not collide with note names:

```kf
dyn mp
dyn ff
dyn fff@4
```

The optional `@N` suffix sets the 1-based beat inside the measure. Dynamics default below the staff; add `above` only when needed:

```kf
dyn mf above
```

Hairpins use `hairpin` with `<` for crescendo and `>` for decrescendo:

```kf
hairpin < 2..4
hairpin > 1..4
```

These are structured notation, not generic staff text, and should be used when preserving MusicXML dynamics or wedges.

## Readability

Keep each chart line to roughly four measures. Dense measures can go one per line. Put major region text on its own line before the content:

```kf
ch 12
"=ROARING GROOVE"
A<#fb>  A /. B /.  C#m <fb> /. <fb> /.
```

Prefer readable repetition:

```kf
C#"Major Triad" %
```

`%` repeats the previous measure.
