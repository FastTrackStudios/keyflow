---
title: Key & Meter Changes
kind: concept
type: concept
order: 6
stage: The music
---

# Key & Meter Changes

Most songs pick a key and a meter in the [[header|header]] and never move. When
one does move, Keyflow's rule is the same as everywhere else: say it once, where
it happens, and everything downstream follows.

## Changing key

A key on a section header takes effect from that section on:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 4
1 4 5 1

CH 4 #G
1 4 5 1
```

The key signature on the staff changes, and — because those chords are
[[notation-systems|Nashville numbers]] — the same `1 4 5 1` now means something
different. That is the point. A song that lifts a whole step is one edit, not a
rewrite of every chord after it.

Write the chorus in letter names instead and you would have to retype all four
bars, then retype them again the next time the key moved. Numbers are what keeps
the key in one place.

## Changing meter

Meter changes go on the music line, not the section header, and they take a `T`
prefix:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 4
G D T6/8 Am
```

From that bar on, the chart is in 6/8.

### Why the `T`

Because `6/8` on its own is already a chord — a `6` chord over a `8` bass. The
parser cannot tell a meter from a slash chord by shape alone, so it does not
guess. Without the prefix the token is read as music and your meter change
silently does not happen:

```kf
VS 4
G D 6/8 Am
```

That chart stays in 4/4. `T` is what makes it a meter.

### One bar only

A `!` in front means *this bar only*, then back to what it was:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 4
G D !T2/4 Am
```

Bar three is 2/4; bar four is 4/4 again. This is the common case — a single
clipped bar going into a chorus — and it is worth reaching for before you write
two meter changes to fake it.

| Write | Meaning |
| --- | --- |
| `T6/8` | 6/8 from here on |
| `!T2/4` | one bar of 2/4, then back |
| `6/8` | a chord, not a meter |

---

Previous: [[rhythm|Rhythm]] · Next: [[dynamics|Dynamics]] · Up: [[lifecycle|The Life of a Chart]]
