---
title: Extensions
kind: reference
type: reference
order: 2
stage: Chords
summary: Sixths, sevenths, and the stack above them.
---

# Extensions

How far up the stack the chord goes. See [Extension](/guide/chord-extension).

```kf+
#C
VS 6
C6 C7 Cmaj7 C9 C11 C13
```

| Written | Adds |
|---|---|
| `6` | sixth |
| `7` | flat seventh |
| `maj7` | major seventh |
| `9` | ninth |
| `11` | eleventh |
| `13` | thirteenth |

An extension implies the ones beneath it — `C13` is a thirteenth chord, not a triad with a thirteenth added.

## Under a quality

Quality first, then extension.

```kf+
#C
VS 5
Cm6 Cm7 Cm9 Cm11 Cm13
```

```kf+
#C
VS 3
Cmaj7 Cmaj9 Cmaj13
```

> [!warning] `Cmaj11` engraves a minor chord
> `maj` with an eleventh under a seventh does not parse — `Cmaj11` and
> `Cmaj7#11` both engrave as `Cm`. `Cmaj13#11` is fine.
> ([issue #10](https://github.com/FastTrackStudios/keyflow/issues/10))

See also: [Extension](/guide/chord-extension), [[alterations|Alterations]]

---

Up: [Extension](/guide/chord-extension)
