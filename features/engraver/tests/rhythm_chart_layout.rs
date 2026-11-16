//! A Master Rhythm chart, engraved the way a pro chart spaces it.
//!
//! The fixture is the bars of "This Christmas" that broke: an intro hook of
//! eight chord changes with a 3/4 turnaround, a verse that mixes plain
//! slashes with written note values, and a chorus bar of `C/F_8 F_4.
//! B7(b5)_2`. Each test pins one thing a reader relies on:
//!
//! - a chord symbol's left edge sits over the slash it is played on,
//! - slashes are spaced by time, not squeezed to the end of the bar,
//! - written note values print as written (a half is a half-note slash),
//! - the hook and its turnaround share a line, as the reference chart sets them.

use engraver::api::pipeline::{ChartPipeline, Preset, PresetOptions};
use engraver::engraver::layout::chart::{BeatPosition, ChartLayoutResult};
use engraver::engraver::scene::node::SceneNode;
use engraver::engraver::scene::paint::PaintCommand;
use kurbo::Affine;

const FIXTURE: &str = include_str!("fixtures/rhythm-chart-layout.kf");

/// U+E103 `noteheadSlashWhiteHalf`.
const WHITE_HALF_SLASH: char = '\u{E103}';

// Global measure indices in the fixture.
const VERSE_HALF_AND_REST: usize = 8; // Ebmaj9_2 r2
const VERSE_HALF_QUARTER_QUARTER: usize = 9; // Ebmaj7_2 >Dm7_4 >C11_4
const VERSE_PLAIN: usize = 6; // Fmaj7
const CHORUS_FIGURE: usize = 10; // >C/F_8 F_4. >B7(b5)_2

fn layout() -> ChartLayoutResult {
    let chart = keyflow_lang::text::chart::parse_chart(FIXTURE).expect("the fixture parses");
    ChartPipeline::shared()
        .expect("the pipeline builds")
        .layout_preset(&chart, Preset::Page, PresetOptions::for_export())
}

#[derive(Debug)]
struct Symbol {
    text: String,
    x: f64,
    system: usize,
}

/// Every chord symbol: its text, the world x of its first glyph run, and
/// the system it is on.
fn symbols(scene: &SceneNode) -> Vec<Symbol> {
    fn texts(node: &SceneNode, parent: Affine, out: &mut Vec<(String, f64)>) {
        let t = parent * node.transform;
        for command in &node.commands {
            if let PaintCommand::Text { text, position, .. } = command {
                out.push((text.clone(), (t * *position).x));
            }
        }
        for child in &node.children {
            texts(child, t, out);
        }
    }
    fn walk(node: &SceneNode, parent: Affine, out: &mut Vec<Symbol>) {
        let t = parent * node.transform;
        if node.metadata.get("element_type").map(String::as_str) == Some("chord") {
            let mut runs = Vec::new();
            texts(node, parent, &mut runs);
            if let Some(x) = runs.iter().map(|(_, x)| *x).reduce(f64::min) {
                out.push(Symbol {
                    text: runs.iter().map(|(s, _)| s.as_str()).collect(),
                    x,
                    system: node
                        .metadata
                        .get("system")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(usize::MAX),
                });
            }
            return;
        }
        for child in &node.children {
            walk(child, t, out);
        }
    }
    let mut out = Vec::new();
    walk(scene, Affine::IDENTITY, &mut out);
    out
}

fn beats(result: &ChartLayoutResult, measure: usize) -> Vec<&BeatPosition> {
    result
        .beat_positions
        .iter()
        .filter(|b| b.measure == measure)
        .collect()
}

fn nearest_beat<'a>(result: &'a ChartLayoutResult, symbol: &Symbol) -> &'a BeatPosition {
    result
        .beat_positions
        .iter()
        .filter(|b| b.system == symbol.system)
        .min_by(|a, b| (a.x - symbol.x).abs().total_cmp(&(b.x - symbol.x).abs()))
        .unwrap_or_else(|| panic!("no beats on the system of {symbol:?}"))
}

#[test]
fn every_chord_symbol_sits_on_its_slash() {
    let result = layout();
    let symbols = symbols(&result.scene);
    assert!(symbols.len() > 30, "found only {} symbols", symbols.len());
    for symbol in &symbols {
        let beat = nearest_beat(&result, symbol);
        assert!(
            (beat.x - symbol.x).abs() < 1.0,
            "{symbol:?} is {:.1}pt off the nearest slash (measure {} tick {})",
            symbol.x - beat.x,
            beat.measure,
            beat.tick
        );
    }
}

