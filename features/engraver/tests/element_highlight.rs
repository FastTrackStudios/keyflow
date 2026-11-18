//! An element highlighted as itself: every selectable names the scene node
//! that draws it, and `ChartView::record_elements` draws those nodes again
//! in one colour, the way a hover or a selection recolours a chord symbol,
//! a note or a rest instead of laying a box over it.

#![cfg(feature = "paint")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyrender::Paint;
use anyrender::recording::RenderCommand;
use engraver::layout::chart::select::{Kind, kind_of, node_at};
use engraver::renderer::{ChartTheme, ChartView};
use peniko::Color;

const CHART: &str = "\
Title: Highlight
Tempo: 120
Time: 4/4

Verse
| C / / / | Am7 / / / | F / G / | C / / / |
";

fn laid_out() -> (ChartView, keyflow::Chart) {
    let chart = keyflow::parse(CHART).expect("the chart parses");
    let mut view = ChartView::new().unwrap();
    view.set_theme(ChartTheme::default());
    let mut scene = anyrender::Scene::new();
    let _ = view.paint(&mut scene, &chart, 1, 900.0, 1.0, 1.0, (0.0, 0.0), None);
    (view, chart)
}

#[test]
fn every_selectable_but_a_bar_names_the_node_that_draws_it() {
    let (view, _) = laid_out();
    let items = view.selectables();
    assert!(items.iter().any(|s| s.kind == Kind::Chord), "chords to select");
    let root = view.layout_scene().expect("a layout");
    for item in &items {
        match (&item.node, item.kind) {
            (None, Kind::Bar) => {}
            (None, kind) => panic!("{}: a {kind:?} with no node", item.key),
            (Some(path), kind) => {
                let (node, _) = node_at(root, path)
                    .unwrap_or_else(|| panic!("{}: no node at {path:?}", item.key));
                assert_eq!(kind_of(node), Some(kind), "{}: another node", item.key);
            }
        }
    }
}

#[test]
fn an_element_is_drawn_again_in_one_colour_and_nothing_else() {
    let (view, _) = laid_out();
    let items = view.selectables();
    let chord = items.iter().find(|s| s.kind == Kind::Chord).unwrap();
    let path = chord.node.as_deref().unwrap();
    let blue = Color::from_rgb8(0, 101, 191);
    let commands = view.record_elements(&[path], blue).expect("a layout");
    assert!(!commands.is_empty(), "the chord drew nothing");
    for command in &commands {
        let brush = match command {
            RenderCommand::Fill(f) => Some(&f.brush),
            RenderCommand::Stroke(s) => Some(&s.brush),
            RenderCommand::GlyphRun(g) => Some(&g.brush),
            _ => None,
        };
        if let Some(Paint::Solid(c)) = brush {
            let alpha = c.components[3];
            assert!(
                alpha == 0.0 || c.with_alpha(1.0) == blue,
                "a command in {c:?}, not the highlight's colour"
            );
        }
    }
    assert!(
        view.record_elements(&[], blue).unwrap().is_empty(),
        "nothing to highlight draws nothing"
    );
}
