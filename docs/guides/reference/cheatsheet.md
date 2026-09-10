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
| `@` | a cue for one instrument group, on its own line | `@keys "pad"` |
| `< >` | an intensity marking, on its own line | `<Build>` |

```kf+
VS 4
^"Build" G C
@keys "pad"
Em D"hit"
```

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
| `\progression G B C Cm` | the chords a section falls back to when it names none |
| `\alias name value` | give a name to a run of chart text |

```kf+
\duration 2

VS 2
G C D Em
```

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