#[test]
fn the_chorus_figure_puts_each_symbol_on_its_own_attack() {
    let result = layout();
    let symbols = symbols(&result.scene);
    let on = |text: &str| {
        let symbol = symbols
            .iter()
            .find(|s| s.text == text)
            .unwrap_or_else(|| panic!("no `{text}` symbol in {symbols:?}"));
        let beat = nearest_beat(&result, symbol);
        assert!((beat.x - symbol.x).abs() < 1.0, "{symbol:?} vs {beat:?}");
        (beat.measure, beat.tick)
    };
    assert_eq!(on("F"), (CHORUS_FIGURE, 240), "F is played on the and of 1");
    // C/F has an eighth before F, so it stacks into a vertical fraction:
    // its slash is drawn as a stroke, not a `/` glyph, and its text runs
    // are just the root and the bass.
    assert_eq!(on("CF").0, CHORUS_FIGURE, "C/F starts the chorus figure");
    let b7 = symbols
        .iter()
        .find(|s| s.text.starts_with('B') && s.text.contains('5'))
        .expect("the B7(b5) symbol");
    let beat = nearest_beat(&result, b7);
    assert_eq!((beat.measure, beat.tick), (CHORUS_FIGURE, 960), "{b7:?}");
}

fn count_glyphs(node: &SceneNode, glyph: char) -> usize {
    let own = node
        .commands
        .iter()
        .filter(|command| {
            matches!(command, PaintCommand::Glyph { codepoint, .. } if *codepoint == glyph)
        })
        .count();
    own + node
        .children
        .iter()
        .map(|c| count_glyphs(c, glyph))
        .sum::<usize>()
}

#[test]
fn rhythm_figures_print_as_written_and_groove_bars_as_beat_slashes() {
    let result = layout();
    let ticks = |m| beats(&result, m).iter().map(|b| b.tick).collect::<Vec<_>>();
    // Figures: a hit and a stop, an accented change, an eighth pickup.
    assert_eq!(
        ticks(VERSE_HALF_AND_REST),
        vec![0, 960],
        "a held half, then a half rest"
    );
    assert_eq!(ticks(VERSE_HALF_QUARTER_QUARTER), vec![0, 960, 1440]);
    assert_eq!(ticks(CHORUS_FIGURE), vec![0, 240, 960]);
    // Groove bars: `Fadd2_2 Dm_4` is a change on beat 3, not a hold.
    assert_eq!(
        ticks(1),
        vec![0, 480, 960],
        "the 3/4 turnaround is three beats"
    );
    // A plain bar of one chord is still four beat slashes.
    assert_eq!(ticks(VERSE_PLAIN), vec![0, 480, 960, 1440]);

    // Three held halves — Ebmaj9_2 before its rest, Ebmaj7_2 before the
    // accents, B7(b5)_2 after the eighth — each a hollow half-note slash.
    assert_eq!(
        count_glyphs(&result.scene, WHITE_HALF_SLASH),
        3,
        "only a held half is a half-note slash"
    );
}

fn glyph_xs(node: &SceneNode, parent: Affine, glyph: char, out: &mut Vec<f64>) {
    let t = parent * node.transform;
    for command in &node.commands {
        if let PaintCommand::Glyph {
            codepoint,
            position,
            ..
        } = command
            && *codepoint == glyph
        {
            out.push((t * *position).x);
        }
    }
    for child in &node.children {
        glyph_xs(child, t, glyph, out);
    }
}

