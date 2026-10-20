#![cfg(feature = "pdfscore")]
//! Print every glyph instance as `font<TAB>char<TAB>x<TAB>y<TAB>size` (page
//! points), for building font tables.
//!
//! Usage: `cargo run -p keyflow-pdfscore --example glyphs -- <in.pdf>`

fn main() {
    let input = std::env::args().nth(1).expect("usage: glyphs <in.pdf>");
    let pages = engraver::pdfscore::read_pdf(&std::fs::read(&input).expect("read")).expect("pdf");
    for p in pages {
        for g in p.glyphs {
            println!(
                "{}\t{}\tU+{:04X}\t{:.2}\t{:.2}\t{:.2}\t{}",
                g.font, g.ch, g.ch as u32, g.origin.x, g.origin.y, g.size, p.number
            );
        }
    }
}
