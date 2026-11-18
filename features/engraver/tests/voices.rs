//! Voices: the lanes of a `<< chords m{ … } ; m{ … } >>` bar sound
//! together, so they are drawn together in that bar (voice 1's stems up,
//! voice 2's down), not one after the other, spilling into the next bar.

#![cfg(feature = "paint")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use engraver::layout::chart::select::Kind;
use engraver::renderer::{ChartTheme, ChartView};
use engraver::scene::{ElementType, SceneNode};

const CHART: &str = "\
Title: Voices
Tempo: 120
Time: 4/4

Verse
<< C m{ E:52 G:52 } ; m{ C:44 C:44 C:44 C:44 } >> | G / / / |
";

#[test]
fn two_voices_share_their_bar() {
    let chart = keyflow::parse(CHART).expect("the chart parses");
    let bars: Vec<_> = chart
        .sections
        .iter()
        .flat_map(|s| s.measures().iter())
        .collect();
    assert_eq!(bars.len(), 2, "the second voice adds no bar");
    let voices: Vec<u8> = bars[0].melodies.iter().map(|m| m.voice).collect();
    assert_eq!(voices, [0, 1], "each lane its own voice");

    let mut view = ChartView::new().unwrap();
    view.set_theme(ChartTheme::default());
    let mut scene = anyrender::Scene::new();
    let _ = view.paint(&mut scene, &chart, 1, 900.0, 1.0, 1.0, (0.0, 0.0), None);
    let items = view.selectables();
    let bar = |key: &str| {
        items
            .iter()
            .find(|s| s.key.starts_with(key))
            .unwrap_or_else(|| panic!("no {key}"))
            .bounds
    };
    let (first, second) = (bar("bar#0."), bar("bar#1."));
    let notes_in = |b: kurbo::Rect| {
        items
            .iter()
            .filter(|s| s.kind == Kind::Note)
            .filter(|s| {
                let x = s.bounds.center().x;
                x > b.x0 && x < b.x1
            })
            .map(|s| s.bounds)
            .collect::<Vec<_>>()
    };
    // Voice 1 on beats 1 and 3, voice 2 on every beat, at the same places.
    let mut xs: Vec<i64> = notes_in(first)
        .iter()
        .map(|r| r.x0.round() as i64)
        .collect();
    xs.sort_unstable();
    xs.dedup();
    assert_eq!(xs.len(), 4, "four beats, both voices on the same four");
    let on_beat_one: Vec<_> = notes_in(first)
        .into_iter()
        .filter(|r| r.x0.round() as i64 == xs[0])
        .collect();
    let staff_middle = first.y0 - 10.0;
    assert!(
        on_beat_one.iter().any(|r| r.y0 < staff_middle - 10.0),
        "voice 1's stem goes up"
    );
    assert!(
        on_beat_one.iter().any(|r| r.y1 > first.y0 + 5.0),
        "voice 2's stem goes down"
    );
    assert!(
        notes_in(second).iter().all(|r| r.height() < 10.0),
        "the second bar keeps its own slashes: nothing spilled into it"
    );
}

fn count(node: &SceneNode, kind: ElementType) -> usize {
    usize::from(node.id.as_ref().is_some_and(|id| id.element_type == kind))
        + node.children.iter().map(|c| count(c, kind)).sum::<usize>()
}

#[test]
fn a_melody_s_triplets_are_drawn_as_a_triplet() {
    let chart = keyflow::parse(
        "Title: T\nTime: 4/4\n\nVerse\n<< C ; m{ C:48t D:48t E:48t F:44 G:42 } >> | G / / / |\n",
    )
    .expect("the chart parses");
    let mut view = ChartView::new().unwrap();
    view.set_theme(ChartTheme::default());
    let mut scene = anyrender::Scene::new();
    let _ = view.paint(&mut scene, &chart, 1, 900.0, 1.0, 1.0, (0.0, 0.0), None);
    let root = view.layout_scene().expect("a layout");
    assert_eq!(count(root, ElementType::Tuplet), 1, "one `3` bracket");
}