#[test]
fn a_chord_tied_over_the_barline_draws_a_tie_and_is_not_named_again() {
    // Joy to the World's intro: the G is held into the next bar.
    fn ties(node: &SceneNode) -> usize {
        usize::from(node.metadata.get("element_type").map(String::as_str) == Some("chord_tie"))
            + node.children.iter().map(ties).sum::<usize>()
    }
    let render = |source: &str| {
        let chart = keyflow_lang::text::chart::parse_chart(source).expect("the chart parses");
        ChartPipeline::shared()
            .expect("the pipeline builds")
            .layout_preset(&chart, Preset::Page, PresetOptions::for_export())
    };
    let tied = render("Joy\n#D 128bpm 4/4\n\nIN 2\nD_2. G_4 ~\nG\n");
    let g_symbols = |result: &ChartLayoutResult| {
        symbols(&result.scene)
            .iter()
            .filter(|s| s.text == "G")
            .count()
    };
    assert_eq!(ties(&tied.scene), 1, "the tie is drawn");
    assert_eq!(g_symbols(&tied), 1, "the held G is not re-attacked");

    // Without the tie bar 2 strikes G again, but G is the harmony already
    // sounding, so the repeat rule leaves its symbol off all the same: the
    // tie is the only difference on the page.
    let struck = render("Joy\n#D 128bpm 4/4\n\nIN 2\nD_2. G_4\nG\n");
    assert_eq!(ties(&struck.scene), 0);
    assert_eq!(g_symbols(&struck), 1);
}

fn render(source: &str) -> ChartLayoutResult {
    let chart = keyflow_lang::text::chart::parse_chart(source).expect("the chart parses");
    ChartPipeline::shared()
        .expect("the pipeline builds")
        .layout_preset(&chart, Preset::Page, PresetOptions::for_export())
}

#[test]
fn one_value_filling_the_bar_is_a_hold() {
    // Silent Night's End bar: `A_2.` in 3/4, with a fermata.
    let result = render("End\n#A 90bpm 3/4\n\nOUT 1\nA_2. \\fermata\n");
    assert_eq!(
        beats(&result, 0).iter().map(|b| b.tick).collect::<Vec<_>>(),
        vec![0],
        "one held value, not three beat slashes"
    );
    assert_eq!(count_glyphs(&result.scene, WHITE_HALF_SLASH), 1);
    assert_eq!(count_glyphs(&result.scene, '\u{E1E7}'), 1, "with its dot");
    assert_eq!(
        count_glyphs(&result.scene, '\u{E4C0}'),
        1,
        "and the fermata over it"
    );
}

#[test]
fn a_key_change_at_a_line_start_cancels_the_old_key() {
    // Sleigh Ride: B major, then a verse in C.
    let result = render("K\n#B 86bpm 4/4\n\nVS 2\nB E\n\nVS 2 #C\nC F\n");
    assert_eq!(
        count_glyphs(&result.scene, '\u{E261}'),
        5,
        "five naturals cancel B major's sharps"
    );
}

#[test]
fn a_fermata_shows_even_when_its_chords_symbol_does_not() {
    // `D // \fermata` on a D already sounding: no symbol, but the hold.
    let result = render("F - x\n#D 90bpm 4/4\n\nVS 2\nD\nD // \\fermata r2\n");
    assert_eq!(count_glyphs(&result.scene, '\u{E4C0}'), 1);
}

#[test]
fn a_marcato_chord_draws_the_wedge() {
    let result = render("Hit\n#C 120bpm 4/4\n\nVS 1\nC_4 \\marcato r4 G_2\n");
    assert_eq!(count_glyphs(&result.scene, '\u{E4AC}'), 1);
}

#[test]
fn a_held_dotted_half_shows_its_dot_clear_of_the_head() {
    // Silent Night's end: `A_2. r4`.
    let chart = keyflow_lang::text::chart::parse_chart("End\n#A 90bpm 4/4\n\nVS 1\nA_2. r4\n")
        .expect("the chart parses");
    let result = ChartPipeline::shared()
        .expect("the pipeline builds")
        .layout_preset(&chart, Preset::Page, PresetOptions::for_export());
    let (mut heads, mut dots) = (Vec::new(), Vec::new());
    glyph_xs(
        &result.scene,
        Affine::IDENTITY,
        WHITE_HALF_SLASH,
        &mut heads,
    );
    glyph_xs(&result.scene, Affine::IDENTITY, '\u{E1E7}', &mut dots);
    assert_eq!(heads.len(), 1, "one held half");
    assert_eq!(dots.len(), 1, "and its augmentation dot");
    // The hollow half slash is about 2.8 spaces wide (5pt spaces).
    assert!(
        dots[0] - heads[0] > 2.5 * 5.0,
        "the dot sits past the head, not inside it: head x={:.1} dot x={:.1}",
        heads[0],
        dots[0]
    );
}

