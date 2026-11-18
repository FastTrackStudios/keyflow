# Captures for Keyflow's chart input (selection, hover, MuseScore-style note input)

Taken on Keyflow `05dc3d5` running as a process extension in a stock Architect (`runtime-compositor` `51e50333`), in the focused dev window (`--only keyflow --hide all`), in an isolated headless Loft session. The chart is *COLDPLAY – YELLOW* from the dev vault, opened fresh each time.

## Recording

`recording/sketch-bar12.mp4` (30 s, 1280 px):
1. Click bar 12 and press K (input via syntax).
2. Type four bars of chords, pressing Enter after each: `Am7 / D7 /`, `Gmaj7`, `Em7 / A7 /`, `Dmaj7`. Each bar shows on the chart as a preview before Enter writes it, and the field clears and moves on to the next bar.
3. Press Esc, click bar 12 and press N (input by note name), then Y to enter notes rather than chord symbols.
4. With 5 (a quarter), type C then E, and raise the E a semitone with ↑.

## Captures

Each was taken at 390 and 1280 px, light (`light`) and dark (`dark`):

| file | what it shows |
|---|---|
| `*-select-hover.png` | B selected, drawn in the selection colour; Bsus4 under the pointer, in the hover colour; each redrawn as itself, not boxed |
| `*-note-name-chords.png` | Input by note name, entering chord symbols: a C chord typed at a half note, the cursor on the next beat, and the status line |
| `*-note-name-notes-pointer.png` | Input by note name, entering notes: the note just entered highlighted as the selection, the cursor, and the shadow note under the pointer |
| `*-duration.png` | Input by duration: the pitch cursor (G) shown as a shadow note at the cursor |
| `*-syntax-preview.png` | Input via syntax: `Dm7 / G7 / \| Cmaj7` previewed over bars 12–13, before it is written |
| `kf1280-light-chrome-note-name.png` | Input by note name with the app's chrome shown |

## Round 3

Taken on Keyflow `feat/chart-selection-and-input-modes` after keyflow#92, in the same setup, with Architect at `runtime-compositor-keyflow-input` `d20f1fa0` (the focus fix).

### Recording

`recording/sketch-16-bars.mp4` (40 s, 1280 px), a new blank chart (`Sketch\n4/4\n`):
1. One click on the empty paper to give the chart the keyboard (a newly opened pane does not take it; see the report). Every key after that is the keyboard alone.
2. K, then 16 bars of chords typed on the chart, previewed as they are typed. Enter writes them; the chart had no bars, so they are added as a section of their own.
3. Esc, Ctrl+Home, Ctrl+→ ×3 to bar 4, K, `G7 / / /`, Enter. The chart follows the cursor.
4. The same at bar 8 (`Gsus4 / G7 /`) and at bar 11 (`E7 / / /`).
5. N, Ctrl+Home, Ctrl+Alt+1 (voice 1, which also means notes), 5 (a quarter), E, G: two melody notes in bar 1.

### Captures

| file | what it shows |
|---|---|
| `captures/round3/kf1280-light-voices-triplet.png` | bar 6 of YELLOW with two voices: voice 1 stems up, voice 2 stems down with a triplet under a `3`. The cursor is in voice 2's colour (green), restored after a reload |
| `captures/round3/kf390-light-voices-triplet.png` | the same bar at 390 px |

## Round 4

| file | what it shows |
|---|---|
| `captures/round4/kf1280-blank-chart.png` | a blank chart (`Sketch\n4/4\n`) opened: its title and an empty first system, framed as a page |
| `captures/round4/kf1280-retype-chord-and-replay.png` | *Held* (`E /// G#m / G#m B /// C#sus2 / C#sus2`, then CH, then a bare `VS`): bar 2 of the held-chord line edited to D (the line written out as `E /// G#m \| D_1 \| B /// C#sus2 / C#sus2`), the D retyped as `Dsus4` in the bar under the chart, and VS 2 playing VS 1's bars, D included |

## Round 5

