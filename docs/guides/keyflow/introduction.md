---
title: An Introduction
kind: concept
type: concept
order: 0
stage: Start here
---

# An Introduction

A plain-text format for charts. You type what the song does; it engraves.

```kf+
Song Title - Artist Name
4/4 #G 120bpm

VS 4
G C Em D
```

A complete file.

The job is **master rhythm charts** — form, chords, the hits that matter. Not an engraving program: see [[alternatives|Alternatives]].

## Two goals

**Complexity only when the music asks for it.** `G C Em D` is a valid chart. The syntax for a key change is there when the song needs one.

> [!tip] The test for any new syntax
> Does the simplest chart get longer? Then it is wrong.

**Everything derives from the plain text.** No project format, no sidecar, no database — so a chart fits in a URL, survives the PDF it made, and anything that writes text can write one. It also diffs, merges, and opens in thirty years.

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

Numbers and numerals name *positions in the key* — change the key and the chart follows.

---

Next: [[alternatives|Alternatives]] · Up: [[introduction|An Introduction]]

See also: [[header|Header]], [[structure|Structure]], [[chords|Chords]], [[rhythm|Rhythm]]
