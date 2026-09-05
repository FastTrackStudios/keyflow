---
title: An Introduction
kind: concept
type: concept
order: 0
stage: Start here
---

# An Introduction

Keyflow is a plain-text format for charts. You write what the song does and it engraves.

```kf+
Song Title - Artist Name
4/4 #G 120bpm

VS 4
G C Em D
```

That is the whole file.

It is for the chart a band reads on a stand: form, chords, the hits that matter. Writing one should take about as long as saying it out loud. Not an engraving program; see [[alternatives|Alternatives]].

## Two goals

Simple charts stay simple. `G C Em D` is a chart — no header, no sections, nothing declared. The syntax for a key change exists; you meet it when the song has one.

> [!tip] The test for any new syntax
> Does the simplest chart get longer? Then it is wrong.

No project file, no sidecar, no database. A chart fits in a URL; a PDF carries its own source; anything that writes text can write a chart. Text diffs, merges, and still opens in thirty years.

## Letters, numbers, or numerals

Three ways to write a root. The chords come out the same.

````tabs
=== Letters
```kf+
4/4 #C 120bpm

VS 4
C F Am G
```

The pitches themselves. What people say out loud, and what a dep wants
to see.
=== Numbers
```kf+
4/4 #C 120bpm

VS 4
1 4 6m 5
```

Positions in the key. Change `#C` and every chord moves with it.
=== Roman numerals
```kf+
4/4 #C 120bpm

VS 4
I IV vi V
```

Positions again. The case carries the quality: `vi` is minor because it
is lowercase.
````

Numbers and numerals name positions rather than pitches, so transposing is editing one word.

---

Next: [[alternatives|Alternatives]] · Up: [[introduction|An Introduction]]

See also: [[header|Header]], [[structure|Structure]], [[chords|Chords]], [[rhythm|Rhythm]]
