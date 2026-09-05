---
title: Structure
kind: concept
type: concept
order: 5
stage: Chart
---

# Structure

A section is a name and how many bars it lasts.

```kf+
IN 2
VS 8
CH 4
```

Chords go on the line underneath.

```kf+
VS 4
G C Em D
```

## Auto Numbering Sections

You write `VS` every time. Keyflow numbers them for you, under this rule:

> A section can't increment in number unless there is another section
> between it.

This avoids the classic VS 3 confusion, where VS 3 sits right after CH 1
and someone has to ask which verse we're on.

```kf+
VS 4
G C Em D

CH 4
C G D D

VS 4
G C Em D

CH 4
C G D D
```

Two verses back to back are the same verse continuing, so they don't
increment.

```kf+
VS 8
G C Em D

VS 8
G C Em D
```

A section that only happens once gets no number at all.

Repeating a whole section is the same idea with nothing underneath it.

```kf+
VS 4
G C Em D

CH 4
C G D D

VS
CH
```

## Section names

| Section | Short | Section      | Short  |
| ------- | ----- | ------------ | ------ |
| Intro   | `IN`  | Instrumental | `INST` |
| Verse   | `VS`  | Interlude    | `INT`  |
| Chorus  | `CH`  | Solo         | `SOLO` |
| Bridge  | `BR`  | Outro        | `OUT`  |

Case doesn't matter. The full list is in [Sections](/appendix/sections).

## The bar count is checked

Three chords can't fill four bars, and Keyflow won't guess which one you
meant to hold. It tells you what it counted instead of engraving a form
you didn't write.

```kf-
VS 4
G C Em
```

## Changing key

A key on the section header applies from there on.

```kf+
#G
VS 4
1 4 5 1

BR 4 #Bb
1 4 5 1
```

## A direction on the section

```kf+
VS 4 "Half-time"
G C Em D
```

See also: [Sections](/appendix/sections), [[rhythm|Rhythm]]

---

Previous: [[header|Header]] · Next: [[chords|Chords]] · Up: [[introduction|An Introduction]]