#[test]
fn slashes_written_with_a_slash_stay_beat_slashes_inside_a_figure() {
    // Writing `/` is the writer saying "groove", even in a bar with a hit.
    let chart = keyflow_lang::text::chart::parse_chart(
        "Groove\n#F 93bpm 4/4\n\nCH 3\n>C/F_8 F_4. >B7(b5) //\nF#7sus // >.G7sus / r4\nC#m9 / r2.\n",
    )
    .expect("the chart parses");
    let result = ChartPipeline::shared()
        .expect("the pipeline builds")
        .layout_preset(&chart, Preset::Page, PresetOptions::for_export());
    let ticks = |m| beats(&result, m).iter().map(|b| b.tick).collect::<Vec<_>>();
    assert_eq!(
        ticks(0),
        vec![0, 240, 960, 1440],
        "B7(b5) is two beat slashes"
    );
    assert_eq!(
        ticks(1),
        vec![0, 480, 960, 1440],
        "F#7sus is two beat slashes"
    );
    assert_eq!(
        count_glyphs(&result.scene, WHITE_HALF_SLASH),
        0,
        "no slash-written chord becomes a held half"
    );
}

#[test]
fn slashes_are_spaced_by_time() {
    let result = layout();

    let plain = beats(&result, VERSE_PLAIN);
    let gaps: Vec<f64> = plain.windows(2).map(|w| w[1].x - w[0].x).collect();
    for gap in &gaps {
        assert!(
            (gap - gaps[0]).abs() < 0.5,
            "four quarters, four equal beats: {gaps:?}"
        );
    }

    let mixed = beats(&result, VERSE_HALF_QUARTER_QUARTER);
    let half = mixed[1].x - mixed[0].x;
    let quarter = mixed[2].x - mixed[1].x;
    let last = mixed[2].measure_x1 - mixed[2].x;
    assert!(
        half >= quarter && half <= quarter * 2.0,
        "a half is wider than a quarter, but not by twice: half={half:.1} quarter={quarter:.1}"
    );
    assert!(
        (last - quarter).abs() < quarter * 0.4,
        "the last quarter is as wide as the one before it: {quarter:.1} vs {last:.1}"
    );
}

#[test]
fn the_hook_and_its_turnaround_share_a_line() {
    let result = layout();
    let system_of = |m| {
        beats(&result, m)
            .first()
            .unwrap_or_else(|| panic!("measure {m} has no beats"))
            .system
    };
    for (hook, turnaround) in [(0, 1), (2, 3), (4, 5)] {
        assert_eq!(
            system_of(hook),
            system_of(turnaround),
            "intro bars {} and {} belong on one line",
            hook + 1,
            turnaround + 1
        );
    }
    assert_ne!(system_of(1), system_of(2), "three lines, two bars each");
    assert_ne!(system_of(3), system_of(4), "three lines, two bars each");
}

/// Sleigh Ride's verse: a bar of changes, then the chord it started on.
const CLUSTER_THEN_HELD: &str = "Sleigh Ride verse\n#B 86bpm 4/4\n\nVS 4\n\
     Bmaj7 / G#m7 / C#m9 / F#7 / Bmaj7\nBmaj7\n";

fn printed_b_symbols(source: &str) -> usize {
    let chart = keyflow_lang::text::chart::parse_chart(source).expect("the chart parses");
    let result = ChartPipeline::shared()
        .expect("the pipeline builds")
        .layout_preset(&chart, Preset::Page, PresetOptions::for_export());
    symbols(&result.scene)
        .iter()
        .filter(|s| s.text.starts_with('B'))
        .count()
}

#[test]
fn beat_one_is_named_when_the_harmony_changes_there() {
    // Bar 1 Bmaj7 G#m7, bar 2 C#m9 F#7, bar 3 Bmaj7, bar 4 Bmaj7 held.
    // Bar 3's Bmaj7 is a change from the F#7 sounding before it; bar 4
    // only restates the chord bar 3 held, so it does not print.
    assert_eq!(printed_b_symbols(CLUSTER_THEN_HELD), 2);
}

#[test]
fn a_chord_still_sounding_over_the_barline_is_not_named_again() {
    // Joy to the World: `D /// G /` then a bar of G. The G is still
    // sounding, so the bar names nothing — even though the bar before it
    // had a change.
    let result = render("Joy\n#D 128bpm 4/4\n\nIN 2\nD /// G /\nG\n");
    let g = symbols(&result.scene)
        .iter()
        .filter(|s| s.text == "G")
        .count();
    assert_eq!(g, 1);
}

