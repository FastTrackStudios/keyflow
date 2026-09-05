---
title: LilyPond Rhythm Notation
kind: concept
type: concept
order: 12
stage: Rhythm
summary: Name the note value instead of counting beats.
---

# LilyPond Rhythm Notation

An underscore and a number names the duration.

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

Reach for it when the figure is easier to name than to count.

## Mixing with slashes

```kf+
VS 4
G_2 C // D_1
```

See also: [[slash-notation|Rhythmic Slash Notation]], the other way to write a duration · [[chords|Chords]]

---

Previous: [[slash-notation|Rhythmic Slash Notation]] · Up: [[rhythm|Rhythm]]
