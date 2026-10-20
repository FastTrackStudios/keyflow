#![cfg(feature = "pdfscore")]
//! Dump what `keyflow-pdfscore` extracts from a PDF, and redraw it as SVG
//! to compare against the original.
//!
//! Usage: `cargo run -p keyflow-pdfscore --example dump -- <in.pdf> [out.svg]`

use std::collections::BTreeMap;
use std::fmt::Write as _;

use kurbo::Shape as _;

fn main() {
    let mut args = std::env::args().skip(1);
    let input = args.next().expect("usage: dump <in.pdf> [out.svg]");
    let svg_out = args.next();
    let bytes = std::fs::read(&input).expect("read input");
    let pages = engraver::pdfscore::read_pdf(&bytes).expect("read pdf");
    for page in &pages {
        println!(
            "page {} ({:.0}x{:.0}): {} glyphs, {} strokes, {} fills, {} images",
            page.number,
            page.width,
            page.height,
            page.glyphs.len(),
            page.strokes.len(),
            page.fills.len(),
            page.images.len()
        );
        let mut by_font: BTreeMap<&str, BTreeMap<char, usize>> = BTreeMap::new();
        for g in &page.glyphs {
            *by_font.entry(&g.font).or_default().entry(g.ch).or_default() += 1;
        }
        for (font, chars) in &by_font {
            let list: Vec<String> = chars
                .iter()
                .map(|(c, n)| format!("{c}(U+{:04X})×{n}", *c as u32))
                .collect();
            println!("  {font}: {}", list.join(" "));
        }
        for img in &page.images {
            println!("  image {} at {:?}", img.name, img.bounds);
        }
    }
    if let (Some(out), Some(page)) = (svg_out, pages.first()) {
        let mut s = String::new();
        let _ = writeln!(
            s,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><rect width="100%" height="100%" fill="white"/>"#,
            w = page.width,
            h = page.height
        );
        for st in &page.strokes {
            let _ = writeln!(
                s,
                r#"<path d="{}" fill="none" stroke="{}" stroke-width="{:.2}"/>"#,
                st.path.to_svg(),
                if st.dashed { "orange" } else { "black" },
                st.width.max(0.2)
            );
        }
        for f in &page.fills {
            let _ = writeln!(
                s,
                r#"<path d="{}" fill="steelblue" opacity="0.7"/>"#,
                f.path.to_svg()
            );
        }
        for g in &page.glyphs {
            let esc = match g.ch {
                '<' => "&lt;".to_string(),
                '>' => "&gt;".to_string(),
                '&' => "&amp;".to_string(),
                c => c.to_string(),
            };
            let color = if g.font.contains("Text")
                || g.font.contains("Palatino")
                || g.font.contains("Chords")
            {
                "crimson"
            } else {
                "green"
            };
            let _ = writeln!(
                s,
                r#"<circle cx="{:.2}" cy="{:.2}" r="0.8" fill="{color}"/><text x="{:.2}" y="{:.2}" font-size="{:.1}" fill="{color}" font-family="monospace">{esc}</text>"#,
                g.origin.x,
                g.origin.y,
                g.origin.x,
                g.origin.y,
                (g.size * 0.25).clamp(3.0, 14.0)
            );
        }
        for img in &page.images {
            let r = img.bounds;
            let _ = writeln!(
                s,
                r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="none" stroke="magenta" stroke-width="1"/>"#,
                r.x0,
                r.y0,
                r.width(),
                r.height()
            );
        }
        s.push_str("</svg>\n");
        std::fs::write(&out, s).expect("write svg");
        let _ = page.strokes.first().map(|st| st.path.bounding_box());
        println!("wrote {out}");
    }
}
