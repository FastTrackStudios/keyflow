---
title: Rhythm
kind: concept
type: concept
order: 5
stage: Rhythm
summary: A chord lasts one bar unless you say otherwise.
---

# Rhythm

A chord with nothing after it lasts one bar. That covers most of a chart,
and it is why the examples so far have said nothing about rhythm at all.

```kf+
G C Em D
```

Four chords, four bars.

When a bar holds more than one chord you have to say how long each lasts,
and Keyflow gives you two ways to say it. Most of the time you want the
first one.

## Slashes, in one line

One slash per beat, written after the chord:

```kf+
G // C // D // Em //
```

Two beats each, so two chords fill a bar. That is the whole idea, and it
covers nearly every chart — [[slash-notation|Rhythmic Slash Notation]]
has the rest: uneven bars, mixed lengths, and what happens when the
count does not add up.

## Note values, in one line

The other way names the duration instead of counting beats — `_2` for a
half, `_4` for a quarter:

```kf+
VS 4
G_2 C_2 D_2 Em_2
```

Reach for it when the figure is easier to name than to count. See
[[lilypond-rhythm|LilyPond Rhythm Notation]].

## Which to use

| | Counts | Reads as |
|---|---|---|
| Slashes | beats | what a player counts |
| Note values | durations | what a score names |

Use whichever matches how you are thinking about the bar. They mix
freely inside one chart.

---

Previous: [[chords|Chords]] · Next: [[slash-notation|Rhythmic Slash Notation]] · Up: [[introduction|An Introduction]]
