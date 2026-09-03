---
title: The Life of a Chart
kind: concept
type: concept
order: 0
stage: Start here
---

# The Life of a Chart

Keyflow grows with what you are writing. A chart for a song you already know
should take about as long to type as it takes to say out loud; a chart that has
to carry three key changes, a horn section and click-synced lyrics should be
able to, without making the simple one pay for it.

Almost every chart is the simple one. So the format is built so that the first
thing you type already works, and everything past it is something you add *when
the song asks for it* — never something you write to get started.

## Start anywhere, render immediately

There is no minimum. Each of these is a complete file, and each one engraves:

```kf+
Sunday Morning
```

```kf+
120bpm
```

```kf+
#E
```

A chart is not something you finish and then render. It renders from the first
token and grows under you as you type.

## The recommended order

Write a chart in any order you like. This is the one that tends to save you
backtracking, because each step gives the next one something to hang on.

### 1. Say what the song is

Title, artist, and the musical defaults — time signature, tempo, key.

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E
```

See [[header|Header]].

### 2. Lay out the form

Name the sections and say how many bars each runs. You now have the shape of the
song — and the bar counts are a claim Keyflow holds you to. If the chords you
write later do not fill a section, it says so instead of quietly engraving
something else.

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

IN 2
VS 8
CH 4
```

See [[structure|Structure]].

### 3. Add the harmony

The chords, in whichever naming suits the chart.

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E

IN 2
1 4

VS 8
1 4 5 1 1 4 5 5

CH 4
4 1 5 1
```

See [[chords|Chords]], and [[rhythm|Rhythm]] once a bar holds more than one.

### 4. Add the hits and stops

The rhythmic punches the band has to catch together — what makes a chart worth
having rather than a chord sheet. See [[rhythm|Rhythm]].

### 5. Add the melody or riff

Only what belongs on the master rhythm chart: the hook everybody plays, the line
the whole band lands on.

### 6. Add the dynamics and the cues

How loud, and who does what. See [[dynamics|Dynamics]] and
[[annotations|Annotations]].

### 7. Add the lyrics

Words under the chords, lined up with the changes.

### 8. Sync the lyrics

Per-syllable timing, for playback that follows a recording. Advanced, rarely
needed, and never in the way when it is not.

Most charts stop around step 3 or 4. That is the point.

## Two ways to write the same chart

Everything above puts the chords directly under the section that owns them. For
most charts that is the right shape — one place to look, one place to edit.

```kf+
Keep on Finding More - John Allan
118bpm 4/4 #C

VS1 4
1 6m7 5 4

CH 4
4 1 5 1
```

A chart can also be written as **lanes**: the form in one block, the harmony in
another, the words in a third.

```kf+
Keep on Finding More - John Allan
118bpm 4/4 #C
sections { VS1 · CH · VS1 }
rhythm { VS1 1 6m7 5 4
 CH 4 1 5 1 }
```

Both produce the same chart. `sections { … }` is the spine — each label in it is
played in order — and every other lane fills that spine in by name. Lanes earn
their keep when one layer is being worked on alone: reharmonising without
scrolling past lyrics, or writing words against a form that is already settled.

Either way the rule is the same: **one result, one place to edit it.** A
section's length lives in the spine and nowhere else; its chords live in one
lane and nowhere else. Nothing has to be kept in step by hand.

---

Next: [[header|Header]] · Up: [[lifecycle|The Life of a Chart]]
