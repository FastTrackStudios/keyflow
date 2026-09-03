---
title: Chords
kind: concept
type: concept
order: 3
stage: The music
---

# Chords

Chords go under the section that owns them, one per bar unless you say
otherwise.

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 4
E A B E
```

## Three ways to name a chord

The same progression, three ways. Pick whichever fits the chart — they are
interchangeable, and a chart can mix them.

Letter names say the chord outright:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 4
E A B E
```

Nashville numbers say the chord's place in the key, so the chart transposes by
changing one token in the header:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 4
1 4 5 1
```

Roman numerals say the same thing in the analyst's dialect:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 4
I IV V I
```

Numbers and numerals are **relative to the key**, which is what makes the key
token in the [[header|header]] more than a signature: change `#E` to `#G` and
every number moves with it.

## Quality and colour

Everything you would write on a chord chart works on the root, in any of the
three systems:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 8
Emaj7 C#m7 F#m9 B7
Amaj9 G#m7b5 C#7#9 Bsus4
```

The same in numbers:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 4
1maj7 6m7 2m9 5
```

## Slash bass

A `/` names the note in the bass:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 4
E B/D# C#m A
```

---

Previous: [[structure|Structure]] · Next: [[notation-systems|Notation Systems]] · Up: [[lifecycle|The Life of a Chart]]
