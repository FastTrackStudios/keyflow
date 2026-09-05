---
title: Header
kind: concept
type: concept
order: 1
stage: Start here
---

# Header

The first lines say what the song is. Everything on them is optional.

```kf+
Sunday Morning - The Wandering
4/4 #G 120bpm

VS 4
G C Em D
```

Two lines: the title and artist, then the musical defaults.

## Title and artist

The first line is the title. A dash separates the artist:

```kf+
Sunday Morning - The Wandering
```

That is a whole file, and it engraves — a title page with nothing under it
yet, which is what you have when you have only decided what you are writing.

Either half can stand alone. A leading dash means the line is an artist:

```kf+
- The Wandering
```

## Meter, key and tempo

The second line carries the defaults. Order does not matter:

```kf+
Sunday Morning
4/4 #G 120bpm
```

`4/4` is the time signature, `#G` the key, `120bpm` the tempo. Write them in
whatever order you think of them — `#G 4/4 120bpm` is the same line.

Each is independent, and each engraves on its own:

```kf+
120bpm
```

```kf+
#E
```

> [!info] Why a bare key engraves a bar
> A key signature has to sit on a staff, so declaring one implies an opening
> measure in 4/4. The alternative — refusing to draw anything until you have
> typed enough — makes the format feel like it is waiting for you.

## The `#` is not a sharp

`#G` is *the key of G*, not G-sharp. The `#` marks the token as a key
signature. A sharp key is written the way you would say it:

```kf+
Sharp Key - Demo
4/4 #F# 120bpm

VS 4
F# B C#m F#
```

---

Previous: [[introduction|An Introduction]] · Next: [[structure|Structure]] · Up: [[introduction|An Introduction]]
