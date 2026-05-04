# Keyflow reference datasets

Test fixtures for engraving, MIDI ingest, and rhythm-chart syntax development.

`./data/` is gitignored. Run `./fetch.sh` (no args = default set) or pass dataset names.

## Default set (~885 MB total)

| Dataset | Size | Format | License | Use |
|---|---|---|---|---|
| `groove` | 9 MB | MIDI (drums, grid + swing metadata) | Apache-2.0 | Rhythm-chart syntax: tuplets, swing, slash notation, drum-line engraving |
| `openscore-lieder` | 835 MB | MusicXML / MSCZ | CC0 | Engraving parity vs. MuseScore; vocal + piano staves, lyrics |
| `bach-fixtures` | 28 KB | MXL (compressed MusicXML) | BSD (music21 corpus) | Tiny deterministic CI golden set; 4-part chorales |
| `wjd` | 41 MB | SQLite (`wjazzd.db`) | Free for research | Jazz lead-sheet ingest: chord symbols, beat grid, solo transcriptions |

## Optional (pass name to `fetch.sh`)

- `pop909` — 909 pop songs, melody/lead/piano MIDI (research license)
- `bach` — pointer only; install via `pip install music21`

## Suggested integration order

1. **`bach-fixtures/`** → first integration test in `keyflow-text` / `engraver` (small, fast, deterministic)
2. **`groove/`** → drives `keyflow-midi` rhythm-chart ingest (real swing vs. quantized grid)
3. **`wjd`** → exercises chord-symbol-above-staff and slash-notation features
4. **`openscore-lieder/`** → render-parity regression suite vs. MuseScore output

## Notes

- Datasets w/ git history (lieder) cloned `--depth 1`; still ~835 MB. Use `--filter=blob:none` w/ sparse-checkout if size becomes a problem.
- WJD URL occasionally rotates — check https://jazzomat.hfm-weimar.de/dbformat/dbcontent.html if 404.
- All datasets here are research/PD/permissive licensed. Do **not** add commercial-restricted MIDI dumps (e.g., scraped MIDI sites) without license review.
