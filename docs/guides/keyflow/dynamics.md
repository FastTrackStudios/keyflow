---
title: Dynamics
kind: concept
type: concept
order: 7
stage: For the band
---

# Dynamics

By this point the chart is correct: the right chords, in the right bars, in the
right form. Dynamics are what make it *musical* — the difference between a chart
a band can read and a chart a band can play.

Keyflow keeps two separate things separate, because they are read by different
people for different reasons.

## Cues — what the band is told

A cue is free text on its own line. It attaches to the bar that follows:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 4
<Build>
1 4 5 1
```

Anything you would shout across a rehearsal room goes here — `<Build>`,
`<Drop out>`, `<Guitar only>`, `<Go Crazy>`. Multiple words are fine. There is no
vocabulary to learn because there is no vocabulary: it prints what you wrote.

Pin one to a beat with `:`:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 4
<Hit>:3
1 4 5 1
```

## Dynamics — what the page says

The engraved dynamic — the italic `p`, `mf`, `ff` under the staff — is a `dyn`
line:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 4
dyn mf
1 4 5 1
```

The levels are the standard ones: `ppp` `pp` `p` `mp` `mf` `f` `ff` `fff`, plus
`sf`, `sfz` and `fp`.

Add `@beat` to place it inside the bar, and `above` or `below` to choose a side:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 4
dyn p@3 above
1 4 5 1
```

## Hairpins

A crescendo or diminuendo is a `hairpin` line with a beat span:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 4
hairpin < 1..4
1 4 5 1
```

`<` opens out, `>` closes in. `crescendo`, `cresc`, `dim` and `diminuendo` all
work in place of the symbols if you prefer words, and a trailing `above` or
`below` sets placement here too.

## Which one to use

They stack — a bar can carry both — but they are not interchangeable.

| | Cue | Dynamic |
| --- | --- | --- |
| `<Build>` | ✅ | |
| `dyn mf` | | ✅ |
| Read by | the band, in rehearsal | the reader, on the page |
| Vocabulary | anything you type | the standard levels |

If you are telling people what to *do*, it is a cue. If you are marking how loud
the music *is*, it is a dynamic. Most rhythm charts want cues and little else;
reach for `dyn` when the chart is going to be read rather than talked through.

---

Previous: [[key-meter-changes|Key & Meter Changes]] · Next: [[annotations|Annotations]] · Up: [[lifecycle|The Life of a Chart]]
