---
title: Extension
kind: concept
type: concept
order: 10
stage: Chords
summary: How far up the stack the chord goes.
---

# Extension

How far up the stack the chord goes.

```kf+
#C
VS 5
C7 Cmaj7 C9 C11 C13
```

| Written | Adds |
|---|---|
| `7` | flat seventh |
| `maj7` | major seventh |
| `9` | ninth |
| `11` | eleventh |
| `13` | thirteenth |

An extension implies the ones below it: `C13` is a thirteenth chord, not a triad with a thirteenth stuck on.

## With a quality

Quality first, then extension.

```kf+
Cm7 Cm9 Cmaj9 Cm11
```

## Added tones

`add` puts one tone on without the ones below it: `Cadd9` is a triad and a ninth, no seventh. The added 2nd has the worship-chart shorthand — a bare `2` on a triad (`G2`, `42` for the 4 chord, `4:2`) is `add2`, shown `Gadd2`, `4add2`. An addition can be parenthesised: `5(add4)`.

```kf+
Cadd9 G2 Dsus(add4) C
```

See also: [[chord-alteration|Alteration]], [[chord-quality|Quality]], [[chords|Chords]]

---

Previous: [[chord-quality|Quality]] · Next: [[chord-alteration|Alteration]] · Up: [[chords|Chords]]
