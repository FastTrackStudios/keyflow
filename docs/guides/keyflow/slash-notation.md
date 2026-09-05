---
title: Rhythmic Slash Notation
kind: concept
type: concept
order: 6
stage: Rhythm
summary: One slash per beat, written after the chord.
---

# Rhythmic Slash Notation

One slash per beat, written after the chord:

```kf+
G // C // D // Em //
```

Two beats each, so two chords fill a bar.

> [!warning] Slashes are a duration, not a separator
> `G // C //` is two chords of two beats — one bar. It is easy to read
> them as "play G, then C" and be surprised by a half-empty section. If
> the bar count does not add up, Keyflow will tell you what it counted.

Mix lengths freely. A chord can hold most of a bar while its neighbours
split the rest:

```kf+
G /// C / Em
```

This is the system to reach for when you are thinking in beats — which is
most of the time on a stand, because it is what a player counts.

See also: [[lilypond-rhythm|LilyPond Rhythm Notation]], the other way to write a duration · [[chords|Chords]]

---

Previous: [[rhythm|Rhythm]] · Next: [[lilypond-rhythm|LilyPond Rhythm Notation]] · Up: [[rhythm|Rhythm]]
