---
title: Header
kind: concept
type: concept
order: 1
stage: Start here
---

# Header

The first two lines say what the song is. Everything on them is optional.

```kf+
Song Title (Subtitle) - Artist Name, Second Artist Name
#C 120bpm 4/4
```

That is the whole header: a title line and a defaults line. Every chart in
the rest of this guide leaves them out, because once you have seen them
once they are noise.

## The title line

Everything before the dash is the title, everything after it is the
artist. Parentheses make a subtitle, and a comma separates artists.

| Written | Means |
|---|---|
| `Song Title` | a title, no artist |
| `- Artist Name` | an artist, no title |
| `Song Title (Subtitle)` | a title with a subtitle |
| `Song Title - A, B` | two artists |

Either half can stand alone. A line that opens with a dash is an artist,
which is how you write one without inventing a title for it yet.

## The defaults line

`#C` is the key, `120bpm` the tempo, `4/4` the time signature. Order does
not matter — `4/4 #C 120bpm` is the same line, so write them in whatever
order you think of them.

Each is independent of the others. A line carrying only a tempo is a
valid header, and so is a line carrying only a key.

> [!info] Why a bare key engraves a bar
> A key signature has to sit on a staff, so declaring one implies an
> opening measure in 4/4. The alternative — refusing to draw anything
> until you have typed enough — makes the format feel like it is waiting
> for you.

## The `#` is not a sharp

`#C` is *the key of C*, not C-sharp. The `#` marks the token as a key
signature. A sharp key is written the way you would say it — `#F#` is the
key of F-sharp.

---

Previous: [[introduction|An Introduction]] · Next: [[structure|Structure]] · Up: [[introduction|An Introduction]]
