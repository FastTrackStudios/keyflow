---
title: Header
kind: concept
type: concept
order: 1
stage: Start here
---

# Header

The header says what the song *is*: what it is called, who wrote it, and the
three musical defaults everything else is measured against.

## The title line

The first line of text is the title.

```kf+
Sunday Morning
```

Bare words are read as a title rather than as music, because nothing else in
Keyflow looks like that — a chord, a meter, a tempo and a key each have a shape,
and a line of ordinary words has none of them.

A line that *opens* with a dash credits somebody and names nothing:

```kf+
- The Wandering
```

Put both on one line and the dash separates them:

```kf+
Sunday Morning - The Wandering
```

Text in parentheses becomes a subtitle:

```kf+
Sunday Morning (Live) - The Wandering
```

- Text before ` - ` is the **title**; text after it is the **artist**.
- A line starting `- ` is the **artist** alone.
- Text in `(parentheses)` is the **subtitle**.

## The metadata line

The next line sets the musical defaults. Up to three tokens, space-separated, in
any order:

```kf+
Sunday Morning - The Wandering
4/4 120bpm #E
```

| Token  | Means          | Examples                    |
| ------ | -------------- | --------------------------- |
| `N/D`  | Time signature | `4/4`, `6/8`, `3/4`, `12/8` |
| `Nbpm` | Tempo, in BPM  | `120bpm`, `68bpm`           |
| `#Key` | Key            | `#C`, `#Gm`, `#Eb`, `#F#`   |

`68bpm 4/4 #G` and `4/4 #G 68bpm` are the same line.

### Each token stands alone

Every token is optional, and each draws something by itself. A meter opens the
first bar and puts its time signature in it:

```kf+
4/4
```

A tempo marks the tempo:

```kf+
120bpm
```

A key opens the first bar with its key signature, in `4/4` until you say
otherwise:

```kf+
#E
```

Anything you leave out falls back to a default — `4/4`, no fixed tempo, and the
key of C. That is why `#E` above already has a time signature: the defaults were
always there, and the chart draws what it knows so far.

### Reading the key

The key token opens with a `#` (or `b`) **marker**. Its only job is to say "this
token is the key," so it is not mistaken for a chord. The marker does *not* mean
the key is sharp or flat — the accidental and the quality are written into the
name itself.

| Written | Key      |
| ------- | -------- |
| `#C`    | C major  |
| `#Gm`   | G minor  |
| `#Eb`   | E♭ major |
| `#F#`   | F♯ major |
| `#Am`   | A minor  |

A trailing `m` makes it minor; no `m` means major. `#Eb` and `bEb` mean the same
thing — use whichever reads better.

The key earns its place beyond the signature: it is what lets you write chords
as Nashville numbers or Roman numerals, which are relative to it. See
[[chords|Chords]].

---

Previous: [[lifecycle|The Life of a Chart]] · Next: [[structure|Structure]] · Up: [[lifecycle|The Life of a Chart]]
