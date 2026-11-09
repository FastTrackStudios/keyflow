---
title: Cheatsheet
kind: reference
type: reference
order: 98
stage: Reference
summary: Every symbol Keyflow reads, on one page.
---

# Cheatsheet

Every symbol the language reads, grouped by where you meet it. Each group ends with a chart that uses the symbols above it — those are live, so you can open one in the editor and take it apart.

The chapters have the reasoning; this page is for when you know what you want and need the spelling.

## The header

Two lines, both optional.

| Symbol | Means | Example |
| --- | --- | --- |
| `-` | separates title from artist | `Rosalita - Bruce Springsteen` |
| `( )` | subtitle | `Rosalita (Come Out Tonight)` |
| `#` | a sharp key | `#G` |
| `b` | a flat key | `bEb` |
| `m` | minor | `#F#m` |
| `bpm` | tempo | `120bpm` |
| `/` | time signature | `4/4` |

```kf+
Rosalita (Come Out Tonight) - Bruce Springsteen
#G 120bpm 4/4
```

A line `Transcribed By Cody Wright` under the title (any case, a colon after *By* is fine) credits the transcriber: it prints as a small line above the page footer, on every page, and leaves the subtitle alone. To keep parentheses in the title itself, give it with `\title "Joy To The World (Unspeakable Joy)"` (and `\subtitle "…"` for a subtitle of its own).

[[header|Header]] has the detail.

## Sections

| Symbol | Means | Example |
| --- | --- | --- |
| *name* + count | a section and how many bars | `VS 4` |
| `" "` | a variant note, which rides along with the numbering | `CH 4 "Down"` |
| `[ ]` | a name that isn't one of the built-ins | `[Guitar Solo] 8` |
| `#` | a key change from here | `BR 8 #A` |
| *nothing under it* | the last section of this kind, again | `CH 4` |
| `= OTHER` | the chords from another section | `IN 4 = VS` |

```kf+
VS 4
G C Em D

CH 4 "Down"
C G D D

[Guitar Solo] 4
G C Em D
```

The built-in names are in [[structure|Structure]].

## Chords

| Symbol | Means | Example |
| --- | --- | --- |
| `#` `b` | accidental on the root | `F#` · `Bb` |
| `m` | minor | `Em` |
| `maj` `dim` `aug` `sus` | quality | `Cmaj7` · `Bdim` · `Dsus4` |
| `7` `9` `11` `13` | extension | `G13` |
| `( )` | alteration | `C7(b9)` |
| `/` | a bass note under the chord | `C/E` |
| `!` | major, said out loud | `!C` |

```kf+
VS 4
Cmaj7 F#m7(b5) Bb13 C/E
```

Four chapters cover these: [[chord-root|Root]], [[chord-quality|Quality]], [[chord-extension|Extension]], [[chord-alteration|Alteration]].

## Rhythm

| Symbol | Means | Example |
| --- | --- | --- |
| `/` | one beat | `G // C //` |
| `.` after slashes | dotted | `G //. C /` |
| `_2` `_4` `_8` | note value | `G_2 C_2` |
| `.` after a value | dotted | `G_4.` |
| `t` after a value | triplet | `G_8t` |
| `~` | tie into the next chord | `G // ~` |
| `r1` `r2` `r4` | a rest | `r1` |
| `s1` `s2` `s4` | a space — silent, and not drawn | `s1` |

```kf+
VS 2
G //. C / D_8t Em_8t F_8t A_2
```

[[rhythm|Rhythm]], then [[slash-notation|Rhythmic Slash Notation]] and [[lilypond-rhythm|LilyPond Rhythm Notation]].

## Repeats

| Symbol | Means | Example |
| --- | --- | --- |
| `%` | simile — one more measure of the last one | `G % % %` |
| `%3` | three simile marks | `G %3` |
| `.` | direct repeat — as long as the last chord lasted | `G . . .` |
| `.3` | three direct repeats | `G .3` |

```kf+
VS 6
G %3 C // D // .2
```

`%` counts bars; `.` counts whatever you last wrote, so `G // . . .` is two bars and `G . . .` is four. [[repeats|Repeats]] has the rest, including why `.3` is off in a scale-degree chart.

Bar lines are drawn for you, but you can place them yourself, and a pair of repeat signs plays a run twice:

```kf+
VS 2
|: G | C :|
```

## Marks on a chord

| Symbol | Means | Example |
| --- | --- | --- |
| `'` | push by an eighth | `'C` |
| `''` | push by a sixteenth | `''C` |
| `>` | accent | `>C` |
| `.` before a chord | staccato | `.C` |
| `!STOP` | a full stop, before or after | `G !STOP` |
| `\marcato` after a chord | marcato (^) | `C \marcato` |
| `\fermata` after a chord | a hold, drawn over the symbol | `A_2. \fermata` |

A bar written as one value that fills it (`A_1`, `A_2.` in 3/4) is a hold and draws as a held slash with its stem and dot. `G_4 ~` ties the chord over the barline: the tie is drawn and the next bar's `G` is not named again.

```kf+
VS 4
G >C .Em 'D
```

## Words on the page

