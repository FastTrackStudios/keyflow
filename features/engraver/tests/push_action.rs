//! Push and Pull on a selection (`keyflow_lang::edit::select`), engraved:
//! the chord's push field set by the action draws as a push typed by hand
//! does (the early arrival tied into its beat), and a pull back draws the
//! chart as it was.

#![cfg(feature = "svg")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use engraver::api::pipeline::{ChartPipeline, Preset, PresetOptions};
use engraver::layout::chart::select::{Kind, selectables};
use keyflow_lang::edit::select::{Action, Pick, Step, apply};

const SONG: &str = "Push - Test\n#C 120bpm 4/4\n\nVS 4\nC // G //\nAm // F //\nC // G //\nDm ////\n\nCH 4\nF G Am G\n";

/// What the chart's page draws (every element's kind and box, and every
/// beat's place), and how many ties are among it.
fn engraved(source: &str) -> (String, usize) {
    use std::fmt::Write as _;
    let chart = keyflow_lang::text::chart::parse_chart(source).expect("parses");
    let pipeline = ChartPipeline::shared().expect("pipeline");
    let (mode, config) =
        ChartPipeline::resolve_preset(Preset::Page, PresetOptions::for_screen(612.0, 1.0));
    let layout = pipeline.layout_with_config(&chart, &mode, &config);
    let items = selectables(&layout);
    let ties = items.iter().filter(|s| s.kind == Kind::Tie).count();
    let mut drawn = String::new();
    for s in &items {
        let b = s.bounds;
        let _ = writeln!(
            drawn,
            "{} {:.2} {:.2} {:.2} {:.2}",
            s.key, b.x0, b.y0, b.x1, b.y1
        );
    }
    for b in &layout.beat_positions {
        let _ = writeln!(
            drawn,
            "beat {} {} {:.2} {:.2}",
            b.measure, b.tick, b.x, b.width
        );
    }
    (drawn, ties)
}

fn act(source: &str, picks: &[Pick], action: Action) -> String {
    match apply(source, picks, &action) {
        Ok(done) => done.text,
        Err(why) => panic!("not applied: {why}"),
    }
}

#[test]
fn a_pushed_chord_is_drawn_tied_into_its_beat_and_a_pull_draws_it_back() {
    let (plain, no_ties) = engraved(SONG);
    let at = [Pick::At { bar: 0, beat: 3.0 }];
    let pushed = act(SONG, &at, Action::Push(None));
    let (drawn, ties) = engraved(&pushed);
    assert!(ties > no_ties, "the push's tie is drawn");
    assert!(drawn != plain, "it draws differently");
    // The same picture as the push typed in syntax.
    let typed = SONG.replace("C // G //\nAm", "C // 'G //\nAm");
    assert_eq!(pushed, typed, "what is saved");
    assert!(drawn == engraved(&typed).0, "and what is drawn");
    // Pulled back: the chart as it was, to the byte.
    let back = act(&pushed, &at, Action::Pull(None));
    assert!(engraved(&back).0 == plain, "drawn as it was");
}

#[test]
fn a_sixteenth_draws_differently_from_an_eighth() {
    let at = [Pick::At { bar: 0, beat: 3.0 }];
    let eighth = engraved(&act(SONG, &at, Action::Push(Some(Step::Eighth)))).0;
    let sixteenth = engraved(&act(SONG, &at, Action::Push(Some(Step::Sixteenth)))).0;
    assert!(eighth != sixteenth);
}

#[test]
fn a_push_over_a_barline_and_over_a_range_still_engraves_every_bar() {
    let bars = |source: &str| {
        let chart = keyflow_lang::text::chart::parse_chart(source).expect("parses");
        keyflow_lang::edit::bars(&chart).len()
    };
    // Bar 4's only chord: it arrives in bar 3.
    let over = act(SONG, &[Pick::At { bar: 3, beat: 1.0 }], Action::Push(None));
    assert_eq!(bars(&over), bars(SONG));
    let (_, ties) = engraved(&over);
    assert!(ties > 0, "tied over the barline");
    // Bars 2 to 8, over the section's first bar too.
    let range = act(SONG, &[Pick::Bars { from: 1, to: 7 }], Action::Push(None));
    assert_eq!(bars(&range), bars(SONG));
    assert!(engraved(&range).1 >= ties);
}
