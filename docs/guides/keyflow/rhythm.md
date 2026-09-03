---
title: Rhythm
kind: concept
type: concept
order: 5
stage: The music
---

# Rhythm

A bare chord fills its bar. That is the whole default, and it is why most charts
need nothing on this page.

```kf+
One Per Bar - Demo
4/4 #C

VS 4
C F G C
```

Four chords, four bars. You reach for the rest of this page only when a bar
holds more than one chord, or when the band has to catch something together.

## Splitting a bar

A `/` is one beat of the chord before it. So `C // F //` is two beats of C and
two of F, in one bar:

```kf+
Split - Demo
4/4 #C

VS 2
C // F //
G / Am / F / G /
```

The slashes are how the chart *looks* on the stand, too — they are the rhythm
marks a player reads.

## Grouping with ( )

Parentheses hold several chords inside one bar, sharing it evenly:

```kf+
Grouped - Demo
4/4 #C

VS 2
(C F) (G Am)
```

Two chords to a bar, two beats each. Grouping says "these belong to one bar"
without counting slashes.

## Exact note values

An underscore gives a chord an explicit duration — `_2` is a half note, `_1` a
whole, `_4` a quarter, `_8` an eighth:

```kf+
Durations - Demo
4/4 #C

VS 2
C_2 F_2 | G_1
```

A duration sticks until something changes it, so a run of eighths does not need
marking on every chord.

## Hits and stops

The reason a chart beats a chord sheet: the punches everyone has to land
together.

A `>` marks an accent — the chord is hit, not strummed through:

```kf+
Hits - Demo
4/4 #C

VS 2
>C // >F //
```

An `s` and a duration is a stop: `s1` holds a whole bar of silence after the
hit, so the band lands and leaves it.

```kf+
Stops - Demo
4/4 #C

VS 2
>C // s1
```

Hits and stops are step four of the [[lifecycle|life of a chart]] — the first
thing you add once the harmony is right, and often the last thing a working
chart needs.

---

Previous: [[notation-systems|Notation Systems]] · Next: [[key-meter-changes|Key & Meter Changes]] · Up: [[lifecycle|The Life of a Chart]]