| Symbol | Means | Example |
| --- | --- | --- |
| `" "` | text under the staff | `"quiet" G` |
| `^" "` | text above it | `^"Build" G` |
| `" "` on a chord | text beside that chord | `G"hit"` |
| `@` | a cue for one instrument group, on its own line; `@all` is everyone | `@keys "pad"` |
| `< >` | an intensity marking, on its own line | `<Build>` |

Words go under the staff unless you ask for above: plain `" "` text and every `@` cue (in its group's colour) sit below the bar, and only `^" "` puts text over it.

```kf+
VS 4
^"Build" G C
@keys "pad"
Em D"hit"
```

## Lyrics

A `[lyrics]` line after a section's chords is its lyric lane. Stamp the words with where they are sung and they are drawn under the slashes, each word under its bar and beat:

| Symbol | Means | Example |
| --- | --- | --- |
| `@` + bar | bar of the section, from 1 | `@2 world` |
| `@` bar `:` beat | a beat in that bar; `.5` is the "and" | `@1:3.5 to` |
| `-` | one word, sung over two places | `@3 re- @3:3 ceive` |
| `_` | the word before is held here | `@1 night @2 _` |

A word without a stamp follows the one before it, and a word whose beat is already taken runs on after its neighbour, so a busy bar still reads. The syllables of a word get a hyphen between them, and a held word a line out to the next one. A section whose only line is its lane keeps the chords it would have recalled. Bars are counted from the section's first bar; a `Count` in front of the chart is not one of them.

```kf+
VS 4
G C D G
[lyrics]
@1 Sing @1:3 a @2 new @2:3 song
@3 ev- @3:3 ery @4 day
```

Two commands write those lanes for you, as a `[lyrics lrc]` lane in each section that running them again replaces:

- `kf lyrics-lrc song.kf song.lrc --downbeat 12.4 -o out.kf` reads an `.lrc`. Bar 1 starts 12.4 seconds into the recording and the chart's tempo and meters place the rest, or `--sync sync.json` takes each bar's start from a Songsterr sync (skipping its lead-in bar on its own). LRC lines are stamped a touch early, so they are moved a quarter second late; `--offset` changes that. Lines that start before bar 1 or after the last bar are reported, not dropped.
- `kf lyrics-sheet song.kf sheet.txt -o out.kf` reads a chord-over-lyrics sheet, each chord one bar of the chart in order, and puts every word in the bar of the chord at or before it; `[Chorus]` moves on to the chart's next chorus. With `--map` it reads one `bar beat word` per line instead.

Words go on the nearest eighth; `--grid 4` puts them on beats.

## Directives

A line starting with `\` sets something for the chart. Backslash, not slash: a line starting with `/` is a bar of rhythm slashes, and the two would be impossible to tell apart. The editor's command menu offers these, and the parser rejects one it doesn't know — so the list here is the list.

| Directive | Means |
| --- | --- |
| `\duration 4` | the length every chord takes unless it says otherwise |
| `\push standard` | how a pushed chord divides the beat |
| `\swing straight` | straight, triplet, or a ratio |
| `\smart_repeats = true` | group repeated phrases under repeat signs |
| `\auto_rhythm_slashes = true` | fill long chords with quarter-note slashes |
| `\push_alters_rhythm = true` | a push changes the notation, not just the symbol |
| `\chord_memory = true` | a bare root recalls the quality its degree last had |
| `\repeat_chord_symbols = clusters` | `changes` (default), `clusters` or `always`: when a chord still sounding is named again |
| `\progression G B C Cm` | the chords a section falls back to when it names none |
| `\title "Joy To The World (Unspeakable Joy)"` | the title exactly as written, parentheses and all |
| `\subtitle "Live at the Ryman"` | the line under the title |
| `\alias name value` | give a name to a run of chart text |

```kf+
\duration 2

VS 2
G C D Em
```

A bar's first chord is named when the harmony changes there and at the start of every line or section. A chord still sounding from the bar before is not named again — not after `D /// G /` then `G`, and not in a bar of changes that starts on it (`Bmaj7` then `Bmaj7 / G#m7 / C#m9 / F#7 /` names only the G#m7, C#m9 and F#7). That is `\repeat_chord_symbols = changes`, the default. `clusters` also names a bar of changes' first chord when the bar before held that one chord (the second `Bmaj7` above prints); `always` prints every symbol.

A bar written with note values (`_2`, `_8`, `r4`) is drawn as rhythmic notation — a half is a stemmed half-note slash, an eighth a flagged one — and every chord symbol sits over the slash it is played on. A bar of plain chords (`G // C //`, or `G` for the whole bar) is drawn as beat slashes. Bars widen for their chord symbols rather than letting one drift off its beat; `kf pdf --chord-gap <pt>` (and `kf png`) sets the clear space kept between two symbols (default 3pt), so a smaller gap packs chord-dense bars onto fewer lines.

## Melody

| Symbol | Means | Example |
| --- | --- | --- |
| `m{ }` | a melody line | `m{ c4 d4 e4 f4 }` |
| `<< ; >>` | a melody alongside the chords | `<< G ; m{ c4 d4 e4 f4 } >>` |
| `$` | recall a named melody | `$mainRiff` |

```kf+
VS 1
<< G ; m{ c4 d4 e4 f4 } >>
```

---

Previous: [[alternatives|Alternatives]] · Up: [[introduction|An Introduction]]
