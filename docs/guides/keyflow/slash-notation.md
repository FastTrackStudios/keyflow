---
title: Rhythmic Slash Notation
kind: concept
type: concept
order: 6
stage: Rhythm
summary: One slash per beat, written after the chord.
---

# Rhythmic Slash Notation

One slash per beat, after the chord.

## One slash per beat

```kf+
G // C // D // Em //
```

Two slashes, two beats — so in 4/4 two chords fill a bar.

> [!warning] Slashes are a duration, not a separator
> `G // C //` is two chords of two beats: one bar. Read as "play G, then C" it surprises you with a half-empty section. If the count is wrong, Keyflow says what it counted.

## Uneven bars

Lengths need not match.

```kf+
G /// C / Em
```

Three beats of G, one of C, then Em takes the next bar.

## A chord with no slashes

Nothing after a chord means a whole bar.

```kf+
G // C // Am
```

Two chords share bar one; Am holds bar two.

## Mixing with note values

```kf+
VS 4
G_2 C // D_1
```

See also: [[lilypond-rhythm|LilyPond Rhythm Notation]], the other way to write a duration · [[chords|Chords]]

---

Previous: [[rhythm|Rhythm]] · Next: [[lilypond-rhythm|LilyPond Rhythm Notation]] · Up: [[rhythm|Rhythm]]
