---
title: Rhythmic Slash Notation
kind: concept
type: concept
order: 13
stage: Rhythm
summary: One slash per beat, written after the chord.
---

# Rhythmic Slash Notation

One slash per beat, after the chord. [[rhythm|Rhythm]] has the one-line version; this is the rest.

## Two chords in a bar

```kf+
G /// C
```

Three beats of G, one of C. The bar is full, so the next chord starts the next bar.

> [!warning] Slashes are a duration, not a separator
> `G // C //` is two chords of two beats: one bar. Read as "play G, then C" it surprises you with a half-empty section. If the count is wrong, Keyflow says what it counted.

## Mixed lengths across bars

```kf+
G /// C / Em // D //
```

Bar one is three beats of G and one of C; bar two splits between Em and D.

## A chord with no slashes

Nothing after a chord means a whole bar, so you only write slashes where a bar holds more than one chord.

```kf+
G // C // Am
```

Two chords share bar one; Am holds bar two. That is why most of a chart carries no rhythm notation.

## Mixing with note values

Slashes and [[lilypond-rhythm|note values]] mix inside one chart.

```kf+
VS 4
G_2 C // D_1
```

See also: [[lilypond-rhythm|LilyPond Rhythm Notation]] · [[chords|Chords]]

---

Previous: [[rhythm|Rhythm]] · Next: [[lilypond-rhythm|LilyPond Rhythm Notation]] · Up: [[rhythm|Rhythm]]
