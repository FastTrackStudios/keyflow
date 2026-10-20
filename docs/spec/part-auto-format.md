# Part Auto-Format — line, page and page-turn breaking for orchestral parts

Status: draft (2026-10-19). Owner: engraver domain. Design only — nothing
here is implemented yet.

Goal: a **real auto-formatter** for extracted parts. Given one part of a
score (and, for books, the songs around it), choose system breaks, page
breaks and song start pages so the part is as readable as possible and
**every page turn falls where the player can turn** — during a rest long
enough to turn, or not at all. Target: the Columbus 2026 books (38 chairs ×
~16 songs) with zero turns inside played music wherever the music allows
one, and a report saying exactly where it does not.

## Goals and non-goals

Goals:

- **Readable systems.** Even spacing at a comfortable density, consistent
  bars per system, breaks at musical boundaries (rehearsal marks, double
  bars, before multirests), no lone or orphaned bars, never below minimum
  measure widths.
- **Playable turns.** A turn only where the trailing + leading silence
  around the page break covers the turn time at the song's tempo. With a
  bound book, breaks from a left page to a right page are free (both are
  visible); only right→left breaks are turns.
- **Book-aware.** A 2-page song starts on a left page so it needs no turn;
  blank pages are added when they buy a turn-free song and the extra paper
  is worth it.
- **Measurable.** Every decision is visible in a metrics report
  (`kf parts --turns`) and comparable across the whole library, the way
  `kf parts --check` measures collisions today.
- **Opt-in, reversible.** The arranger's stored Finale layout stays
  available as `--layout finale` and remains the default until the auto
  formatter beats it on the metrics.

Non-goals (for this spec):

- Horizontal spacing inside a system. The spring model
  (`measure_layout::distribute_measure_widths_spring`, chart
  `width_dist`) is unchanged; we only choose which measures share a line.
- Full scores / multi-staff systems. Parts only (grand-staff parts — harp,
  keyboard, choir pairs — are single systems of two staves and are in).
- Rewriting notation: no cue insertion, no re-barring, no transposition.
  Splitting a multirest at a turn is the one structural change allowed.
- Repeat / D.S. / coda flow. The notation model has no repeats or jumps
  yet; see Risks.
- Interactive editing (MuseScore-style system/page locks). Overrides come
  from a sidecar file, not a UI.

## What we have today

| Piece | Where | Role |
| --- | --- | --- |
| Stored Finale layout | `finale.rs` → `ExplicitLayout` (`systems`, `multirests`, `measure_widths`) | The arranger's breaks, page per system, staff top, spatium |
| Prepared items | `layout.rs` `prepare_measures` → `Vec<Prepared>` (`Notated` / `MultiRest(MultiRestBlock)`) | The unit of line breaking: a bar or a multirest run |
| Measure metrics | `layout_part` `MeasureMetrics { min_width, weight, source_width }` | Compact-build minimum width + spring weight per item |
| System plan | `layout_part` `match explicit_systems { Some(e) …, None => … }` | Finale breaks, or greedy min-width packing, `MAX_MEASURES_PER_SYSTEM = 6` |
| Stale check | same, `demand <= 1.15 * width` | Drops Finale breaks when a stored system cannot hold its bars |
| Vertical placement | `stack_systems` | Skyline min distance (`needed`), Finale tops kept, compact+share slack, re-flow on overflow or a <50 % non-final page (keeping Finale line breaks) |
| Collision metric | `inspect.rs` → `kf parts --check` | Overlapping ink per part |
| Books | `apps/cli/src/book.rs` `cmd_book` | Cover + per-song part PDFs (`layout_part` → `render::pdf`) + TACET pages, merged as PDF bytes |

Nothing in this pipeline knows how long a rest lasts in seconds, which
pages face each other, or where in a book a song starts. The greedy packer
only ever runs as a fallback, and its page flow (`y += system_gap`, new
page when the next system does not fit) is first-fit.

## Prior art

**Knuth–Plass (1981)** — total-fit paragraph breaking: every feasible break
is a node, a line between two nodes has badness from its stretch ratio
(`100·|r|³`), demerits add penalties, dynamic programming finds the global
minimum. Music adaptation: **Hegazy & Gourlay (1987), "Optimal line breaking
in music"** — the same DP with measure springs instead of glue. This is the
core of our line breaker.

**LilyPond** (GPL; `lily/constrained-breaking.cc`, `page-breaking.cc`,
`optimal-page-breaking.cc`, `page-turn-page-breaking.cc`,
`page-turn-engraver.cc`):

