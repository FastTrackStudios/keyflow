---
title: Annotations
kind: concept
type: concept
order: 8
stage: For the band
---

# Annotations

Everything left over: the words on the page that are not chords, not lyrics and
not [[dynamics|dynamics]]. Notes to a player, a groove description, a repeat, an
ending.

## A note on a section

Quotes on a [[structure|section header]] ride along with the section and print
under its card:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

CH 4 "big finish"
1 4 5 1
```

This is the right place for something that describes the whole section — *half
time*, *no drums*, *acoustic only*.

## Text on a bar

Quotes on a music line attach to the bar they sit in front of:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 4
"Ac. Gtr. groove" 1 4 5 1
```

By default it prints below the staff. `^` puts it above, `_` puts it below —
useful when a bar carries two notes meant for two different people:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 4
^"lift" 1 _"pad" 4 5 1
```

Above the staff reads as an instruction to the whole band; below reads as detail
about the part. That is convention, not enforcement — but following it means a
player can scan one side of the staff and find what is theirs.

## Repeats and endings

Repeat barlines are `|:` and `:|`:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 4
|: 1 | 4 | 5 | 1 :|
```

Alternate endings are numbered in brackets:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

VS 4
|: 1 | 4 | [1] 5 :| [2] 1 |
```

A repeat is worth reaching for when the music is genuinely identical. When it is
not — when the second time round changes one chord — write the bars out. A chart
whose repeats need a paragraph of explanation is slower to read than one that
just says what happens.

## What this is for

None of this changes a note. That is exactly the point: a chart is read by a
person under stage light with one pass to get it right, and the annotations are
what make the difference between a chart that is *correct* and a chart that is
*playable*.

---

Previous: [[dynamics|Dynamics]] · Up: [[lifecycle|The Life of a Chart]]
