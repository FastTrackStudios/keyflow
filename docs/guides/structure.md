---
title: Structure
kind: concept
type: concept
order: 5
stage: Chart
---

# Structure

A section is a name and a bar count.

```kf+
IN 2
VS 8
CH 4
```

## Naming a section

| Section | Short | Section      | Short  |
| ------- | ----- | ------------ | ------ |
| Intro   | `IN`  | Instrumental | `INST` |
| Verse   | `VS`  | Interlude    | `INT`  |
| Chorus  | `CH`  | Solo         | `SOLO` |
| Bridge  | `BR`  | Outro        | `OUT`  |

Case does not matter. Full list: [Sections](/appendix/sections).

## Music under a section

Chords go on the line below.

```kf+
VS 4
G C Em D
```

> [!warning] A section header holds no music
> `VS 4 G C Em D` fails. The header names the section and counts bars; music goes underneath.

## The bar count is enforced

It is not decoration. Three chords cannot fill four bars, and Keyflow will not guess which one you meant to hold:

```kf-
VS 4
G C Em
```

Rather than engrave a form you did not write, it fails and tells you what it counted. This is the first thing most people hit. It is also why a chart that parses is a chart whose form is right.

## Replaying a section

Name a section again with nothing under it.

```kf+
VS 4
1 4 5 1

CH 4
4 1 5 1

VS
CH
```

Lay out each section once, then order the repeats. Replays are numbered for you — *Verse 2*, *Chorus 2*.

## A note on the section

A quoted string after the count is a direction, engraved with the section name.

```kf+
VS 4 "Half-time"
G C Em D
```

## Changing key

A key on a section header changes it from there on.

```kf+
#G
VS 4
1 4 5 1

BR 4 #Bb
1 4 5 1
```

The degrees are unchanged; what they mean is not.

See also: [Sections](/appendix/sections), [[rhythm|Rhythm]]

---

Previous: [[header|Header]] · Next: [[chords|Chords]] · Up: [[introduction|An Introduction]]