- `ly:optimal-breaking` (default) minimises horizontal *and* vertical
  stretch together. `Constrained_breaking` precomputes, for a range of
  measures, the best line breaking into exactly *n* systems; the page
  breaker then searches over how many systems each page gets.
  `page-spacing-weight` (default 10) weighs page (vertical) force against
  line (horizontal) force. Reusable as-is: **the "best k-system breaking of
  a range" table feeding a page DP** is exactly our joint structure.
- `ly:page-turn-breaking` runs the optimal breaker *between* allowed turn
  points, giving priority to breaks at places marked `\allowPageTurn`.
  `Page_turn_engraver` marks them automatically: a passage with **no notes**
  (rests in one voice are not enough) at least `minimumPageTurnLength`
  long (default one whole note) gets an allowed turn at its last barline or
  at a special barline (double bar) inside it; it also refuses turns inside
  volta repeats shorter than `minimumRepeatLengthForPageTurn`. When a turn
  would land after a left page, the breaker inserts a blank page and
  charges `blank-page-penalty` (default 5); `blank-last-page-penalty`
  (0) charges ending on an even page, `blank-after-score-page-penalty` (2)
  a blank between pieces; `auto-first-page-number` lets the first page
  number float odd/even. Reusable: the turn-point discovery rule, the
  recto/verso constraint, blank-page penalties, and the floating first
  page (= our book parity choice). Not reusable: turns are binary
  (allowed/not) — no tempo, no "tight vs comfortable".

**MuseScore 4** (GPL, cloned at `/Volumes/dev-drive/reference/MuseScore`,
porting allowed):

- Line breaking is greedy: `SystemLayout::collectSystem`
  (`rendering/score/systemlayout.cpp` ~L225) adds measures until
  `curSysWidth > targetSystemWidth`, then backs off past `noBreak`
  measures; an over-full single measure is squeezed
  (`HorizontalSpacing::squeezeSystemToFit`). The last system is justified
  only when its fill ≥ `lastSystemFillLimit` (0.3,
  `SystemLayout::shouldBeJustified`). Reusable: the squeeze/justify rules
  and the last-system threshold, not the breaking.
- Page breaking is greedy too (`PageLayout::collectPage`): systems stack
  at `SystemLayout::minDistance` (skyline + `minVerticalDistance` 0.5 sp —
  already ported as `stack_systems::needed`) until the page is full. No
  page-turn awareness anywhere.
- Vertical justification is the valuable part.
  `PageLayout::layoutPage` (legacy) equalises system-to-system gaps by
  growing the smallest gaps first, capped at `maxSystemDistance` (15 sp).
  `PageLayout::distributeStaves` (`enableVerticalSpread`) does the same per
  `VerticalGapData` gap with stretch factors (`spreadSystem` 2.5), limits
  `minSystemSpread` 8.5 sp / `maxSystemSpread` 32 sp, then a capped final
  fill (`maxPageFillSpread` 6 sp per gap). We port this shape in §
  Vertical spacing.

**Dorico** (commercial, documentation only): per-layout "make page turns"
with a minimum rest duration and automatic "V.S." — confirms the
user-facing model (a minimum rest + a warning marker) but nothing to port.

## Pipeline

```
prepare_measures ──► Prepared items + MeasureMetrics        (today)
        │
timing::TempoMap ──► seconds per item, silence at each item edge   (new)
        │
breaking::candidates ─► BreakPoint per item boundary (+ inside multirests)
        │
breaking::system_cost(i, j) for feasible ranges              (new, phase 3)
        │
paginate::solve ──► systems + pages, turn windows             (new, phase 2)
        │           (estimated heights; re-solve once with measured
        │            skylines if a page overflows)
render systems (today's per-system code, unchanged)
        │
vspace::justify ──► final staff tops                          (replaces the
        │                                         auto path of stack_systems)
assemble pages + LayoutReport
```

`--layout finale` skips breaking/paginate entirely and keeps today's
`explicit_systems` + `stack_systems(explicit = true)` path byte-for-byte.

## Data model

All new types live in `features/engraver/src/score/` in new modules
`timing.rs`, `breaking.rs`, `paginate.rs`, `vspace.rs`, `report.rs`.

