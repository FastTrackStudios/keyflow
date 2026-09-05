---
title: LilyPond Rhythm Notation
kind: concept
type: concept
order: 7
stage: Rhythm
summary: Name the note value instead of counting beats.
---

# LilyPond Rhythm Notation

Name the note value instead of counting beats. An underscore and a
number, LilyPond-style: `_1` is a whole bar, `_2` a half, `_4` a quarter.

```kf+
VS 4
G_2 C_2 D_2 Em_2
```

| Written | Note value |
|---|---|
| `_1` | whole |
| `_2` | half |
| `_4` | quarter |
| `_8` | eighth |
| `_2.` | dotted half |

The two systems mix inside one chart, so a bar that is easier to say in
beats can be written in beats:

```kf+
VS 4
G_2 C // D_1
```

Reach for note values when the rhythm is what a score would name rather
than what a player would count — a dotted figure, or anything that does
not fall on the beat.

See also: [[slash-notation|Rhythmic Slash Notation]], the other way to write a duration · [[chords|Chords]]

---

Previous: [[slash-notation|Rhythmic Slash Notation]] · Up: [[rhythm|Rhythm]]
