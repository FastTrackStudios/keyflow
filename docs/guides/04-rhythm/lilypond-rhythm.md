---
title: LilyPond Rhythm Notation
kind: concept
type: concept
order: 14
stage: Rhythm
summary: Name the note value instead of counting beats.
---

# LilyPond Rhythm Notation

An underscore and a number names the duration. [[rhythm|Rhythm]] has the one-line version.

| Written | Note value |
|---|---|
| `_1` | whole |
| `_2` | half |
| `_4` | quarter |
| `_8` | eighth |
| `_2.` | dotted half |

## Quarters and halves

```kf+
VS 4
G_4 C_4 Em_2 D_1
```

A quarter each for G and C, a half for Em, then D holds a whole bar.

## Dotted values

A `.` adds half again.

```kf+
VS 2
G_2. C_4 Em_1
```

Reach for note values when the figure is easier to name than to count — a dotted rhythm, or anything off the beat. Otherwise [[slash-notation|slashes]] read faster.

See also: [[slash-notation|Rhythmic Slash Notation]] · [[chords|Chords]]

---

Previous: [[slash-notation|Rhythmic Slash Notation]] · Up: [[rhythm|Rhythm]]