```rust
/// How the source's layout is used.
pub enum LayoutMode {
    /// Stored Finale systems/pages (today's default; stale-check fallback
    /// still applies).
    Finale,
    /// Today's greedy packer (kept for comparison until phase 3 lands).
    Greedy,
    /// This spec: optimal line + page + turn breaking.
    Auto,
}

/// How the reader sees pages.
pub enum Binding {
    /// Duplex, bound or spiral book: pages 2k and 2k+1 (1-based, k ≥ 1)
    /// face each other; only a break after an odd (recto) page is a turn.
    Bound,
    /// Loose single-sided sheets laid out on a stand: up to `view` pages
    /// visible side by side (2 normally, 3 with a fold-out); a turn is
    /// moving past the current view.
    Loose { view: u8 },
    /// One page at a time (tablet portrait): every page break is a turn.
    Single,
}

pub struct TurnModel {
    pub binding: Binding,
    /// Book page number of this part's first page (1 = recto). Parity
    /// is what matters; `kf parts` defaults to 1, books set it.
    pub first_page: u32,
    /// Seconds of silence a turn needs: comfortable / minimum / hard floor.
    pub good_s: f64,   // 4.0
    pub min_s: f64,    // 2.0
    pub floor_s: f64,  // 1.2
    /// Quarter notes per minute when the score has no tempo mark.
    pub default_qpm: f64, // 120
}

/// One place a line (and so maybe a page) can break: after item `item`,
/// or inside a multirest after `bar` of its bars.
pub struct BreakPoint {
    pub item: usize,
    pub inside: Option<u32>,
    /// Silence ending here / starting here, seconds and bars.
    pub silence_before: Silence,
    pub silence_after: Silence,
    /// Musical weight of breaking here (negative = good place).
    pub penalty: f64,
    pub forced: bool,      // explicit override from the sidecar file
    pub forbidden: bool,   // tie across, override, mid-tuplet
}

pub struct Silence {
    pub seconds: f64,
    pub bars: f64,
    /// Tempo came from a mark (false = `default_qpm` assumed).
    pub tempo_known: bool,
    /// A fermata/caesura sits at the edge (adds unmeasured time).
    pub fermata: bool,
}

/// Output of the solver, also the source of `--turns`.
pub struct LayoutReport {
    pub systems: Vec<PlannedSystem>,   // item range, page, stretch ratio
    pub pages: Vec<PageStat>,          // fill ratio, systems, justified?
    pub breaks: Vec<PageBreakStat>,    // each page break: turn? window, verdict
    pub cost: CostBreakdown,           // line / fill / turn / pages / blanks
}
```

`PartLayout` gains `report: Option<LayoutReport>` (filled for every mode —
for `Finale` it describes the stored layout, which is what phase 1
measures). `LayoutOptions` gains `layout: LayoutMode`, `turns: TurnModel`
and `overrides: Option<BreakOverrides>`.

## Timing: silence in seconds

`timing.rs` builds a score-wide **tempo map** once per score:

- Tempo marks are `DirectionKind::Tempo(TempoMark)` on any part's
  measures; union them by measure/tick (parts often carry the mark only on
  a few staves). `per_minute` takes the first number of a range
  (`"120-126"` → 120); `beat_unit` + `dots` convert to quarters per minute
  (dotted quarter = 1.5 q). Words without a number ("Slower", "a tempo")
  do not change the map but are logged as `tempo_known = false` from that
  point on.
- Bar length in quarters from `Measure::len_ticks / divisions`
  (pickups use their actual length).
- `seconds(bar) = quarters(bar) · 60 / qpm(bar)`.

Per part, per item, from the primary voice (same choice `prepare_measures`
makes):

- `lead_silence`: time from the bar start to the first sounding note.
- `trail_silence`: time from the end of the last sounding note to the bar
  end. A note **tied across** the barline makes both 0 at that boundary.
- A whole-bar rest or multirest: the full length. **Cue notes count as
  silence** (the player does not play them).
- Grand staves: silence is the intersection over engraved staves (the
  same "rests group where every staff rests" rule `layout_part` uses).

The turn window at a break between items `a` and `b` is
`W = trail_silence(a…) + lead_silence(…b)`, accumulated backwards and
forwards across consecutive silent items (a page ending in two bars of
rest followed by a page starting with one bar of rest has three bars of
window). LilyPond's "no notes in any voice" rule falls out of using the
primary-voice-plus-intersection silence.

## Page-turn model

### Which page breaks are turns

| Binding | Page break after page *p* (book numbering) is a turn when |
| --- | --- |
| `Bound` | *p* is odd (recto → next verso). Even→odd breaks are free: both pages are open. Page 1 of the book is the cover (recto). |
| `Loose { view: v }` | *p* is the last page of the current view. Views are consecutive groups of ≤ *v* pages; a song of ≤ *v* pages has none. A 3-page view is a fold-out and costs `P_foldout` per use. |
| `Single` | always. |

`Bound` also allows a **fold-out**: a single extra leaf glued to a recto
that opens to the right, making a 3-page spread. It is physical work for
the librarian, so it is only an escalation (see Impossible cases) and is
reported as such.

A non-turn page break still has a small cost (`P_glance`) — the eye jumps
across the gutter — so equal layouts prefer fewer pages.

### Turn cost

For a turn with window `W` seconds:

