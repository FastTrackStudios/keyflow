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

So `2` *is* the minor two. Writing `2m` is allowed and means the same thing —
it is simply more explicit, which is what Keyflow itself writes when it
converts a chart into numbers.

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

### What Keyflow writes back

Reading and writing are not the same job. Going in, a bare number is enough and
you are never made to spell out the obvious. Coming out — when you switch a
letter chart to numbers — Keyflow says which chord it means, because that chart
is about to be read at speed:

| Chord in C | Written back as |
| --- | --- |
| `Am` | `6m` |
| `A`  | `6M` |
| `Bb` | `b7` |

`6m` is the one that could have been left bare; it is spelled out anyway, so a
player does not have to work out the sixth degree and recall that it is minor.

`6M` is the one that *must* be spelled out. A bare `6` means the diatonic vi, so
writing an A major triad as `6` would read back as `Am` — the round trip would
quietly reharmonise the song. The marker is skipped where the quality is already
unambiguous: a chromatic degree like `b7` has no diatonic quality to contradict,
and `6maj7` already says major without needing `6Mmaj7`.

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

## Raised and lowered degrees

A `#` or a `b` before a degree or numeral raises or lowers it, which is how you
write a chromatic chord:

```kf+
Accidentals - Demo
4/4 #C

VS 4
1 #4 b7 1
```

In C that is C, F♯, B♭, C — `b7` is the flat seventh degree, the borrowed chord
that makes a mixolydian turnaround.

Lowercase `b` before a digit is always a flat, even in a chart that is
otherwise all letter names:

```kf+
Accidentals - Demo
4/4 #C

VS 4
C b3 F G
```

The note B is written `B`, uppercase — that is what leaves the lowercase form
free to mean "flat". `B`, `B7` and `Bb` are all still the note.

## Slashes mean different things

A `/` is a slash bass in letter names and in numbers — the chord on top, the
bass note underneath:

```kf+
Slashes - Demo
4/4 #C

VS 4
C G/B Am F
```

In Roman numerals it is not. There, `/` writes a **secondary chord** — `V/V` is
"five of five", the dominant of the dominant:

```kf+
Slashes - Demo
4/4 #C

VS 4
I V/V IV V
```

In C, `V/V` is D: take the fifth degree (G), treat it as a temporary tonic, and
play *its* fifth. `V/vi` is E, `V/ii` is A. Add a seventh and it behaves as you
would expect — `V7/V` is D7.

This is not an inconsistency to work around; it is what the two systems already
mean to the people reading them. Roman numerals describe *function*, so a slash
between two of them describes a function applied to a degree. Letters and
numbers name *chords*, so a slash between them names a chord over a bass. If
you want an inversion in Roman numerals, write the figure after a `^` — `V^6`
for the first inversion — rather than a slash.

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
