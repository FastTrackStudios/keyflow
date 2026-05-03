# Chord-Syllable Integration: Complete Implementation Guide

## Overview

This document describes the complete implementation for assigning chords to specific syllables in Keyflow lyrics, enabling synced playback, MIDI generation, interactive charts, and more.

## Architecture Components

### 1. **Multi-Block Documents** (`document/mod.rs`)
- Parse `.kf` files with `--- blockname ---` delimiters
- Support: `keyflow`, `chordpro`, `voicings`, and custom blocks
- Fully backward-compatible with plain `.kf` files
- `keyflow` remains the master rhythm chart; `chordpro` blocks can be
  layered on top for lyric/chord placement.

### 2. **Syllable-Aware Lyrics** (`chart/lyrics.rs`)
- `LyricLine`: Container for syllables
- `LyricSyllable`: Individual syllable with:
  - Text content
  - Chord attachment (optional)
  - Attachment type (before word, on syllable, between, etc.)
  - Word boundary markers
  - Timing information (measure + beat)
- `LyricLine` metadata now records source format, sync level, source label,
  singer, and part so the same section can carry multiple vocal/lyric layers.

### 3. **Syllable Parsing** (`chart/syllable_parser.rs`)
- **Knuth-Liang hyphenation** (when feature enabled)
- **Explicit syllable marks**: `A|ma|zing`
- **Graceful fallback**: Works without hyphenation (treats words as single syllables)
- Optional dependency: `hyphenation` crate

### 4. **Manual Chord Assignment** (`chart/lyric_chord_parser.rs`)
- **Format**: `{ChordSymbol}syllable {NextChord}next-syllable`
- **Examples**:
  - `{Gm}Slow {A#}down you {F}cra-zy {Gm}child`
  - `{Cmaj7}A-{Dm7}ma-{G}zing {Cmaj7}grace`
- Integrated into `.kf` lyric track parser
- Fallback: If parsing fails, uses simple syllable splitting

### 5. **Chord-Syllable Alignment** (`chart/chord_syllable_alignment.rs`)
- `ChordSyllableMapping`: One chord → one syllable relationship
- `SectionAlignment`: Container for all mappings in a section
- Bidirectional queries:
  - `chords_for_syllable(idx)` → all chords on this syllable
  - `syllables_for_chord(idx)` → all syllables with this chord
- `ChordSyllableAligner`: Algorithm to compute alignments from chord instances + syllables

## Usage Example

### 1. **Input: `.kf` File**
```
vs verse 1
Gm //// A#//// F //// Gm////
[lyrics] {Gm}Slow down you {A#}crazy child, {F}you're so {Gm}am-bi-tious
```

### 2. **Parsing Pipeline**
```rust
// 1. Parse multi-block document
let doc = parse_kf_document(content)?;

// 2. Extract lyrics block and parse with chord assignments
let chord_parser = LyricChordParser::new();
let lyric_line = chord_parser.parse("{Gm}Slow {A#}down")?;
// Result: syllables with chords attached

// 3. Create chord instances from chart
let chords = vec![
    ChordInstance { full_symbol: "Gm", position: 0:0, ... },
    ChordInstance { full_symbol: "A#", position: 0:2, ... },
];

// 4. Align chords to syllables
let alignment = ChordSyllableAligner::align(&chords, &lyric_line.syllables)?;

// 5. Use alignment for various features
for mapping in &alignment.mappings {
    println!("Syllable '{}' → Chord '{}' for {} beats",
        syllables[mapping.syllable_index].text,
        chords[mapping.chord_index].full_symbol,
        mapping.duration_until_next_chord.beat);
}
```

## Enabled Features

### 🎵 **Synced Lyrics / Karaoke**
Each syllable knows:
- Which chord is active
- When the chord starts (timing)
- How long until the next chord change

```
Syllable "Slow" @ t=0.0s
  ├─ Chord: Gm
  ├─ Duration: 2 beats
  └─ Next chord: A# @ t=1.0s
```

### 📟 **Interactive Lead Sheets**
- Click syllable → highlight its chord
- Hover chord → highlight all syllables under it
- Bidirectional lookups enable both directions

### 🎵 **MIDI Generation**
- Syllable boundaries = note event timing
- Exact pitch per syllable (with voicing notation in Phase 3)
- Tempo-aware timing

### 📱 **Visual Lyric Slides**
```
"Slow ━━━━━━━ down ━━━━━"
    Gm ─────  A# ──
```
Visual representation of syllable duration and chord changes

