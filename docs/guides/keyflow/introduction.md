---
title: An Introduction
kind: concept
type: concept
order: 0
stage: Start here
---

# An Introduction

Keyflow is a plain-text format for charts. You type what the song does; it engraves.

```kf+
Sunday Morning - The Wandering
4/4 #G 120bpm

VS 4
G C Em D
```

A complete file. Nothing omitted for the example.

## What it is for

**Master rhythm charts** — form, chords, the hits that matter. Typing one should take about as long as saying it out loud.

It is not an engraving program. For manual control use [MuseScore](https://musescore.org) or [LilyPond](https://lilypond.org). Keyflow exports SVG, so the door to a graphics editor is open.

> [!note]- Where it is going
> Whole orchestral parts, eventually. Precise manual engraving stays out of scope — that is a different job, and two projects already do it well.

## Two goals

**Complexity only when the music asks for it.** `G C Em D` is a valid chart — no header, no section, no declarations. When the song asks for a key change or a horn section, the syntax is there.

> [!tip] The test for any new syntax
> Does the simplest chart get longer? Then it is wrong.

**Everything derives from the plain text.** No project format, no sidecar, no database. That is why a chart fits in a URL, why the text survives the PDF, and why any tool that can write text can write a chart.

## Why plain text

- Survives — a `.kf` file opens in thirty years, in anything.
- Diffs, merges, version-controls like code.
- Scripts — a shell one-liner transposes a folder.
- Same input, same engraving. Change the house style and every chart follows.

## Letters, numbers, or numerals

Three ways to write a root. Same chords, same engraving.

````tabs
=== Letters
```kf+
4/4 #C 120bpm

VS 4
C F Am G
```

Names the pitches themselves. What most people say out loud, and what a
chart handed to a dep should probably use.
=== Numbers
```kf+
4/4 #C 120bpm

VS 4
1 4 6m 5
```

Names positions in the key. Change `#C` in the header and every chord
follows.
=== Roman numerals
```kf+
4/4 #C 120bpm

VS 4
I IV vi V
```

Positions again, with the case carrying the quality — `vi` is minor
because it is lowercase.
````

Letters name pitches. Numbers and numerals name *positions in the key* — change the key and the chart follows.

---

Next: [[header|Header]] · Up: [[introduction|An Introduction]]

See also: [[header|Header]], [[structure|Structure]], [[chords|Chords]], [[rhythm|Rhythm]]
