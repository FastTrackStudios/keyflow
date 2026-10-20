#![cfg(feature = "pdfscore")]
//! Print strokes/fills with their segment geometry, for calibrating the
//! recogniser. Usage: `cargo run -p keyflow-pdfscore --example geom -- <in.pdf>`
use kurbo::Shape;

fn main() {
    let input = std::env::args().nth(1).expect("usage: geom <in.pdf>");
    let pages = engraver::pdfscore::read_pdf(&std::fs::read(&input).expect("read")).expect("pdf");
    for p in pages {
        for s in &p.strokes {
            let b = s.path.bounding_box();
            let seg = engraver::pdfscore::extract::as_segment(&s.path).is_some();
            println!(
                "S w={:.2} seg={} x={:.2}..{:.2} y={:.2}..{:.2} els={}",
                s.width,
                seg,
                b.x0,
                b.x1,
                b.y0,
                b.y1,
                s.path.elements().len()
            );
        }
        for f in &p.fills {
            let b = f.path.bounding_box();
            println!(
                "F x={:.2}..{:.2} y={:.2}..{:.2} els={} curves={}",
                b.x0,
                b.x1,
                b.y0,
                b.y1,
                f.path.elements().len(),
                f.path
                    .elements()
                    .iter()
                    .filter(|e| matches!(e, kurbo::PathEl::CurveTo(..) | kurbo::PathEl::QuadTo(..)))
                    .count()
            );
        }
    }
}