### 🎹 **Synced Playback**
```
t=0.0s: Display "Slow", play Gm notes
t=1.0s: Display "down", play A# notes
```

## Format Specification

### Multi-Block `.kf` With ChordPro Lyrics

The `.kf` file can hold a keyflow rhythm block plus one or more ChordPro lyric
blocks:

```text
--- keyflow ---
120bpm 4/4 #C
VS 1: | 1 4 5 1 |
CH 1: | 4 5 1 1 |

--- chordpro ---
{sov: singer=lead part=Lead sync=words}
[C]Twinkle, [F]little [C]star
{eov}
{soc: singer=lead part=Lead sync=slides}
[F]How I [C]wonder
{eoc}

--- chordpro ---
{sov: singer=harmony part=Harmony sync=syllables}
[C]Twin-kle, [F]lit-tle [C]star
{eov}
```

Rules:
- The `keyflow` block is the timing source of truth for section structure and
  chord rhythms.
- Each `chordpro` block is a lyric layer over that rhythm chart. A second
  `chordpro` block can attach another singer, part, translation, or sync
  granularity to the same sections.
- ChordPro environments (`{sov}`, `{soc}`, `{sob}`) attach to matching keyflow
  Verse/Chorus/Bridge sections in source order.
- Environment labels can carry lightweight metadata:
  `singer=<id>`, `part=<name>`, and `sync=section|slides|words|syllables`.

### Chord Assignment Syntax in `.kf` Files

```
[lyrics] {Chord1}syllable {Chord2}next-syllable {Chord3}more
```

**Rules:**
- Chord symbols in braces: `{Cmaj7}`, `{Bb7#9}`, `{Am}`
- Applied to immediately following syllable
- Multiple chords possible per line
- Works with hyphenated syllables: `{Dm7}a-{G}ma-{C}zing`
- Syllables without explicit chords are still tracked

**Examples:**
```
// Simple
[lyrics] {C}Say it {G}isn't {D}so

// With hyphenation (multi-note syllables)
[lyrics] {Cmaj7}A-{Dm7}ma-{G}zing {Cmaj7}grace

// Complex
[lyrics] {C}You've got {Dm}your passion, {G}you've got {C}your pride
```

## Data Structure Overview

```
ChordSyllableMapping
├─ chord_index: usize          (which chord)
├─ syllable_index: usize       (which syllable)
├─ chord_position: AbsolutePosition
├─ duration_until_next_chord: MusicalDuration
└─ attachment: ChordAttachmentType

SectionAlignment
├─ mappings: Vec<ChordSyllableMapping>
├─ chord_count: usize
└─ syllable_count: usize
    └─ Query: chords_for_syllable() / syllables_for_chord()

LyricSyllable
├─ text: String
├─ chord: Option<String>       (e.g., "Gm", "A#", "Cmaj7")
├─ chord_attachment: Option<ChordAttachment>
├─ hyphen_after: bool          (melisma indicator)
├─ measure_index: usize
├─ beat: f32
└─ word_initial: bool
```

## Integration Points

### In `.kf` Parser (`keyflow-text/src/chart/parser/sections.rs`)

When parsing a lyrics track:
```rust
TrackType::Lyrics => {
    let combined = track_lines.join(" ");

    // Try chord assignment parsing first
    let lyric_line = match LyricChordParser::new().parse(&combined) {
        Ok(line) => line,
        Err(_) => LyricLine::parse_simple(&combined) // fallback
    };

    let track = Track::lyrics(lyric_line);
    tracks.push(track);
}
```

## Backward Compatibility

✅ **Fully backward compatible**
- Plain `.kf` files work unchanged
- Lyrics without `{Chord}` syntax parsed as before
- Existing ChordPro/voicing blocks ignored if not used
- Graceful fallback when chord parsing fails

## Examples

See `/keyflow/examples/` for complete working examples:
- `vienna_manual_alignment.rs` - Chord assignment syntax demo
- `vienna_full_example.rs` - Complete pipeline walkthrough
- `vienna.kf` - Real Vienna song with all blocks

## Next Steps

### Phase 3: Voicing Notation
- Syntax: `<c e g> <d f# a>` for exact pitches
- LilyPond-style octave marking
- Maps to MIDI pitches

### Phase 4: MIDI Export
- Generate .mid files from voicings
- Use syllable-chord alignment for timing
- Tempo-aware MIDI formatting

### Integration
- UI for interactive chord-syllable assignment
- Visual representation of alignments
- Synced playback engine
- Export to Karaoke formats