#[test]
fn a_chord_pushed_in_at_the_end_of_a_bar_is_not_named_again() {
    // This Christmas: the hook ends on a pushed `>Fadd2_8`, and the 3/4
    // turnaround starts on that same Fadd2. It is already named and still
    // sounding, so the turnaround names only its Dm.
    let result = render(
        "TC\n#F 93bpm 4/4\n\nIN 2\nBbmaj7_8 Ebmaj7_8 Am7_16 Dm7_8. Abmaj9_8 Bbmaj7_8 Ebmaj7_8 >Fadd2_8 !T3/4 Fadd2 // Dm /\n",
    );
    let fadd2 = symbols(&result.scene)
        .iter()
        .filter(|s| s.text.starts_with('F'))
        .count();
    assert_eq!(fadd2, 1);
}

#[test]
fn a_bar_of_changes_does_not_rename_a_chord_still_sounding_by_default() {
    // All I Want for Christmas: `G` then `G // B7 //` — the G is still
    // sounding, so by default the bar names only its B7.
    let g = |source: &str| {
        symbols(&render(source).scene)
            .iter()
            .filter(|s| s.text == "G")
            .count()
    };
    assert_eq!(g("AIW\n#G 150bpm 4/4\n\nCH 2\nG\nG // B7 //\n"), 1);
    // `\repeat_chord_symbols = clusters` opts in to naming it.
    assert_eq!(
        g("AIW\n#G 150bpm 4/4\n\\repeat_chord_symbols = clusters\n\nCH 2\nG\nG // B7 //\n"),
        2
    );
}

/// Sleigh Ride, verse bars 1 to 4 as charted: a cluster, a held Bmaj7, then
/// a cluster starting on that same Bmaj7.
const SLEIGH_RIDE_OPENING: &str = "VS 4\n\
     Bmaj7 / G#m7 / C#m9 / F#7 / Bmaj7\n\
     Bmaj7 / G#m7 / C#m9 / F#7 / Bmaj7 // Gmaj7 / F#7 /\n";

#[test]
fn sleigh_ride_bar_three_renames_its_bmaj7_only_when_asked() {
    let header = "Sleigh Ride - Tori Kelly\n#B 86bpm 4/4\n";
    // Default: bar 1 and bar 2 (a change from F#7) name Bmaj7; bar 3's
    // Bmaj7 is still sounding; bar 4's comes back after F#7.
    assert_eq!(
        printed_b_symbols(&format!("{header}\n{SLEIGH_RIDE_OPENING}")),
        3
    );
    assert_eq!(
        printed_b_symbols(&format!(
            "{header}\\repeat_chord_symbols = changes\n\n{SLEIGH_RIDE_OPENING}"
        )),
        3
    );
    assert_eq!(
        printed_b_symbols(&format!(
            "{header}\\repeat_chord_symbols = clusters\n\n{SLEIGH_RIDE_OPENING}"
        )),
        4
    );
}

#[test]
fn by_default_no_still_sounding_chord_is_named_again() {
    // This Christmas, Joy to the World and All I Want for Christmas shapes:
    // a held chord, then a bar of changes beginning on it.
    for (source, chord, expected) in [
        (
            "TC\n#F 93bpm 4/4\n\nVS 2\nBbmaj7\nBbmaj7 // Am7 //\n",
            "B",
            1,
        ),
        ("Joy\n#D 128bpm 4/4\n\nVS 2\nG\nG // A //\n", "G", 1),
        ("AIW\n#G 150bpm 4/4\n\nVS 2\nC\nC // D //\n", "C", 1),
    ] {
        let all = symbols(&render(source).scene);
        let n = all.iter().filter(|s| s.text.starts_with(chord)).count();
        assert_eq!(n, expected, "{source}: {all:?}");
    }
}

/// World y of every glyph `glyph` in the scene.
fn glyph_ys(node: &SceneNode, parent: Affine, glyph: char, out: &mut Vec<f64>) {
    let t = parent * node.transform;
    for command in &node.commands {
        if let PaintCommand::Glyph {
            codepoint,
            position,
            ..
        } = command
            && *codepoint == glyph
        {
            out.push((t * *position).y);
        }
    }
    for child in &node.children {
        glyph_ys(child, t, glyph, out);
    }
}

