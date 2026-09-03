---
title: Structure
kind: concept
type: concept
order: 2
stage: Start here
---

# Structure

A section is a named part of the song and a number of bars. Write those and you
have the form — the frame everything else hangs on.

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

IN 2
VS 8
CH 4
```

The number is how many bars that section runs. It is not decoration: Keyflow
holds you to it. Write chords that do not fill the section and the parse fails
with what it counted, rather than engraving a form you did not mean.

## Naming a section

Use the common short name — most are what you would guess.

| Section     | Short | Section      | Short |
| ----------- | ----- | ------------ | ----- |
| Intro       | `IN`  | Instrumental | `INST`|
| Verse       | `VS`  | Interlude    | `INT` |
| Chorus      | `CH`  | Solo         | `SOLO`|
| Bridge      | `BR`  | Outro        | `OUT` |

Case does not matter — `vs 8` and `VS 8` are the same.

## Replaying a section

Write a section's music once, then name it again with nothing under it to play
it back:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 4
1 4 5 1

CH 4
4 1 5 1

VS
CH
```

So a whole song is mostly its section list: lay out `VS`, `CH`, `BR` once, then
order the repeats however the song goes. Notice the numbering — the replays come
back as *Verse 2* and *Chorus 2*, counted across the chart for you.

## A note on a section

Anything in quotes on the header rides along with the section and prints under
its card:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

CH 4 "big finish"
4 1 5 1
```

A cue, an instruction, a reminder — whatever the band needs to see at that
moment.

## Changing key at a section

A key written on the header takes effect from that section on, and the key
signature changes with it. See [[key-meter-changes|Key & Meter Changes]].

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 4
1 4 5 1

CH 4 #G
1 4 5 1
```

Because the chords are Nashville numbers, the same `1 4 5 1` means something
different in the new key — which is usually exactly what you want when a song
lifts.

---

Previous: [[header|Header]] · Next: [[chords|Chords]] · Up: [[lifecycle|The Life of a Chart]]
