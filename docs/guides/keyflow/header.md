---
title: Header
kind: concept
type: concept
order: 2
stage: Chart
---

# Header

Two lines. Everything on them is optional.

```kf+
Song Title (Subtitle) - Artist Name, Second Artist Name
#C 120bpm 4/4
```

Every chart after this one leaves them out.

## The title line

Before the dash is the title, after it the artist. Parentheses make a subtitle; a comma separates artists.

| Written | Means |
|---|---|
| `Song Title` | title, no artist |
| `- Artist Name` | artist, no title |
| `Song Title (Subtitle)` | title with subtitle |
| `Song Title - A, B` | two artists |

## The defaults line

`#C` key, `120bpm` tempo, `4/4` time signature. Order does not matter. Each is independent — a line with only a tempo is a valid header.

> [!info]- Why a bare key engraves a bar
> A key signature needs a staff, so declaring one implies an opening 4/4 measure. Refusing to draw until you have typed enough makes the format feel like it is waiting for you.

## The `#` is not a sharp

`#C` is *the key of C*, not C-sharp. A sharp key is written as you would say it: `#F#`.

See also: [[chords|Chords]]

---

Previous: [[alternatives|Alternatives]] · Next: [[structure|Structure]] · Up: [[introduction|An Introduction]]