#[test]
fn an_accented_staccato_chord_draws_both_marks() {
    // Sleigh Ride's pre-chorus button: `>.G7sus` is accent AND staccato.
    let chart = keyflow_lang::text::chart::parse_chart(
        "Button\n#C 120bpm 4/4\n\nPRE 1\nF7sus // >.G7sus / r4\n",
    )
    .expect("the chart parses");
    let result = ChartPipeline::shared()
        .expect("the pipeline builds")
        .layout_preset(&chart, Preset::Page, PresetOptions::for_export());
    let (mut accents, mut dots) = (Vec::new(), Vec::new());
    glyph_ys(&result.scene, Affine::IDENTITY, '\u{E4A0}', &mut accents);
    glyph_ys(&result.scene, Affine::IDENTITY, '\u{E4A2}', &mut dots);
    assert_eq!((accents.len(), dots.len()), (1, 1), "one accent, one dot");
    assert!(
        dots[0] - accents[0] > 3.0,
        "the dot sits nearest the staff with the accent above it, not on top of it: \
         accent y={:.1} dot y={:.1}",
        accents[0],
        dots[0]
    );
}

/// Sleigh Ride's first verse, as charted.
const SLEIGH_RIDE_VERSE: &str = "Sleigh Ride - Tori Kelly\n#B 86bpm 4/4\n\nVS 8\n\
     Bmaj7 / G#m7 / C#m9 / F#7 / Bmaj7\n\
     Bmaj7 / G#m7 / C#m9 / F#7 / Bmaj7 / Cmaj7 / Dmaj7 / F#7 /\n\
     Bmaj7 / G#m7 / C#m9 / F#7 / Bmaj7\n\
     Bmaj7 / G#m7 / C#m9 / F#7 / Bmaj7 ////\n";

#[test]
fn a_verse_of_chord_clusters_keeps_four_bars_a_line() {
    let chart = keyflow_lang::text::chart::parse_chart(SLEIGH_RIDE_VERSE).expect("parses");
    let result = ChartPipeline::shared()
        .expect("the pipeline builds")
        .layout_preset(&chart, Preset::Page, PresetOptions::for_export());
    let system_of = |m| beats(&result, m).first().expect("beats").system;
    for line in [0..4, 4..8] {
        let first = system_of(line.start);
        assert!(
            line.clone().all(|m| system_of(m) == first),
            "bars {} to {} are one line of four",
            line.start + 1,
            line.end
        );
    }
    // A held Bmaj7 after a bar of changes is named (a change from F#7);
    // a cluster after that held Bmaj7 is not, except at a line start (bar 5).
    assert_eq!(printed_b_symbols(SLEIGH_RIDE_VERSE), 6);
    // With `clusters`, every bar names its Bmaj7.
    let clusters =
        SLEIGH_RIDE_VERSE.replacen("4/4\n", "4/4\n\\repeat_chord_symbols = clusters\n", 1);
    assert_eq!(printed_b_symbols(&clusters), 8);
}

#[test]
fn repeat_chord_symbols_always_prints_every_symbol() {
    let source = format!("\\repeat_chord_symbols = always\n\n{CLUSTER_THEN_HELD}");
    assert_eq!(printed_b_symbols(&source), 3);
}

/// Each line of `source`'s first section: (bar widths, the beat xs of each
/// bar).
fn lines_of(source: &str) -> Vec<Vec<(f64, Vec<f64>)>> {
    let chart = keyflow_lang::text::chart::parse_chart(source).expect("parses");
    let result = ChartPipeline::shared()
        .expect("the pipeline builds")
        .layout_preset(&chart, Preset::Page, PresetOptions::for_export());
    let bars = chart.sections[0].measures().len();
    let mut lines: Vec<Vec<(usize, f64, Vec<f64>)>> = Vec::new();
    for m in 0..bars {
        let b = beats(&result, m);
        let first = b.first().expect("the bar has beats");
        let xs = b.iter().map(|p| p.x).collect();
        let entry = (first.system, first.measure_x1 - first.measure_x0, xs);
        match lines.last_mut() {
            Some(line) if line[0].0 == entry.0 => line.push(entry),
            _ => lines.push(vec![entry]),
        }
    }
    lines
        .into_iter()
        .map(|l| l.into_iter().map(|(_, w, xs)| (w, xs)).collect())
        .collect()
}

