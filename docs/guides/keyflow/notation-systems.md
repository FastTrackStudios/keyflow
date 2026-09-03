---
title: Notation Systems
kind: concept
type: concept
order: 4
stage: The music
---

# Notation Systems

The same music can be written three ways, and they are interchangeable.
Everything *after* the root — quality, sevenths, extensions, slash bass — is
identical in all three. The only thing you are choosing is how to spell the
root.

| System           | Four bars     | The root means            |
| ---------------- | ------------- | ------------------------- |
| Letter names     | `C  F  Am  G` | an absolute pitch         |
| Nashville numbers| `1  4  6  5`  | a scale degree in the key |
| Roman numerals   | `I  IV  vi  V`| a scale degree in the key |

All three engrave the same chart:

```kf+
Four Bars - Letter Names
4/4 #C

VS 4
C F Am G
```

```kf+
Four Bars - Nashville
4/4 #C

VS 4
1 4 6 5
```

```kf+
Four Bars - Roman
4/4 #C

VS 4
I IV vi V
```

## Letter names

Absolute pitch. `C` is C in every key, and the quality is written out.

```kf+
Letters - Demo
4/4 #C

VS 4
Cmaj7 F#m7b5 Bb Am
```

Use them when the chart is for one key and always will be — a lead sheet, a
transcription, anything you would hand to a reader who is not transposing.

## Nashville numbers

The root is a scale degree, `1`–`7`, relative to the key. **A bare number takes
the key's own quality**, so you do not write the `m`:

| Degree in a major key | `1` | `2` | `3` | `4` | `5` | `6` | `7` |
| --------------------- | --- | --- | --- | --- | --- | --- | --- |
| Quality               | maj | min | min | maj | maj | min | dim |
| In C                  | C   | Dm  | Em  | F   | G   | Am  | B°  |

So `2` *is* the minor two — writing `2m` is allowed but says nothing extra.

```kf+
Diatonic - Demo
4/4 #C

VS 8
1 2 3 4
5 6 7 1
```

The point of numbers is that the chart is about the *progression*, not the
pitches. Change the key in the [[header|header]] and every chord follows — this
is the same file as above with one token changed:

```kf+
Diatonic - Demo
4/4 #G

VS 8
1 2 3 4
5 6 7 1
```

### Overriding the quality

Two ways to say "not the diatonic one":

- `!` before the number makes it **literal** — a plain major triad, ignoring the
  key. In C, `!2` is D.
- An **explicit quality** wins: `2M` for major, `2m` for minor, and the rest —
  `2dim`, `2aug`, `2sus4`.

```kf+
Overrides - Demo
4/4 #C

VS 4
2 !2 2M 2m
```

That is Dm, D, D, Dm — the diatonic default, then three ways of departing from
or restating it.

## Roman numerals

Also a scale degree, but **case carries the quality**: uppercase is major,
lowercase is minor.

```kf+
Roman - Demo
4/4 #C

VS 8
I ii iii IV
V vi vii I
```

Descriptors stack on top as usual — `Imaj7`, `V7`, `iim7`.

Roman numerals suit analysis and anything with a classical accent. Numbers suit
a chart a band will transpose on the stand. They resolve identically; pick the
dialect your reader speaks.

## The `:` separator

When a quality begins with a digit, two runs of digits collide — is `17` degree
one with a seventh, or the number seventeen? A colon separates them:

```kf+
Colon - Demo
4/4 #C

VS 4
1:7 4:maj9 2:m7 5:9
```

The colon is **purely for reading**. `1:7` and `17` parse identically, and the
chart engraves the same either way. It works in all three systems — `V:7`,
`C:7` — though numbers are where it earns its keep.

Write it. `1:7` reads at a glance; `17` is correct and easy to misread.

## Sharpened degrees

A `#` before a degree or numeral raises it, which is how you write a chromatic
chord:

```kf+
Sharps - Demo
4/4 #C

VS 4
1 #4 5 1
```

In C that is C, F♯, G, C.

> **Flats before a degree do not work yet.** `b3`, `b7`, `bIII` and `bVII` are
> currently read as the *note* B — `b7` engraves as B7, not as the flat-seven
> degree. Until that is fixed, write the borrowed chord by its letter name
> (`Bb` rather than `b7`), which is unambiguous in any chart.

## Mixing systems

A chart may mix them. Nothing stops a letter-name chart borrowing a numeral, or
a number chart naming one chord outright:

```kf+
Mixed - Demo
4/4 #C

VS 4
1 4 Bb 1
```

---

Previous: [[chords|Chords]] · Next: [[rhythm|Rhythm]] · Up: [[lifecycle|The Life of a Chart]]
