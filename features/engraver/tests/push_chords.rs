//! A chord pushed inside a bar (`Dsus2 // 'D //`) is played an eighth before
//! its written beat. The rhythm builder draws that anticipation; these tests
//! pin that the chord symbol prints over it and that the anticipation is tied
//! into the beat it was pushed from, so the beat does not read as a second
//! attack.

use engraver::api::pipeline::{ChartPipeline, Preset, PresetOptions};
use engraver::engraver::layout::chart::ChartLayoutResult;
use engraver::engraver::scene::node::SceneNode;
use engraver::engraver::scene::paint::PaintCommand;
use kurbo::Affine;

fn render(source: &str) -> ChartLayoutResult {
    let chart = keyflow_lang::text::chart::parse_chart(source).expect("the chart parses");
    ChartPipeline::shared()
        .expect("the pipeline builds")
        .layout_preset(&chart, Preset::Page, PresetOptions::for_export())
}

/// Every chord symbol as `(text, world x of its first glyph run)`.
fn symbols(scene: &SceneNode) -> Vec<(String, f64)> {
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
    fn walk(node: &SceneNode, parent: Affine, out: &mut Vec<(String, f64)>) {
        let t = parent * node.transform;
        if node.metadata.get("element_type").map(String::as_str) == Some("chord") {
            let mut runs = Vec::new();
            texts(node, parent, &mut runs);
            if let Some(x) = runs.iter().map(|(_, x)| *x).reduce(f64::min) {
                out.push((runs.iter().map(|(s, _)| s.as_str()).collect(), x));
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

fn count(node: &SceneNode, element_type: &str) -> usize {
    usize::from(node.metadata.get("element_type").map(String::as_str) == Some(element_type))
        + node
            .children
            .iter()
            .map(|c| count(c, element_type))
            .sum::<usize>()
}

/// A wide symbol (`Dsus2`) before the push used to leave too little room
/// for the pushed one, which then printed on its written beat (3) instead
/// of over its anticipation (the and of 2).
#[test]
fn a_pushed_chord_prints_over_its_anticipation() {
    let result = render("T - X\n#Bm 112bpm 4/4\n\nVS 2\nDsus2 // 'D // Bm\n");
    let (_, d_x) = symbols(&result.scene)
        .into_iter()
        .find(|(text, _)| text == "D")
        .expect("the pushed D prints");
    let bar = |tick: i32| {
        result
            .beat_positions
            .iter()
            .find(|b| b.measure == 0 && b.tick == tick)
            .unwrap_or_else(|| panic!("a segment at tick {tick}: {:?}", result.beat_positions))
            .x
    };
    let anticipation = bar(720); // the and of 2
    assert!(
        (d_x - anticipation).abs() < 1.0,
        "D sits {:.1}pt off its anticipation at the and of 2 (beat 3 is at {:.1})",
        d_x - anticipation,
        bar(960) - anticipation
    );
}

#[test]
fn a_push_that_keeps_sounding_is_tied_into_its_beat() {
    // D is pushed from beat 3 and held through beat 4: one tie.
    let held = render("T - X\n#Bm 112bpm 4/4\n\nVS 1\nDsus2 // 'D //\n");
    assert_eq!(count(&held.scene, "push_tie"), 1);

    // G lasts one beat: the anticipation is all of it, and the note after
    // it is Am's, so nothing is tied.
    let short = render("T - X\n#Bm 112bpm 4/4\n\nVS 1\nC // 'G_4 Am_4\n");
    assert_eq!(count(&short.scene, "push_tie"), 0);

    // No push, no tie.
    let plain = render("T - X\n#Bm 112bpm 4/4\n\nVS 1\nDsus2 // D //\n");
    assert_eq!(count(&plain.scene, "push_tie"), 0);
}
