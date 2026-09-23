---
title: Alterations
kind: reference
type: reference
order: 3
stage: Chords
summary: Notes moved out of the scale, written last.
---

# Alterations

A note moved out of the scale. See [Alteration](/guide/chord-alteration).

```kf+
#C
VS 6
Cb5 C#5 C7b9 C7#9 C7#11 C7b13
```

| Written | Moves |
|---|---|
| `b5` `#5` | the fifth |
| `b9` `#9` | the ninth |
| `#11` | the eleventh |
| `b13` | the thirteenth |

## Stacking

More than one, in order:

```kf+
#C
VS 3
C7b9b13 C7#9b13 C7b9#11
```

> [!warning] An altered fifth swallows what follows
> `Cm7b5b9` engraves `Cm7♭5` and `C7#5b9` engraves `C7♯5` — the second
> alteration is dropped without an error. Stacking from the ninth
> upward works.
> ([issue #10](https://github.com/FastTrackStudios/keyflow/issues/10))

## Half-diminished

```kf+
#C
VS 3
Cm7b5 F7 Bbmaj7
```

See also: [Alteration](/guide/chord-alteration), [[extensions|Extensions]]

---

Up: [Alteration](/guide/chord-alteration)
