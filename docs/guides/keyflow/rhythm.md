---
title: Rhythm
kind: concept
type: concept
order: 4
stage: Start here
---

# Rhythm

A chord with nothing after it lasts one bar. That covers most of a chart, and
it is why the simple examples so far said nothing about rhythm at all.

```kf+
Sunday Morning - The Wandering
4/4 #G 120bpm

VS 4
G C Em D
```

Four chords, four bars.

## Rhythmic slash notation

When a bar holds more than one chord, say how long each lasts with slashes —
one slash per beat:

```kf+
Sunday Morning - The Wandering
4/4 #G 120bpm

VS 2
G // C // D // Em //
```

Two beats each, so two chords fill a bar and four chords fill the section's
two bars.

> [!warning] Slashes are a duration, not a separator
> `G // C //` is two chords of two beats — one bar. It is easy to read them
> as "play G, then C" and be surprised by a half-empty section. If the bar
> count does not add up, Keyflow will tell you what it counted.

Mix them freely. A chord can hold a bar while its neighbours split one:

```kf+
Sunday Morning - The Wandering
4/4 #G 120bpm

VS 2
G /// C / Em
```

## Durations by note value

The other way is to name the note value, LilyPond-style: `_2` is a half note,
`_4` a quarter, `_1` a whole bar.

```kf+
Sunday Morning - The Wandering
4/4 #G 120bpm

VS 2
G_2 C_2 D_2 Em_2
```

The two systems do the same job from opposite ends — slashes count beats,
underscores name values. Use whichever matches how you are thinking, and
mix them if you like:

```kf+
Sunday Morning - The Wandering
4/4 #G 120bpm

VS 2
G_2 C // D_1
```

---

Previous: [[chords|Chords]] · Up: [[introduction|An Introduction]]
