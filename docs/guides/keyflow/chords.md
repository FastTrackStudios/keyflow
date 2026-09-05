---
title: Chords
kind: concept
type: concept
order: 3
stage: Chords
---

# Chords

Write the root. Add only what the chord has.

```kf+
C F Am G
```

## The four parts

| Part | In `Cmaj7#11` | What it says |
| --- | --- | --- |
| **Root** | `C` | the note it is built on |
| **Quality** | `maj` | major, minor, diminished… |
| **Extension** | `7` | how far up the stack |
| **Alteration** | `#11` | a note moved out of the scale |

Only the root is required. `C`, `Cm`, `Cm7`, `Cm7b5` are all chords.

```kf+
Cmaj7 Dm7 G7 Cmaj7
```

## Slash chords

`/` puts a note in the bass.

```kf+
C G/B Am F
```

> [!important] `/` differs in Roman numerals
> There it writes a **secondary chord**. `V/V` is "five of five" — the dominant of the dominant, D in C. Keyflow follows what the notation already means.

```kf+
4/4 #C 120bpm

VS 4
I V/V IV V
```

## Numbers and the colon

`57` could be a fifty-seven. A colon separates root from quality.

```kf+
4/4 #C 120bpm

1:maj7 2m 3m 5:7
```

It is for reading only — `1:maj7` and `1maj7` parse identically and both engrave as `1maj7`. Skip it where it earns nothing: `2m`, `6m9`.

See also: [[rhythm|Rhythm]] for how long each chord lasts, and [[appendix-sections|All Section Names]] for what they sit under.

---

Previous: [[structure|Structure]] · Next: [[rhythm|Rhythm]] · Up: [[introduction|An Introduction]]
