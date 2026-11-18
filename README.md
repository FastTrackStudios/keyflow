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
