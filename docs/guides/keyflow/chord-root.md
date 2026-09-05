---
title: Root
kind: concept
type: concept
order: 4
stage: Chords
summary: The note the chord is built on.
---

# Root

The note the chord is built on. The only required part.

```kf+
C F Am G
```

## Three ways to write it

| | Written | Names |
|---|---|---|
| Letters | `C` `F#` `Bb` | the pitch |
| Numbers | `1` `4` `6` | position in the key |
| Roman | `I` `IV` `vi` | position, case carries quality |

See [[introduction#Letters, numbers, or numerals|Letters, numbers, or numerals]].

## Accidentals

`#` sharp, `b` flat, after the letter.

```kf+
F# Bb C# Eb
```

## The colon

`57` could be a fifty-seven. A colon separates root from quality.

```kf+
4/4 #C 120bpm

1:maj7 2m 3m 5:7
```

Reading only — `1:maj7` and `1maj7` parse identically. Skip it where it earns nothing: `2m`, `6m9`.

## Slash chords

`/` puts a different note in the bass.

```kf+
C G/B Am F
```

> [!important] `/` differs in Roman numerals
> There it writes a **secondary chord**. `V/V` is "five of five" — the dominant of the dominant, D in C.

```kf+
4/4 #C 120bpm

VS 4
I V/V IV V
```

See also: [[chord-quality|Quality]], [[chords|Chords]]

---

Previous: [[chords|Chords]] · Next: [[chord-quality|Quality]] · Up: [[chords|Chords]]