```
turn_cost(W) =
    P_turn                                              W ≥ good_s
    P_turn + P_tight · ((good_s − W)/(good_s − min_s))²  min_s ≤ W < good_s
    P_turn + P_tight + P_bad · (min_s − W)/(min_s − floor_s)   floor_s ≤ W < min_s
    ∞  (escalate; see Impossible cases)                 W < floor_s
```

Modifiers:

- `tempo_known = false`: evaluate `W` at `0.75 · default_qpm` *and*
  `1.5 · default_qpm` and take the worse cost — an unknown tempo must be
  safe both ways, and bar counts alone give the floor
  (≥ 1 bar of 4/4 at 180 qpm ≈ 1.3 s).
- Window inside a **multirest**: −`P_mm_turn` bonus (the canonical place;
  players expect it).
- Fermata at the edge: treat as +1.5 s.
- Family factor from `book::part_role`: strings sharing a stand
  (`Violin`, `Viola`, `Cello`, `Bass`) multiply `min_s`/`floor_s` by 0.6
  (the inside player turns while the outside player plays); harp,
  keyboard and percussion multiply by 1.5 (two hands busy, instrument
  changes).
- Turns in the **last 2 bars before a song end** are free (the song ends;
  the next page belongs to the next song) — only relevant inside books.

Initial constants (tuned in phase 2 against hand-judged turns):

| Constant | Value | Meaning |
| --- | --- | --- |
| `P_turn` | 40 | any turn, even comfortable |
| `P_tight` | 400 | a turn at `min_s` |
| `P_bad` | 4000 | slope below `min_s` |
| `P_glance` | 5 | non-turn page break |
| `P_mm_turn` | 20 | bonus for turning inside a multirest |
| `P_foldout` | 1500 | one fold-out page |
| `P_page` | 60 | one more page in a part |
| `P_blank` | 120 | one inserted blank page in a book |

## Line breaking

### Break points

Between every pair of prepared items, and **inside multirests** of ≥ 8
bars at every bar from bar 4 to n−4 (splitting a multirest is what makes a
mid-rest turn possible; `MultiRestBlock` gains `split: Option<u32>` and the
two halves get their own counts and first-bar numbers). Each break point
carries a penalty (negative = attractive):

| Situation at the boundary | Penalty |
| --- | --- |
| Section-end barline (`BarStyle::is_section_end`) | −60 |
| Next item starts a rehearsal mark / section badge (`section_marks`) | −80 |
| Next item has a tempo or key/time change | −30 |
| Before a multirest / after a multirest | −25 / −15 |
| Inside a multirest (split) | +30 |
| Slur or hairpin crossing the boundary | +20 |
| Note tied across the boundary | +60 (forbidden for page breaks unless forced) |
| Inside a tuplet or beam group spanning the bar | forbidden |
| Sidecar override `break` / `nobreak` | forced / forbidden |

### System cost

For a candidate system holding items `[i, j)` with prefix width `P`
(`calculate_prefix_width`: clef + key + time, depends on the first item's
key and whether a time signature shows) and available line width `A`:

- `M = Σ min_width` (from `MeasureMetrics`, compact build — the hard
  floor; never compressed below, matching `proportional_widths`).
- `N = Σ natural_width`, natural width = the spring result at the target
  density (`base_measure_width` scaled by weight via
  `chart_spacing::duration_stretch`), i.e. what
  `distribute_measure_widths_spring` gives with unbounded room.
- Stretch ratio `r = (A − P − N) / stretchability` when `A − P ≥ N`
  (stretchability = `N` itself: proportional springs), shrink ratio
  `r = (A − P − N) / (N − M)` ∈ [−1, 0) otherwise. Infeasible if
  `A − P < M`.

```
badness(r)   = 100 · |r|³            (shrink weighs ×2: crowding reads worse)
system_cost  = (1 + badness + max(penalty(j), 0))²
               − min(penalty(j), 0)²                 // TeX demerits shape
             + P_lone_mm   if the system is only multirests and not forced
             + P_orphan    if a non-final system has a single notated bar
             + P_widow     if the final system has < 2 notated bars
             + P_dense · max(0, notated_bars − 8)²   // soft cap, not 6
```

The **final system** of a part (or before a forced break) is ragged: it
pays only shrink badness, and is justified only when its natural fill is
≥ 0.7 (MuseScore uses `lastSystemFillLimit` 0.3; parts look better ragged
earlier). That matches today's `total_to_distribute` rule.

### Consistency

Readers find a part easier when systems have similar bar counts and
similar density. Between consecutive systems *s−1, s* on the same page:

```
consistency = w_bars · (bars(s) − bars(s−1))² + w_ratio · (r(s) − r(s−1))²
```

with multirests counting as one bar. This makes the DP state
`(break point, bars in previous system)`; bars per system ≤ 12, so the
state grows by ≤ 12×.