fn assert_four_a_line_and_sparse_bars_readable(source: &str) {
    let lines = lines_of(source);
    for (i, line) in lines.iter().enumerate() {
        assert_eq!(line.len(), 4, "line {} holds four bars", i + 1);
        let average = line.iter().map(|(w, _)| w).sum::<f64>() / 4.0;
        for (bar, (width, _)) in line.iter().enumerate() {
            assert!(
                *width >= 0.65 * average - 0.5,
                "line {} bar {}: {width:.1} is under 65% of the {average:.1} average",
                i + 1,
                bar + 1
            );
        }
    }
}

#[test]
fn sleigh_ride_verse_keeps_four_a_line_and_its_whole_bars_readable() {
    let source = "Sleigh Ride - Tori Kelly\n#B 86bpm 4/4\n\nVS 8\n\
         Bmaj7 / G#m7 / C#m9 / F#7 / Bmaj7\n\
         Bmaj7 / G#m7 / C#m9 / F#7 / Bmaj7 // Gmaj7 / F#7 /\n\
         Bmaj7 / G#m7 / C#m9 / F#7 / Bmaj7\n\
         Bmaj7 / G#m7 / C#m9 / F#7 / Bmaj7 ////\n";
    assert_four_a_line_and_sparse_bars_readable(source);
    // The whole-bar Bmaj7 (bar 2) keeps even beats.
    let lines = lines_of(source);
    let xs = &lines[0][1].1;
    let steps: Vec<f64> = xs.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(
        steps.iter().all(|s| (s - steps[0]).abs() < 1.0),
        "uneven beats in a held bar: {steps:?}"
    );
}

/// The largest chord-symbol font size on the first line of `source`.
fn first_line_symbol_size(source: &str) -> f64 {
    fn sizes(node: &SceneNode, in_chord: bool, out: &mut Vec<(f64, usize)>) {
        let chord =
            in_chord || node.metadata.get("element_type").map(String::as_str) == Some("chord");
        if chord {
            let system = node
                .metadata
                .get("system")
                .and_then(|s| s.parse().ok())
                .unwrap_or(usize::MAX);
            for command in &node.commands {
                if let PaintCommand::Text { font_size, .. } = command {
                    out.push((*font_size, system));
                }
            }
        }
        for child in &node.children {
            sizes(child, chord, out);
        }
    }
    let chart = keyflow_lang::text::chart::parse_chart(source).expect("parses");
    let result = ChartPipeline::shared()
        .expect("the pipeline builds")
        .layout_preset(&chart, Preset::Page, PresetOptions::for_export());
    let mut out = Vec::new();
    sizes(&result.scene, false, &mut out);
    let first = out.iter().map(|(_, s)| *s).min().expect("symbols");
    out.iter()
        .filter(|(_, s)| *s == first)
        .map(|(f, _)| *f)
        .fold(0.0, f64::max)
}

#[test]
fn four_a_line_wins_and_a_crowded_line_draws_smaller_symbols() {
    // Three bars of four symbols around one whole bar. At full size the
    // three clusters alone overflow the line, which used to break it two
    // and two; the line keeps its four bars and draws its symbols smaller.
    let dense = "Bmaj7 / G#m7 / C#m9 / F#7(#9) /";
    let source = format!("Dense\n#B 86bpm 4/4\n\nVS 4\n{dense}\nBmaj7\n{dense}\n{dense}\n");
    let lines = lines_of(&source);
    assert_eq!(lines.len(), 1, "one line of four bars");
    assert_eq!(lines[0].len(), 4);

    let roomy = "Roomy\n#B 86bpm 4/4\n\nVS 4\nBmaj7\nG#m7\nC#m9\nF#7\n";
    let full = first_line_symbol_size(roomy);
    let crowded = first_line_symbol_size(&source);
    assert!(
        crowded < full - 0.1 && crowded >= full * 0.85 - 0.01,
        "crowded line symbols {crowded} vs full {full}"
    );
}

/// Every glyph `glyph` in the scene with its colour.
fn glyph_colors(node: &SceneNode, glyph: char, out: &mut Vec<peniko::Color>) {
    for command in &node.commands {
        if let PaintCommand::Glyph {
            codepoint, color, ..
        } = command
            && *codepoint == glyph
        {
            out.push(*color);
        }
    }
    for child in &node.children {
        glyph_colors(child, glyph, out);
    }
}

