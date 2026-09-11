---
title: Reusing a Progression
kind: concept
type: concept
order: 6
stage: Chart
summary: Write the chords once. Most songs only have a few.
---

# Reusing a Progression

Most songs repeat themselves harder than they look. *Creep* is four chords for three and a half minutes; the rest is form. Writing those four chords out eight times buries the one thing the chart is actually telling you — the shape.

Three ways to say "the same again", from smallest to largest.

## An empty section repeats its own kind

A section header with nothing under it is the last section of that type.

```kf+
VS 4
G B C Cm

CH 4
Cm C G B

VS 4

CH 4
```

Verse two and chorus two are the same chords as verse one and chorus one. Nothing to keep in sync, because there is only one copy.

This is on for the section types a song has several of. Intro, Outro, Pre and Post are left out: those are the ones a song usually has exactly one of, and where a second exists it is more often a variation than a repeat, so copying it silently would be a surprise.

## `= OTHER` borrows from a different section

The intro is often the verse. The instrumental before the bridge is usually the bridge. Name the section you mean:

```kf+
VS 8
G B C Cm G B C Cm

IN 4 = VS

BR 4
Am F C G

INST 4 = BR
```

The name after `=` is an ordinary section name, so `= [Guitar Solo]` works for a custom one. Because it is explicit, it borrows from any section type, including the four that empty headers leave alone — `IN 8 = IN` is how you say the second intro really is the first one again.

## `\progression` sets a chart-wide default

When the whole song is one progression, say it once at the top. Any section that names no chords and has nothing to recall falls back to it.

```kf+
Creep - Radiohead
4/4 #G

\progression G B C Cm

IN 4
VS 8
CH 4
BR 4
OUT 4
```

The progression repeats to fill the section, so a four-chord default under an eight-bar verse goes round twice. Everything else still works normally — a section that writes its own chords uses those, and one that says `= VS` gets the verse.

> [!tip] They stack
> The three are tried in order: an explicit `= OTHER` first, then the section's own kind, then `\progression`. So a chart can lean on the default for most of its form and still spell out the bridge.

See also: [[structure|Structure]] · [[repeats|Repeats]] · [[cheatsheet|Cheatsheet]]

---

Previous: [[structure|Structure]] · Next: [[chords|Chords]] · Up: [[introduction|An Introduction]]
