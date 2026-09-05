---
title: Chords
kind: concept
type: concept
order: 3
stage: Start here
---

# Chords

Write the root, and add to it only what the chord actually has.

```kf+
Sunday Morning - The Wandering
4/4 #C 120bpm

VS 4
C F Am G
```

## The four parts

A chord symbol is built in this order, and every part after the root is optional:

| Part            | Example in `Cmaj7#11`  | What it says                    |
| --------------- | ---------------------- | ------------------------------- |
| **Root**        | `C`                    | which note the chord is built on |
| **Quality**     | `maj`                  | major, minor, diminished…        |
| **Extension**   | `7`                    | how far up the stack it goes     |
| **Alteration**  | `#11`                  | a note moved out of the scale    |

The root is the only part you must write. `C` is a chord; so is `Cm`; so is `Cm7`; so is `Cm7b5`.

## Sevenths and colour

```kf+
Sunday Morning - The Wandering
4/4 #C 120bpm

VS 4
Cmaj7 Dm7 G7 Cmaj7
```

## Slash chords

A `/` puts a different note in the bass:

```kf+
Sunday Morning - The Wandering
4/4 #C 120bpm

VS 4
C G/B Am F
```

> [!important] `/` means something else in Roman numerals
> In letters and numbers a slash is a bass note. In Roman numerals it writes a **secondary chord** — `V/V` is "five of five", the dominant of the dominant, which in C is D. That is what the notation already means to the people who read it, so Keyflow does not fight it.

```kf+
Secondary - Demo
4/4 #C 120bpm

VS 4
I V/V IV V
```

## Numbers and the colon

When a chord is written with numbers, the root and the quality can run together and get hard to read — `57` could be a fifty-seven. A colon separates them:

```kf+
Colon - Demo
4/4 #C 120bpm

VS 4
1:maj7 2m 3m 5:7
```

The colon is purely for reading, and it does not survive into the chart: `1:maj7` and `1maj7` parse identically and both engrave as `1maj7`. Use it where it earns its keep and skip it where it does not — `2m` and `6m9` read fine without one.

---

Previous: [[structure|Structure]] · Next: [[rhythm|Rhythm]] · Up: [[introduction|An Introduction]]
