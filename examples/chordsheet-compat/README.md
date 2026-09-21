# chordsheet-compat

A chordsheet.com account backup and what `keyflow-chordsheet` makes of it.

- `chordsheet/` — the source text, one `.txt` per chart, as the site exports it.
- `manifest.json` — the backup's index: title, artist and song id per chart.
  `kf chordsheet` reads it from beside `chordsheet/` for per-chart options.
- `kf/` — every chart converted to keyflow.
- `png/` — two of them engraved in folded mode.

Regenerate everything after a change to the converter or the engraver:

```bash
kf chordsheet examples/chordsheet-compat/chordsheet -o examples/chordsheet-compat/kf
kf png "examples/chordsheet-compat/kf/THE KILLERS - MR.BRIGHTSIDE.kf" \
  --chart-mode folded --scale 1.5 -o examples/chordsheet-compat/png/mr-brightside-folded.png
```

The `#[ignore]`d corpus tests in `features/chordsheet/tests/corpus.rs` still
read their own copy at `features/examples/chordsheet/`, which stays out of git:
a test fixture lives inside the crate that reads it, and this directory is
outside every crate.
