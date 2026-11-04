//! Keyflow's editor shows its caret in a Blitz host: the plain caret's
//! element is in the DOM and painted, with the CSS a host such as
//! architect's lab gives it.
#![cfg(feature = "native")]

use dioxus::prelude::*;
use dioxus_test::render;
use editor_keyflow::view::{
    KeyflowEditor, PANE_CSS, editor_state::EditorState, editor_view::EDITOR_CSS,
};

const HOST: &str = ".kf-pane-head { display: none; } .editor-root { --editor-caret: #ff0000; } .editor-frame { --editor-caret: #ff0000; }";

#[component]
fn Harness() -> Element {
    // As architect's lab mounts it (apps/lab/ui/src/keyflow.rs): app-level
    // sheets, the dark highlight theme, its colour variables on a wrapper.
    use editor_keyflow::lang::HighlightTheme;
    let highlight = editor_keyflow::lang::highlight_css(&HighlightTheme::default_dark());
    let css = format!("{EDITOR_CSS}\n{PANE_CSS}\n{highlight}\n{HOST}");
    rsx! {
        style { dangerous_inner_html: css }
        div {
            style: "height:100%; min-height:0; min-width:0; --code-bg:#202024; --background:#202024; --foreground:#e9e9ee; --ink:#e9e9ee; --ink-soft:#9a9aa6; --accent:#3ec7a8;",
            KeyflowEditor {
                stylesheet: false,
                initial: "C F G C\n".to_string(),
                highlight_theme: HighlightTheme::default_dark(),
                on_change: move |_: String| {},
            }
        }
    }
}

#[allow(dead_code)]
fn red_pixels(path: &std::path::Path) -> usize {
    let file = std::fs::File::open(path).expect("open png");
    let mut reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .expect("png");
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("frame");
    buf[..info.buffer_size()]
        .chunks_exact(4)
        .filter(|p| p[0] > 180 && p[1] < 90 && p[2] < 90)
        .count()
}

async fn probe(t: &dioxus_test::DocumentTester, name: &str) -> (usize, usize, usize) {
    t.pump().await.ok();
    let carets = t.query_all(".ed-native-caret").immediately().len();
    let dir = std::env::temp_dir().join("kf_caret_probe");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.png"));
    t.render_png(&path);
    let (red, blue) = colours(&path);
    eprintln!("{name}: carets in DOM {carets}, red px {red}, blue px {blue}");
    (carets, red, blue)
}

fn colours(path: &std::path::Path) -> (usize, usize) {
    let file = std::fs::File::open(path).expect("open png");
    let mut reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .expect("png");
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("frame");
    let px = &buf[..info.buffer_size()];
    let red = px
        .chunks_exact(4)
        .filter(|p| p[0] > 180 && p[1] < 90 && p[2] < 90)
        .count();
    let blue = px
        .chunks_exact(4)
        .filter(|p| i32::from(p[2]) - i32::from(p[0]) > 60 && p[2] > p[1])
        .count();
    (red, blue)
}

#[tokio::test]
async fn the_plain_caret_is_in_the_dom_and_painted() {
    use dioxus_test::keyboard_types::{Key, Modifiers, NamedKey};
    let t = render(Harness).build();
    t.query(".editor-root").immediately().unwrap().focus();
    let start = probe(&t, "start").await;
    t.press_key(Key::Named(NamedKey::End), Modifiers::empty());
    let end = probe(&t, "line_end").await;
    t.type_text(" Am");
    let typed = probe(&t, "typed").await;
    for (name, (carets, red, _)) in [("start", start), ("line_end", end), ("typed", typed)] {
        assert!(
            carets >= 1,
            "{name}: no .ed-native-caret element in the DOM"
        );
        assert!(
            red >= 10,
            "{name}: the caret is not painted in the host's colour ({red} px)"
        );
    }
}