Taken on Keyflow `main` `bc59f0b` (after #113–#116) as a process extension in Architect `main` `48d3894e`, with the app's chrome shown, in an isolated headless Loft session (2560 × 1640, dark theme). The chart is *JT Recital / 02 Drive To LA*, at 125%. Architect after `4d537fb8` (architect#874) is needed for a process view's images to draw; at Keyflow's pin (`9d1f2f48`) the palette glyphs are blank.

| file | what it shows |
|---|---|
| `round5/chart-pane-palette-applied-dark.png` | The whole chart pane, from the app's own render (`APP_SHOT`): the toolbar, the palettes, and bar 22's Bdim selected with two palette cells applied, `m7♭5` then the accent, so it reads `>Bm7b5`. The file's line went from `C // Bdim //` to `C // >Bm7b5 //`, keeping its beat slashes (#116). The status line names the selected bar; the palettes' foot names the last cell chosen. Bar 19's G△7 is an earlier `maj7` from the same palette. |
| `round5/palettes-tempo-glyphs-dark-2x.png` | The palettes at 2×: the Tempo cells' quarter notes, the articulations, segno and coda, inked in the theme's text colour (#114), and the accent cell just used. |

## Round 6: the three input modes

Recorded on Keyflow `feat/handbook-differences` `b275efa` (Keyflow `main` `3164597` plus J and the status line's description) as a process extension in Architect `main` `48d3894e`, the app's chrome shown, in an isolated headless Loft session (2536 × 1510 window, encoded at 1280 px). Each starts where the one before left the chart, a blank chart (`Sketch`, `4/4`).

| file | length | what it shows |
|---|---:|---|
| `recording/round6/input-via-syntax.mp4` | 21 s | A click on the blank chart's first bar, then K. `Am7 / D7 / \| Gmaj7` typed: each keystroke previewed on the chart before Enter writes it. Enter; then `Em7 / A7 / \| Dmaj7` and Enter; Esc. The chart has four bars. |
| `recording/round6/input-by-note-name.mp4` | 21 s | A click on bar 1, then N, Y (notes), 5 (a quarter), C, E, G: each entered at the cursor, which moves on. Alt+3 adds B, a third above the G. `+` arms a sharp, F enters F♯, and J respells it G♭. Esc: the status line describes the selected note ("Normal mode · Note G♭3 · quarter · voice 1 · bar 1, beat 4"). |
| `recording/round6/input-by-duration.mp4` | 20 s | A click on bar 2, then M and Y (notes). 5 enters a quarter at the pitch cursor (shown as a shadow note in the voice's colour); ↑ moves the pitch, 5 enters again. ← goes back to the last note, D picks the pitch, and 5 at that note's length adds D to its chord rather than writing over it. Esc: "Normal mode · Chord C4 D4 · quarter · voice 1 · bar 2, beat 3". |

## Round 7: the input modes with MuseScore's remaining behaviours

Recorded on Keyflow `feat/voice-colour-selection` `b82c34f` (Keyflow `main` `b0b4044` plus voice colours, the above-staff ruler, re-pitch and intervals below) as a process extension in Architect `main` `48d3894e`, the app's chrome shown, in an isolated headless Loft session (2536 × 1510 window, encoded at 1280 px). The chart is a two-voice demo: bar 1 is `<< C_1 ; m{ E:44 G:44 C:52 } ; m{ C:42 r2 } >>` (voice 1's stems up, voice 2's down), then three bars of slashes. Each recording starts where the one before left the chart.

| file | length | what it shows |
|---|---:|---|
| `recording/round7/input-via-syntax.mp4` | 15 s | A click on bar 3, then K. `Am7 / D7 / \| Gmaj7` typed: previewed on the chart as it is typed, Enter writes bars 3 and 4, Esc. |
| `recording/round7/input-by-note-name.mp4` | 23 s | A click on voice 2's half rest: it is selected in voice 2's colour (green), and the status line says "Rest · half · voice 2". N: note input enters voice 2 there, C then E, in green; Alt+3 adds a third above the E. Esc. A click on voice 1's quarter note (beat 2), N, then ⇧⌘I (re-pitch): A and G re-pitch voice 1's quarter and half, their lengths kept, and F finds no note left in the bar and says so. ⇧⌘I off, Esc. |
| `recording/round7/input-by-duration.mp4` | 16 s | A click on voice 1's note at beat 2, then M: input by duration's cursor is the above-staff ruler (a pin in the voice's colour where the next note goes, a tick a duration on to the barline), with the pitch cursor as a shadow note. ↑ ↑ move the pitch; 5 (a quarter, the note there's length) adds it to that note's chord rather than writing over it. ↑ and 6 (a half) do the same to the half note. Esc. |

The chart's bar 1 after all three: `<< C_1 ; m{ E:44 <G:4 A:4>4 <G:4 A:4>2 } ; m{ C:42 C:44 <E:4 G:4>4 } >>`.
