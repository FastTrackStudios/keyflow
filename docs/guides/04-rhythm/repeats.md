---
title: Repeats
kind: concept
type: concept
order: 15
stage: Rhythm
summary: Two marks for "again" — one counts bars, one counts whatever you last wrote.
---

# Repeats

Charts repeat. Writing the same bar out four times reads worse than saying "again" three times, and it hides the change when one finally comes.

Keyflow has two marks for it. They look similar and mean different things.

## `%` — the simile mark

`%` is one more measure of the last one.

```kf+
VS 4
G % % %
```

Four bars of G. The mark says *a measure*, so it advances a whole measure however the previous one was written — a bar with four chords in it repeats all four.

```kf+
VS 3
G // C // % %
```

Three bars, each one G then C.

### `%3` — counted

`%3` is three simile marks. `%` on its own is `%1`.

```kf+
VS 8
G %3 C %3
```

Four bars of G, then four of C.

## `.` — the direct repeat

`.` repeats the last chord *for as long as that chord lasted*. It is not measure-bounded, which is the whole difference.

When the chord filled a bar, a dot is a bar:

```kf+
VS 4
G . . .
```

When the chord was two beats, a dot is two beats:

```kf+
VS 2
G // . . .
```

Four units of two beats — two bars, not four. This catches people out, so it is worth saying plainly: `%` counts bars, `.` counts whatever you last wrote.

It follows [[lilypond-rhythm|note values]] the same way:

```kf+
VS 1
G_8 . . . . . . .
```

Eight eighths — one bar.

### `.3` — counted

`.3` is three dots, the same way `%3` is three simile marks.

```kf+
VS 2
G // .3
```

> [!warning] `.3` is off in a scale-degree chart
> `.` is also the staccato prefix (`.C`), so in a chart written in [[notation-systems|scale degrees]] `.3` is staccato on the third — a real chord — as much as it is three repeats. The notation system decides, the same way it decides `b3` is a flat degree and not the note B. In a degree chart, write the dots out: `1 . . .`

## Which one to reach for

Reach for `%` when the unit you are repeating is a bar, which is most of the time. Reach for `.` when it is not — a two-beat vamp, a stab on every eighth.

| | repeats | counted form |
| --- | --- | --- |
| `%` | a whole measure | `%3` |
| `.` | the last chord's own length | `.3` |

See also: [[rhythm|Rhythm]] · [[slash-notation|Rhythmic Slash Notation]] · [[cheatsheet|Cheatsheet]]

---

Previous: [[lilypond-rhythm|LilyPond Rhythm Notation]] · Next: [[writing-a-chart|Writing a Chart]] · Up: [[rhythm|Rhythm]]
