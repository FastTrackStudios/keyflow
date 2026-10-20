//! `kf pdf-score`: read an engraved PDF back into a score and engrave it
//! again at any page size — a Letter page, or a phone screen.

use std::path::{Path, PathBuf};

use engraver_score::layout::{LayoutOptions, layout_part};
use keyflow::engraver::fonts::ChartFontBundle;

/// Output format for `kf pdf-score`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    Svg,
    Png,
    Pdf,
}

pub struct Args<'a> {
    pub input: &'a Path,
    pub out_dir: Option<&'a Path>,
    pub format: Format,
    /// Page size in points.
    pub width: f64,
    pub height: f64,
    /// Staff space in points.
    pub spatium: f64,
    pub margin: f64,
    pub scale: f32,
    /// Also write the chart as keyflow text (`<stem>.kf`).
    pub kf: bool,
    pub artist: Option<&'a str>,
}

pub fn cmd_pdf_score(args: &Args<'_>) -> Result<(), String> {
    let bytes = std::fs::read(args.input).map_err(|e| format!("read {}: {e}", args.input.display()))?;
    let pages = keyflow_pdfscore::read_pdf(&bytes).map_err(|e| e.to_string())?;
    let rec = keyflow_pdfscore::recognize(&pages);
    let r = &rec.report;
    println!(
        "{}: {} system(s), {} staves, {} measures, {} notes, {} rests, repeats {:?}",
        args.input.display(),
        r.systems,
        r.staves,
        r.measures,
        r.notes,
        r.rests,
        r.repeats
    );
    for w in &r.warnings {
        println!("  ! {w}");
    }
    if rec.score.parts.is_empty() || rec.score.parts[0].measures.is_empty() {
        return Err("no music recognised".into());
    }

    let stem = args
        .input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "score".into());
    let out_dir: PathBuf = args
        .out_dir
        .map_or_else(|| PathBuf::from(&stem), Path::to_path_buf);
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;

    if args.kf {
        let path = out_dir.join(format!("{stem}.kf"));
        std::fs::write(&path, keyflow_pdfscore::to_keyflow(&rec.score, args.artist))
            .map_err(|e| e.to_string())?;
        println!("{}", path.display());
    }

    let options = LayoutOptions {
        page_width: args.width,
        page_height: args.height,
        spatium: args.spatium,
        margin: args.margin,
        // Spacing scales with the staff size, so a phone page keeps the
        // proportions of a printed one.
        system_gap: 74.0 * args.spatium / 6.5,
        title_block_height: 84.0 * args.spatium / 6.5,
        ..LayoutOptions::default()
    };
    let layout = layout_part(&rec.score, 0, &options);
    let fonts = ChartFontBundle::shared()?;
    let base = out_dir.join(format!("{stem} {:.0}x{:.0}", args.width, args.height));
    match args.format {
        Format::Pdf => {
            let pdf = engraver_score::render::pdf(&layout, fonts)?;
            let path = PathBuf::from(format!("{}.pdf", base.display()));
            std::fs::write(&path, pdf).map_err(|e| e.to_string())?;
            println!("{} ({} page(s))", path.display(), layout.pages.len());
        }
        Format::Svg | Format::Png => {
            for (i, svg) in engraver_score::render::svg_pages(&layout, fonts).iter().enumerate() {
                let p = if args.format == Format::Svg {
                    let p = PathBuf::from(format!("{}.p{}.svg", base.display(), i + 1));
                    std::fs::write(&p, svg).map_err(|e| e.to_string())?;
                    p
                } else {
                    let p = PathBuf::from(format!("{}.p{}.png", base.display(), i + 1));
                    std::fs::write(&p, crate::parts::svg_to_png(svg, args.scale)?)
                        .map_err(|e| e.to_string())?;
                    p
                };
                println!("{}", p.display());
            }
        }
    }
    Ok(())
}