#[test]
fn every_accent_draws_in_black_even_on_a_chord_named_once() {
    // All I Want for Christmas: `>Am7b5/D_4 >Am7b5/D_4 r2` names the chord
    // once and accents both attacks.
    let result = render("AIW\n#G 150bpm 4/4\n\nOUT 1\n>Am7b5/D_4 >Am7b5/D_4 r2\n");
    let mut accents = Vec::new();
    glyph_colors(&result.scene, '\u{E4A0}', &mut accents);
    assert_eq!(accents.len(), 2, "both attacks are accented");
    assert!(
        accents.iter().all(|c| *c == peniko::Color::BLACK),
        "accents are black: {accents:?}"
    );
}

/// Stroke widths of every vertical line in the scene, left to right.
fn vertical_line_widths(node: &SceneNode, parent: Affine, out: &mut Vec<(f64, f64)>) {
    let t = parent * node.transform;
    for command in &node.commands {
        if let PaintCommand::Line {
            start, end, width, ..
        } = command
        {
            let (a, b) = (t * *start, t * *end);
            if (a.x - b.x).abs() < 0.01 && (a.y - b.y).abs() > 10.0 {
                out.push((a.x, *width));
            }
        }
    }
    for child in &node.children {
        vertical_line_widths(child, t, out);
    }
}

#[test]
fn the_last_bar_closes_with_a_final_barline() {
    let result = render("End\n#G 120bpm 4/4\n\nVS 2\nG\nC\n\nCH 2\nD\nG\n");
    let mut lines = Vec::new();
    vertical_line_widths(&result.scene, Affine::IDENTITY, &mut lines);
    let thin = lines.iter().map(|(_, w)| *w).fold(f64::INFINITY, f64::min);
    let thick: Vec<_> = lines.iter().filter(|(_, w)| *w > thin * 1.5).collect();
    assert_eq!(
        thick.len(),
        1,
        "one thick line, the final barline: {lines:?}"
    );
}

/// A slash chord with an eighth before the next symbol stacks into a
/// vertical fraction: the bass drops below the root's baseline and the slash
/// becomes a stroke. One with a beat or more stays inline.
#[test]
fn a_crowded_slash_chord_stacks_and_a_roomy_one_does_not() {
    let chart = keyflow_lang::text::chart::parse_chart(
        "T - X\n#E 116bpm 4/4\n\nVS 2\nA/E_4. A/E_8 B/F#_8 C/G_4.\nD/F# // G/B //\n",
    )
    .expect("parses");
    let result = ChartPipeline::shared()
        .expect("the pipeline builds")
        .layout_preset(&chart, Preset::Page, PresetOptions::for_export());

    fn find<'a>(node: &'a SceneNode, root: &str, bass: &str) -> Option<&'a SceneNode> {
        if node.metadata.get("element_type").map(String::as_str) == Some("chord") {
            let texts: Vec<&str> = node
                .commands
                .iter()
                .filter_map(|c| match c {
                    PaintCommand::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            if texts.first() == Some(&root) && texts.iter().any(|t| t.starts_with(bass)) {
                return Some(node);
            }
        }
        node.children.iter().find_map(|c| find(c, root, bass))
    }
    let baselines = |node: &SceneNode| -> Vec<f64> {
        node.commands
            .iter()
            .filter_map(|c| match c {
                PaintCommand::Text { position, .. } => Some(position.y),
                _ => None,
            })
            .collect()
    };
    let has_stroke = |node: &SceneNode| {
        node.commands
            .iter()
            .any(|c| matches!(c, PaintCommand::Line { .. }))
    };

    let crowded = find(&result.scene, "B", "F").expect("the B/F# symbol");
    let ys = baselines(crowded);
    assert!(has_stroke(crowded), "B/F# on an eighth stacks: {crowded:?}");
    assert!(
        ys.last().copied().unwrap_or(0.0) > ys[0] + 1.0,
        "its bass sits below the root: {ys:?}"
    );

    let roomy = find(&result.scene, "D", "F").expect("the D/F# symbol");
    assert!(!has_stroke(roomy), "D/F# on two beats stays inline");
    let ys = baselines(roomy);
    assert!(
        ys.iter().all(|y| (y - ys[0]).abs() < 1e-6),
        "one baseline: {ys:?}"
    );
}
