---
title: Qualities
kind: reference
type: reference
order: 1
stage: Chords
summary: Major, minor, diminished, augmented, suspended, power.
---

# Qualities

What kind of chord it is. See [Quality](/guide/chord-quality).

```kf+
#C
VS 7
C Cm Cdim Caug Csus2 Csus4 C5
```

| Written | Quality |
|---|---|
| *(nothing)* | major |
| `m` `min` `-` | minor |
| `dim` `o` | diminished |
| `aug` `+` | augmented |
| `sus2` | suspended second |
| `sus4` | suspended fourth |
| `5` | power chord, no third |

Aliases engrave the same as the name they alias:

```kf+
#C
VS 4
Cmin C- Co C+
```

> [!warning] `M`, `ma` and `Δ` do not mean major
> `CM7` engraves a dominant seventh and `Cma7` engraves a **minor**
> chord. Neither reports an error. Write `maj`.
> ([issue #10](https://github.com/FastTrackStudios/keyflow/issues/10))

See also: [Quality](/guide/chord-quality), [[extensions|Extensions]]

---

Up: [Quality](/guide/chord-quality)
