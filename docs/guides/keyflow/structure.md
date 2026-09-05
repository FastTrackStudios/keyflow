---
title: Structure
kind: concept
type: concept
order: 2
stage: Start here
---

# Structure

A section is a name and a bar count.

```kf+
IN 2
VS 8
CH 4
```

The count is enforced. Chords that do not fill the section fail the parse, with what Keyflow counted.

## Naming a section

| Section | Short | Section      | Short  |
| ------- | ----- | ------------ | ------ |
| Intro   | `IN`  | Instrumental | `INST` |
| Verse   | `VS`  | Interlude    | `INT`  |
| Chorus  | `CH`  | Solo         | `SOLO` |
| Bridge  | `BR`  | Outro        | `OUT`  |

Case does not matter. Full list: [[appendix-sections|All Section Names]].

## Music under a section

Chords go on the line below.

```kf+
VS 4
G C Em D
```

> [!warning] A section header holds no music
> `VS 4 G C Em D` fails. The header names the section and counts bars; music goes underneath.

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

See also: [[appendix-sections|All Section Names]], [[rhythm|Rhythm]]

---

Previous: [[header|Header]] · Next: [[chords|Chords]] · Up: [[introduction|An Introduction]]
