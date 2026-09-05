---
title: Rhythmic Slash Notation
kind: concept
type: concept
order: 6
stage: Rhythm
summary: One slash per beat, written after the chord.
---

# Rhythmic Slash Notation

One slash per beat, written after the chord. It is the system to reach
for when you are thinking in beats — which is most of the time on a
stand, because beats are what a player counts.

## One slash per beat

```kf+
G // C // D // Em //
```

Two slashes is two beats, so in 4/4 two chords fill a bar and these four
fill two.

> [!warning] Slashes are a duration, not a separator
> `G // C //` is two chords of two beats — one bar. It is easy to read
> them as "play G, then C" and be surprised by a half-empty section. If
> the bar count does not add up, Keyflow will tell you what it counted.

## Uneven bars

Lengths do not have to match. A chord can hold most of a bar while its
neighbours split the rest:

```kf+
G /// C / Em
```

Three beats of G, one of C, and Em takes the bar that follows.

## A chord with no slashes

A chord with nothing after it lasts a whole bar, so you only write
slashes where a bar holds more than one chord:

```kf+
G // C // Am
```

Two chords sharing the first bar, then Am holding the second. That is
why most of a chart needs no rhythm notation at all.

## Mixing with note values

Slashes and [[lilypond-rhythm|note values]] mix inside one chart, so a
bar that is easier to name than to count can be written the other way:

```kf+
VS 4
G_2 C // D_1
```

See also: [[lilypond-rhythm|LilyPond Rhythm Notation]], the other way to write a duration · [[chords|Chords]]

---

Previous: [[rhythm|Rhythm]] · Next: [[lilypond-rhythm|LilyPond Rhythm Notation]] · Up: [[rhythm|Rhythm]]