### Finale affinity (optional)

When the source has a non-stale `ExplicitLayout`, a break point where
Finale broke gets `−w_finale` (default 15). This keeps the arranger's
phrasing where it costs little, and is how `Auto` should look "like the
Finale part, minus the bad turns". `--finale-affinity 0` turns it off.

Constants: `P_lone_mm` 300, `P_orphan` 250, `P_widow` 150, `P_dense` 40,
`w_bars` 25, `w_ratio` 400.

### DP

Over break points `b_0 < … < b_n` (n ≈ 50–200 items per part, plus
multirest splits): `best[b][k]` = minimum total cost of laying out
everything before `b` with the last system holding `k` bars. Transition
over the next break `b'` with `b' − b` items while the system is feasible
(the run stops as soon as `M` exceeds the line). O(n · L · K) with L ≈ 12
items per system and K ≈ 12; microseconds per part. The table we actually
need for pages is LilyPond's constrained form, below.

## Page breaking with turns

Line and page breaking are solved **jointly**: the page DP chooses page
breaks, and inside each page the line DP chooses system breaks.

### Heights before rendering

Rendering systems (skylines) only after breaks are known would make the
joint search blind. Each prepared item gets a cheap **height profile**
during `metric_of`: north/south extent of its compact build (the same
build that gives `min_width`) plus its directions' vertical reach
(dynamics, hairpins, rehearsal badges — badges sit in the margin gutter
and do not count). Estimated system extent = max over items; estimated
gap between systems = MuseScore's `minDistance` approximation
`max(north(s+1) + south(s), 4 sp) + 0.5 sp`, never less than the preferred
`system_gap`'s compressible floor (below). Page room = `bottom_limit_of(p)
− top_limit_of(p)` from today's `stack_systems` helpers (page 1 minus the
title block / keyflow header; later pages minus the running header and
footer).

### Page cost

For a page holding systems with estimated total height `U` and room `H`
(gaps at their preferred size), fill `f = U / H`:

```
fill_cost(f) =
    ∞                              U_min > H · (1 + overfill_tol)   // even compact gaps overflow
    P_overfill · ((U_min − H)/H)   H < U_min ≤ H·(1+overfill_tol)   // escalation only
    w_fill · ((0.85 − f)/0.85)²    f < 0.85, non-final page
    0                              otherwise / final page of the part
+ P_one_system                     page holds one system and is not last
```

`U_min` uses the compact gaps (skyline minimum). Constants: `w_fill` 200,
`P_one_system` 300, `overfill_tol` 0.03, `P_overfill` 2000.

### The joint DP

Let `pages(i → j, p)` be the best way to put break points `i..j` on book
page `p` (fill cost + line cost of the best system breaking of that range
into however many systems fit — LilyPond's "best breaking of a range into
n systems", memoised per `(i, j)` since `p` only changes the room on page
1). Then:

```
F[j][p] = min over i < j:
            F[i][p−1] + pages(i → j, p) + break_cost(j, p)
