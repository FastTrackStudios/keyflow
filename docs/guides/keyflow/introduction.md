---
title: An Introduction
kind: concept
type: concept
order: 0
stage: Start here
---

# An Introduction

Keyflow is a plain-text format for writing charts. You type what the song does, and it engraves.

```kf+
Sunday Morning - The Wandering
4/4 #G 120bpm

VS 4
G C Em D
```

That is a complete file. Nothing was left out for the sake of the example.

## What it is for

The everyday job is a **master rhythm chart** — the one a band reads on a stand: form, chords, the hits that matter, and little else. Keyflow is built so that chart takes about as long to type as it takes to say out loud.

It is not an engraving program. If you need to move a slur three points to the left, you want [MuseScore](https://musescore.org) or [LilyPond](https://lilypond.org) — both wonderful, and neither of which Keyflow is trying to replace. Keyflow exports SVG, so the door out to a graphics editor is always open.

> [!note] Where it is going
> The aim is eventually to format whole orchestral parts. Precise manual control over engraving stays out of scope even then: that is a different job, and two projects already do it well.

## Two goals

Everything else in the format follows from these.

### Complexity only when the music asks for it

`G C Em D` is a valid chart. It needs no header, no section, and no declaration of anything, because the song has not asked for any of that yet.

When the song does ask — a key change, a horn section, synced lyrics — the syntax is there. What it must never do is make the simple chart pay for the complicated one's features.

> [!tip] The test for any new syntax
> Does the simplest chart get longer? If it does, it is the wrong design.

### Everything derives from the plain text

The file is the source of truth. There is no project format, no sidecar, no database row that the text is a view of.

That is what lets an entire chart live in a URL, and lets the text be recovered from the PDF it produced. It is also why other tools can generate and edit charts knowing nothing about Keyflow beyond how to write a text file — including, increasingly, tools that write the text for you.

## Why plain text

- It survives. A `.kf` file will open in thirty years, in anything.
- It diffs, merges and lives in version control like code.
- Anything can script it — a shell one-liner transposes a folder.

## What being opinionated buys

The same input always engraves the same way, so you never make a layout decision. Change the house style later and every chart you have written re-engraves to match.

## Letters, numbers, or numerals

A chord root can be written three ways. Same four chords, same engraving —
only the way you typed them differs:

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

Letters name pitches. Numbers and numerals name *positions in the key*, so they survive a transposition — change the key in the header and the chart follows. Pick the one your reader speaks; you can also switch the display afterwards without touching the file.

---

Next: [[header|Header]] · Up: [[introduction|An Introduction]]

See also: [[header|Header]], [[structure|Structure]], [[chords|Chords]], [[rhythm|Rhythm]]
