---
title: Structure
kind: concept
type: concept
order: 5
stage: Chart
---

# Structure

A section is a name and how many bars it lasts. Chords go underneath.

```kf+
VS 4
G C Em D
```

## Auto Numbering Sections

You write `VS` every time. Keyflow numbers them for you, under this rule:

> A section can't increment in number unless there is another section
> between it.

This avoids the classic VS 3 confusion, where VS 3 sits right after CH 1 and someone has to ask which verse we're on.

A quoted note after the count marks a variant, and rides along with the numbering.

```kf+
VS 4
G C Em D

CH 4
C G D D

VS 4
G C Em D

CH 4 "Down"
C G D D
```

Two sections back to back are the same section continuing, so they don't increment. A section that only happens once gets no number at all.

## Section names

| Section | Short | Section      | Short  |
| ------- | ----- | ------------ | ------ |
| Intro   | `IN`  | Instrumental | `INST` |
| Verse   | `VS`  | Interlude    | `INT`  |
| Chorus  | `CH`  | Solo         | `SOLO` |
| Bridge  | `BR`  | Outro        | `OUT`  |

Case doesn't matter. The full list is in [Sections](/appendix/sections).

See also: [Sections](/appendix/sections), [[rhythm|Rhythm]]

---

Previous: [[header|Header]] · Next: [[reuse|Reusing a Progression]] · Up: [[introduction|An Introduction]]