break_cost(j, p) = turn_cost(W(j))   if page p ends in a turn (binding, parity)
                 = P_glance          otherwise (or 0 at the part's end)
                 + P_page            per page
```

`p` is the absolute book page (its parity is what matters), so the state is
`(break point, pages used)`; pages per part ≤ 8. The consistency term
couples the last system of one page to the first of the next; it is
dropped across page breaks (a new page is a fresh visual start).
Complexity per part: `O(n² · pages)` page transitions × a memoised line
DP per range; with n ≤ 200 this is well under 100 ms per part, ~2 minutes
for a 38-book run with both parities — acceptable, and parallel per part.

After the solve, systems are rendered for real; if a measured page
overflows (estimate too low), the offending page's height estimate is
corrected with the measured skylines and the DP is re-run once from that
page on. A second overflow falls back to today's compact stack for that
page and is reported.

### Impossible cases — the escalation ladder

When no layout keeps every turn above `floor_s`, the solver does not fail;
the costs make it pick the cheapest of these, in roughly this order of
preference, and the report names which one it used:

1. **Tight turn** (`floor_s ≤ W < min_s`): accepted with a `V.S.` mark
   (*volti subito*) at the bottom right of the page, small italic, right
   aligned under the last barline. Always drawn for any turn with
   `W < good_s`.
2. **Slight overfill**: gaps compressed to the skyline minimum, then up to
   3 % of the page height taken from the bottom margin (never into footer
   text).
3. **Underfill**: end a page early (a half-empty page before a turn is
   better than a turn in the music — this is exactly the case today's
   `< 50 %` re-flow destroys; under `Auto` an underfilled page is a cost,
   not a trigger).
4. **Fold-out** (`P_foldout`): a 3-page spread. Marked in the report and
   with a "FOLD-OUT" tag in the page footer so the librarian builds it.
5. **Smaller staff** for this song only (one 5 % step of spatium,
   `P_shrink` 800) — only in phase 4, because it breaks book consistency.
6. **Bad turn**: below the floor, cost ∞ is replaced by `P_impossible`
   (20 000) so the layout still completes; reported as `BAD`, drawn with
   `V.S.` — a human decides (cut a cue, photocopy a page).

Turn cues (the first bars of the next page duplicated at the page bottom)
are a known engraver's trick and a possible later rung, not in scope.

## Book-level optimisation

### Parity variants

`layout_part` is run per (song, chair) with `first_page` odd and even —
two `LayoutReport`s with costs `C_odd`, `C_even` (both cheap; rendering
happens once, for the chosen variant). A 1-page part has equal costs; a
2-page part starting on an even (verso) page has no turn at all.

### Book DP

`cmd_book` today renders and merges PDFs song by song. It becomes two
passes: plan (layouts + reports for both parities), then render the chosen
plan. Over the setlist, state = parity of the next free book page:

```
G[s][q] = min over choices for song s starting at parity q':
            G[s−1][q] + blanks(q → q') · P_blank
          + C_{q'}(s) + pages(s) · P_book_page
```

Choices before a song: start where we are, or insert one blank page
(flips parity). Page 1 is the cover (recto), so song 1 can start on page 2
(verso) for free.

Things the book DP can use besides blanks:

- **TACET pages.** A chair's TACET page is a page anyway. Consecutive
  tacet songs (or a tacet next to a section break) may **share one page**
  ("TACET — I Wouldn't Wanna Be Like You · Prime Time"), or a TACET may
  move to the back of the previous song's last page when that page has
  room (a "TACET: next song" line under the final system) — each option
  changes parity at no paper cost.
- **Section breaks** (`SetItem::Break`, "Intermission") are natural places
  to absorb a blank: `P_blank` halved there.
- **Between songs** a turn costs `P_turn` only (applause/talk), never the
  tight/bad terms.

Book-wide constraints:

- **One staff size per book.** The chair's spatium is fixed for the book:
  the mode of its parts' Finale spatia (they are mostly identical; the
  Columbus parts use one size per page format). Songs that would need
  more pages than an available alternative are not shrunk unless rung 5
  above fires, and then the report says so.
- **One page size per book.** Today a book takes the first engraved
  part's page size for covers and tacets; parts in a different format are
  merged as-is. `Auto` lays every part out on the book's page size
  (9×12 or letter) — a re-layout, not a scale — which is a second reason
  the auto path matters.
- **`--pdf-parts` fallbacks** (existing PDFs) are fixed: known page count,
  unknown turns. They participate in the parity DP with
  `C = P_unknown_turns · (turns implied by parity)`.

Constants: `P_book_page` 40 (paper/thickness), `P_unknown_turns` 200.

## Vertical spacing

Once a page's systems are known (`vspace.rs`, replacing the automatic tail
of `stack_systems`; the `explicit` branch stays for `--layout finale`):

1. **Minimum gaps**: `needed(above, below)` from the measured skylines
   (unchanged: skyline min distance + 0.5 sp, floor 4.5 sp).
2. **Preferred gap**: `max(needed, system_gap)` where `system_gap` is the
   page format's preferred top-to-top distance (74 pt at 6.5 pt spatium ≈
   11.4 sp; Finale's stored spacing when the source has it).
3. **Equalise** (MuseScore `layoutPage`): sort gaps by their free space
   (gap − needed); raise the smallest first towards the next, distributing
   the page's slack, never above `max_gap` = 18 sp of free space
   (MuseScore `maxSystemDistance` is 15 sp for scores; parts have fewer,
   taller systems and look right a bit looser).
4. **Fill**: non-final pages are justified to the bottom limit when the
   slack per gap stays ≤ `max_gap`; otherwise the leftover stays at the
   bottom (ragged), capped like `maxPageFillSpread`.
5. **Final page** of a part: justified only when its fill ≥ 0.7, else
   systems sit at the preferred gap from the top (ragged bottom).
6. **Tolerances** recorded per page: `min_gap_sp`, `max_gap_sp`,
   `gap_spread` (max − min free space) — `gap_spread` > 4 sp is a metric
   warning (one system floating far from its neighbours).

Grand-staff parts use the existing `GRAND_STAFF_DISTANCE_SP` within a
system; only system-to-system gaps stretch (no `spreadCurlyBracket`
analogue needed yet).

## Metrics and regression harness

### `kf parts --turns`

Mirrors `--check`: lays out, prints, writes nothing.

```
$ kf parts "Time.musx" --turns --layout finale
Violin 1        3 pp  13 sys  bars/sys 5.2±1.4  fill .91 .88 .41  cost 1840
    p1→p2  glance   m.34            —
    p2→p3  TURN     m.71   0.0 s  (0 bars, ♩=96)    BAD   tie across
Horn 1          2 pp   9 sys  bars/sys 6.0±2.1  fill .93 .52        cost 210
    p1→p2  TURN     m.48  15.0 s  (6 bars mm, ♩=96) OK
...
Time: 23 parts, 41 pages, 18 turns (OK 12, TIGHT 3, BAD 3), 2 lone-mm systems
```

Per part: pages, systems, bars per system mean ± stdev, per-page fill,
lone-multirest and orphan systems, min stretch ratio, cost breakdown. Per
page break: book page numbers, turn or glance, bar number, window in
seconds and bars, tempo (and whether assumed), verdict
`OK | TIGHT | BAD | FOLDOUT`, and the reason for a zero window (tie, note
on the barline). `--binding` and `--start recto|verso` apply, so a single
part can be judged as it would sit in a book.

`--turns --tsv` writes one row per page break and one per part for
diffing.

### `kf book --turns`

The plan pass only: per chair, every song's start page (L/R), turns, blank
pages and TACET placements; totals per chair and for the set. This is the
acceptance report for the Columbus books.

### Library sweep

A small script (`scripts/parts-turns-sweep.sh`, or `kf parts --turns
--summary <dir>`) runs every `.musx` under
`/Users/codywright/Downloads/APLP Finale Scores Complete/By Song` (46
folders; a few songs exist twice — `Breakdown-Freedom` /
`Breakdown - Freedom` — both are swept and listed separately) with each
`--layout` and writes:

| Total | Meaning |
| --- | --- |
| parts, pages | size |
| turns, turns_ok / tight / bad | the headline |
| bad_turn_parts | parts with ≥ 1 BAD turn |
| mean_fill (non-final pages), pages_below_60 | page economy |
| bars_per_sys_stdev (mean over parts) | consistency |
| lone_mm, orphans | line quality |
| collisions | from `inspect::collisions` — must not regress |
| foldouts, overfills, tempo_assumed_parts | escalations |

Baselines live next to the data
(`/Volumes/dev-drive/aplp-parts/metrics/turns-<layout>-<date>.tsv`), not
in the repo (the library is not in the repo).

### Tests in the repo

- `timing.rs`: tempo-mark parsing (`"120-126"`, dotted-quarter beats,
  words-only marks), seconds across meter changes, tie-across silence = 0,
  cue notes count as silence.
- `breaking.rs`: synthetic item lists — equal-width bars give equal
  systems; a forced break is honoured; a rehearsal boundary wins over an
  equal-cost neighbour; nothing is ever below `M`; a long multirest splits
  only when a turn needs it.
- `paginate.rs`: a constructed part whose only safe turn is mid-page
  gets an underfilled page; `Bound` parity — a 2-page part starting verso
  reports zero turns, recto reports one; the escalation ladder picks
  tight turn before fold-out.
- `vspace.rs`: equalisation never drops a gap below `needed`; final-page
  ragged rule.
- Fixtures are synthetic `Score`s built in code, under
  `features/engraver/tests/fixtures/` when they are files. The APLP
  scores are commercial and the repo is public, so a golden `--turns
  --tsv` over a real song is an `#[ignore]`d test that reads a local copy
  (as the other corpus tests do), never a committed file.

## Overrides

A song's `.roles` file (already per song, in
`/Volumes/dev-drive/aplp-parts/roles`) gains a layout section, so
decisions live with the other per-song text decisions:

```
layout Horn 1: break after 48; nobreak 71; page after 112
layout *: turn-ok 71        # a human judged this turn playable
```

`break`/`nobreak` set `forced`/`forbidden` on break points; `page after`
forces a page break; `turn-ok` lifts a turn's verdict to OK (e.g. a
player who can turn with the bell hand).

## Integration points

| Code | Change |
| --- | --- |
| `layout.rs` `LayoutOptions` | `layout: LayoutMode`, `turns: TurnModel`, `overrides` |
| `layout.rs` `layout_part` | split into `prepare_part` (prepared, lower staves, metrics + height profiles) → `plan_systems` → render loop (unchanged) → `place_systems` → assemble; the `None =>` branch of the system plan moves to `LayoutMode::Greedy` |
| `layout.rs` `MeasureMetrics` | + `natural_width`, `north`, `south`; + silence fields via `timing` |
| `layout.rs` `MultiRestBlock` / `prepare_measures` | split support (`split` bar); halves numbered and badged like any multirest |
| `layout.rs` `stack_systems` | explicit branch unchanged; automatic tail replaced by `vspace::justify`; the underfill/overflow re-flow only applies to `Finale` |
| `layout.rs` `PartLayout` | + `report: Option<LayoutReport>` |
| `MAX_MEASURES_PER_SYSTEM` | stays for `Greedy` and for `base_measure_width`; `Auto` uses the soft cap |
| `finale.rs` | unchanged; `ExplicitLayout` also feeds Finale affinity |
| `render.rs` | `V.S.` / `FOLD-OUT` marks are scene text added in assemble — no renderer change |
| `apps/cli/src/parts.rs` | `--layout finale|greedy|auto` (`--auto-layout` stays as a hidden alias for `greedy`), `--turns`, `--tsv`, `--binding`, `--start`, `--finale-affinity` |
| `apps/cli/src/book.rs` | plan pass (parity variants + book DP), render pass; TACET sharing; `--turns`; book spatium/page size passed into each `LayoutOptions` |
| `section_marks.rs` / roles parsing | `layout` lines in `.roles` |

## Phased plan

- **Phase 1 — metrics only.** `timing.rs`, `LayoutReport` built from
  whatever layout ran, `kf parts --turns [--tsv]`, `kf book --turns`
  (with today's layouts), library sweep + first baseline for `finale` and
  `greedy`. No layout changes. Acceptance: report runs over all 46 songs;
  five turns hand-checked against the printed Columbus parts; tempo known
  for ≥ 90 % of songs (else add tempo overrides to `.roles`).
- **Phase 2 — page breaking with turns over fixed lines.** `paginate.rs`
  + `vspace.rs`, with line breaks taken from Finale (non-stale) or
  `Greedy`. Replaces the `< 50 %` / overflow re-flow under `Auto`.
  Acceptance vs the phase-1 `finale` baseline: BAD turns down by ≥ 80 %
  where the line breaks allow it, page count up ≤ 5 %, collisions not up.
- **Phase 3 — line breaking.** `breaking.rs` system DP, height profiles,
  joint DP, multirest splitting, Finale affinity, overrides. Acceptance:
  BAD turns only where the report shows no rest ≥ `floor_s` within reach;
  bars-per-system stdev ≤ Finale's on ≥ 70 % of parts; zero lone-mm
  systems except forced; collisions not up; a visual review of 10 parts
  side by side with the Finale layout.
- **Phase 4 — book spreads.** Parity variants, book DP, blanks, TACET
  sharing, book spatium/page size, fold-out and shrink rungs. Acceptance:
  every 1–2-page song in the Columbus books turn-free; blanks per book ≤ 3;
  `kf book --turns` totals reviewed with the librarian before `Auto`
  becomes the book default.

`--layout finale` stays the default for `kf parts` until phase 3 passes;
`kf book` switches after phase 4.

## Risks

- **Repeats and jumps.** The model has no repeat barlines, voltas, D.S. or
  coda. A turn inside a repeat means turning *back*; a D.S. means flipping
  pages backwards. Until the importer models them, `timing` scans
  `Words` for `D.S.`, `D.C.`, `Coda`, `To Coda` and the report flags
  those parts; a page break inside a detected repeat span is penalised
  like a tight turn. Modelling repeats is a prerequisite for calling a
  part "turn-safe" without review.
- **Tempo uncertainty.** Rubato, fermatas, tempo words without numbers,
  tempo in the conductor score only. Mitigated by the both-ways evaluation
  and `.roles` overrides; the report always says when tempo is assumed.
- **Height estimates.** Wrong estimates cause overflow after rendering;
  the single re-solve plus compact fallback bounds the damage. Measure the
  estimate error in phase 3 (estimated vs measured extent per system).
- **Arranger intent.** Finale layouts carry deliberate choices (a short
  system before a solo, a cue page). Finale affinity and `.roles`
  overrides keep them reachable; `--layout finale` keeps them exactly.
- **Multirest splitting** changes bar-count badges and numbering; it must
  follow the same numbering rules as `prepare_measures`' existing breaks
  (rehearsal marks already split runs).
- **Cost tuning.** The constants are guesses; phase 1 metrics and a small
  set of hand-judged layouts are the tuning set. Keep all constants in one
  `Costs` struct so the sweep can vary them.
- **Book plan cost.** Two parities × 38 chairs × 16 songs is ~1200 part
  layouts per book run, without rendering; parallelise per song and cache
  `LayoutReport`s by (musx hash, part, options).
- **Players differ.** A strings stand partner, a doubling wind player, a
  percussionist moving between instruments — the family factor is crude.
  The `turn-ok` override and per-chair thresholds are the escape hatch.
- **Printing.** `Bound` assumes duplex printing with page 1 on a recto.
  A single-sided print run silently breaks the model; the book cover
  should state "print double-sided" and `--binding loose` exists for
  stand copies.
