//! Cues below the staff: one clean face, no underline, laid out so a bar's
//! dynamic, text cues and instrument cues never sit on each other.
//!
//! The shape is All I Want for Christmas's opening bar — `dyn mp`, a
//! `@bells` cue and a `"Rubato"` text cue on one bar — which drew the `mp`
//! on top of "Rubato", the bells cue grey and underlined below-right, and
//! the text cue in the handwritten chord face (it asked for "Leland Text",
//! which has no letters, and fell back).

use engraver::api::pipeline::{ChartPipeline, Preset, PresetOptions};
use engraver::engraver::scene::node::SceneNode;
use engraver::engraver::scene::paint::PaintCommand;
use kurbo::{Affine, Rect};

const OPENING: &str = "AIW\n#G 150bpm 4/4\n\n[Intro] 2\ndyn mp\n@bells \"Bells only\"\n\"Rubato\" G // Em //\n\"Count 2 off - band in, swung 8ths, all together now\" C\n";

struct Item {
    kind: String,
    rect: Rect,
    commands: Vec<PaintCommand>,
}

/// Every dynamic, text cue and instrument cue, with an ink box estimated
/// the way the layout estimates it.
fn items(source: &str) -> Vec<Item> {
    fn walk(node: &SceneNode, parent: Affine, out: &mut Vec<Item>) {
        let t = parent * node.transform;
        let kind = node.metadata.get("element_type").cloned().unwrap_or_default();
        if matches!(kind.as_str(), "dynamic" | "staff_text" | "text_cue") {
            let rect = node
                .commands
                .iter()
                .filter_map(|c| match c {
                    PaintCommand::Text {
                        text,
                        font_size,
                        position,
                        ..
                    } => {
                        let p = t * *position;
                        let w = text.chars().count() as f64 * font_size * 0.48;
                        Some(Rect::new(p.x, p.y - font_size * 0.65, p.x + w, p.y + font_size * 0.1))
                    }
                    PaintCommand::Glyph { position, size, .. } => {
                        let p = t * *position;
                        Some(Rect::new(p.x, p.y - size * 0.5, p.x + size * 0.9, p.y))
                    }
                    _ => None,
                })
                .reduce(|a, b| a.union(b));
            if let Some(rect) = rect {
                out.push(Item {
                    kind,
                    rect,
                    commands: node.commands.clone(),
                });
            }
            return;
        }
        for child in &node.children {
            walk(child, t, out);
        }
    }
    let chart = keyflow_lang::text::chart::parse_chart(source).expect("the chart parses");
    let result = ChartPipeline::shared()
        .expect("the pipeline builds")
        .layout_preset(&chart, Preset::Page, PresetOptions::for_export());
    let mut out = Vec::new();
    walk(&result.scene, Affine::IDENTITY, &mut out);
    out
}

#[test]
fn a_bars_dynamic_and_cues_do_not_overlap() {
    let items = items(OPENING);
    let kinds: Vec<_> = items.iter().map(|i| i.kind.as_str()).collect();
    assert!(kinds.contains(&"dynamic"), "{kinds:?}");
    assert!(kinds.contains(&"staff_text"), "{kinds:?}");
    assert!(kinds.contains(&"text_cue"), "{kinds:?}");
    for (i, a) in items.iter().enumerate() {
        for b in &items[i + 1..] {
            let overlap = a.rect.intersect(b.rect);
            assert!(
                overlap.width() <= 0.5 || overlap.height() <= 0.5,
                "{} {:?} overlaps {} {:?}",
                a.kind,
                a.rect,
                b.kind,
                b.rect
            );
        }
    }
    // The dynamic comes first, at the bar start: nothing in its bar sits to
    // its left on its row.
    let dynamic = items.iter().find(|i| i.kind == "dynamic").expect("dynamic");
    for other in items.iter().filter(|i| i.kind != "dynamic") {
        if other.rect.y0 < dynamic.rect.y1 && other.rect.y1 > dynamic.rect.y0 {
            assert!(other.rect.x0 >= dynamic.rect.x0, "{} left of the dynamic", other.kind);
        }
    }
}

#[test]
fn cues_are_one_upright_bundled_face_without_underline_and_never_shrink() {
    let items = items(OPENING);
    let mut sizes = Vec::new();
    for item in items.iter().filter(|i| i.kind != "dynamic") {
        for command in &item.commands {
            match command {
                PaintCommand::Text {
                    font_family,
                    font_size,
                    ..
                } => {
                    assert_eq!(font_family, "FreeSans", "{}", item.kind);
                    sizes.push(*font_size);
                }
                PaintCommand::Line { .. } => panic!("{} is underlined", item.kind),
                _ => {}
            }
        }
    }
    assert!(sizes.len() >= 3);
    // The long cue on bar 2 is the same size as the short ones.
    assert!(
        sizes.iter().all(|s| (s - sizes[0]).abs() < 1e-6),
        "cue sizes differ: {sizes:?}"
    );
    assert!(
        items.iter().any(|i| i.commands.iter().any(|c| {
            matches!(c, PaintCommand::Text { text, .. } if text.starts_with("Count 2 off"))
        })),
        "the long cue draws"
    );
}

/// Joy to the World's outro: `"Ring out" E % % %` after a bar of written
/// values. The cue starts at its bar's first beat — it used to draw under
/// beat 2, because the beat anchor scaled the bar's segment positions (in
/// points) by the staff space a second time.
#[test]
fn a_beat_one_cue_starts_at_its_bars_first_beat() {
    let source = "Joy\n#D 128bpm 4/4\n\nOUT 10\n@guitar \"Guitar hook\"\nE /// A /\nA E /// A / A\nE_4. E_4. A_4 A\n\"Ring out\" E % % %\n";
    let chart = keyflow_lang::text::chart::parse_chart(source).expect("parses");
    let bar = chart.sections[0]
        .measures()
        .iter()
        .position(|m| m.staff_text.iter().any(|t| t.text == "Ring out"))
        .expect("the cue is on a bar");
    let result = ChartPipeline::shared()
        .expect("the pipeline builds")
        .layout_preset(&chart, Preset::Page, PresetOptions::for_export());
    let beats: Vec<_> = result.beat_positions.iter().filter(|b| b.measure == bar).collect();
    let bar_start = beats[0].measure_x0;
    let beat_two = beats.iter().find(|b| b.beat == 1 || b.beat == 2).map(|b| b.x);
    let second = beats.get(1).map(|b| b.x).or(beat_two).expect("a second beat");
    let cue = items(source)
        .into_iter()
        .find(|i| {
            i.commands.iter().any(
                |c| matches!(c, PaintCommand::Text { text, .. } if text == "Ring out"),
            )
        })
        .expect("the cue draws");
    assert!(
        cue.rect.x0 >= bar_start - 1.0 && cue.rect.x0 < second,
        "Ring out at {:.1}; bar starts {bar_start:.1}, beat 2 at {second:.1}",
        cue.rect.x0
    );
}
