---
title: Structure
kind: concept
type: concept
order: 2
stage: Start here
---

# Structure

A section is a name and a number of bars. Write those and you have the form.

```kf+
Sunday Morning - The Wandering
4/4 #G 120bpm

IN 2
VS 8
CH 4
```

The number is how many bars the section runs, and it is not decoration: Keyflow holds you to it. Chords that do not fill the section fail the parse with what it counted, rather than quietly engraving a form you did not mean.

## Naming a section

Use the short name. Most are what you would guess.

| Section | Short | Section      | Short  |
| ------- | ----- | ------------ | ------ |
| Intro   | `IN`  | Instrumental | `INST` |
| Verse   | `VS`  | Interlude    | `INT`  |
| Chorus  | `CH`  | Solo         | `SOLO` |
| Bridge  | `BR`  | Outro        | `OUT`  |

Case does not matter — `vs 8` and `VS 8` are the same.

## Music under a section

Put the chords on the line below:

```kf+
Sunday Morning - The Wandering
4/4 #G 120bpm

VS 4
G C Em D
```

> [!warning] A section header holds no music
> `VS 4 G C Em D` will not work. The header line names the section and counts its bars; the music goes on its own line underneath.

## Replaying a section

Write a section once, then name it again with nothing under it to play it back:

```kf+
Sunday Morning - The Wandering
4/4 #G 120bpm

VS 4
1 4 5 1

CH 4
4 1 5 1

VS
CH
```

So a whole song is mostly its section list: lay out `VS`, `CH`, `BR` once, then order the repeats however the song goes. The replays come back numbered — *Verse 2*, *Chorus 2* — counted across the chart for you.

---

Previous: [[header|Header]] · Next: [[chords|Chords]] · Up: [[introduction|An Introduction]]
