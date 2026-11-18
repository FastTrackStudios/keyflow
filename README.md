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
