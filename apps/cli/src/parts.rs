//! `kf parts` — extract every instrument part from a score as print-ready
//! pages.
//!
//! A Finale `.musx` is engraved with the layout its arranger gave each
//! linked part (system breaks, page turns, multirests, headers, measure
//! numbers); MusicXML input gets the engine's own layout and a default
//! title block.

use std::path::{Path, PathBuf};

use engraver_score::finale::{FinaleDocument, PartJob, PrintInfo};
use engraver_score::layout::{LayoutOptions, layout_part};
use engraver_score::model::Score;
use keyflow::engraver::fonts::ChartFontBundle;

/// Output format for `kf parts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum PartsFormat {
    Pdf,
    Svg,
    Png,
}

pub struct PartsArgs<'a> {
    pub input: &'a Path,
    pub out_dir: Option<&'a Path>,
    pub part: Option<&'a str>,
    pub format: PartsFormat,
    pub date: Option<&'a str>,
    pub list: bool,
    /// PNG pixels per point.
    pub scale: f32,
    /// Ignore the source's system/page layout and pack systems
    /// automatically (keeps its page size, headers and multirests).
    pub auto_layout: bool,
    /// Report overlapping ink instead of writing files.
    pub check: bool,
}

/// `--layout auto`: drop the source's system and page breaks (an unformatted
/// Finale part stores its default 2-bars-a-system layout), but keep its
/// page format, page text and measure-number style. Multirests come from
/// the engine, which groups rest runs and breaks them at cues.
pub(crate) fn auto_layout(options: &mut LayoutOptions) {
    options.explicit = None;
}

/// A loaded score and the parts to engrave from it.
struct Source {
    title: String,
    score: Score,
    jobs: Vec<PartJob>,
}

fn load(input: &Path, print: &PrintInfo) -> Result<Source, String> {
    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if ext == "musx" {
        let bytes = std::fs::read(input).map_err(|e| format!("read {}: {e}", input.display()))?;
        let doc = FinaleDocument::from_musx(&bytes).map_err(|e| e.to_string())?;
        let jobs = doc.jobs(print);
        let title = doc.title().unwrap_or(stem);
        return Ok(Source {
            title,
            score: doc.score,
            jobs,
        });
    }
    let score = engraver_score::import_file(input).map_err(|e| e.to_string())?;
    let jobs = score
        .parts
        .iter()
        .enumerate()
        .map(|(i, p)| PartJob {
            name: p.name.clone(),
            part_index: i,
            options: LayoutOptions::default(),
            staves: usize::from(p.staves),
        })
        .collect();
    let title = score.work_title.clone().unwrap_or(stem);
    Ok(Source { title, score, jobs })
}

/// `M/D/YYYY` of a file's modification time (what a "Rev." stamp shows).
pub(crate) fn file_date(path: &Path) -> String {
    let Ok(modified) = std::fs::metadata(path).and_then(|m| m.modified()) else {
        return String::new();
    };
    let Ok(since) = modified.duration_since(std::time::UNIX_EPOCH) else {
        return String::new();
    };
    let (y, m, d) = civil_from_days((since.as_secs() / 86_400) as i64);
    format!("{m}/{d}/{y}")
}

/// Days since 1970-01-01 → (year, month, day), proleptic Gregorian
/// (Howard Hinnant's `civil_from_days`).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// A name safe on every filesystem the parts get copied to.
pub(crate) fn file_safe(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn svg_to_png(svg: &str, scale: f32) -> Result<Vec<u8>, String> {
    let fonts = ChartFontBundle::shared()?;
    let mut fontdb = usvg::fontdb::Database::new();
    fontdb.load_font_data(fonts.symbol_font_data().as_ref().clone());
    fontdb.load_font_data(fonts.bravura_font_data().as_ref().clone());
    fontdb.load_font_data(fonts.leland_text_font_data().as_ref().clone());
    fontdb.load_font_data(fonts.freesans_font_data().as_ref().clone());
    fontdb.load_font_data(fonts.chicago_font_data().as_ref().clone());
    for (_, face) in engraver_score::fonts::serif_faces() {
        fontdb.load_font_data(face.to_vec());
    }
    let mut options = usvg::Options::default();
    *options.fontdb_mut() = fontdb;
    let tree = usvg::Tree::from_str(svg, &options).map_err(|e| e.to_string())?;
    let size = tree
        .size()
        .to_int_size()
        .scale_by(scale)
        .ok_or("bad size")?;
    let mut pixmap =
        tiny_skia::Pixmap::new(size.width(), size.height()).ok_or("pixmap alloc failed")?;
    pixmap.fill(tiny_skia::Color::WHITE);
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(|e| e.to_string())
}

pub fn cmd_parts(args: &PartsArgs<'_>) -> Result<(), String> {
    let print = PrintInfo {
        date: args
            .date
            .map(str::to_string)
            .unwrap_or_else(|| file_date(args.input)),
        file_name: args
            .input
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
    };
    let source = load(args.input, &print)?;
    let wanted = args.part.map(str::to_lowercase);
    let jobs: Vec<&PartJob> = source
        .jobs
        .iter()
        .filter(|j| {
            wanted
                .as_deref()
                .is_none_or(|w| j.name.to_lowercase().contains(w))
        })
        .collect();
    if jobs.is_empty() {
        let names: Vec<&str> = source.jobs.iter().map(|j| j.name.as_str()).collect();
        return Err(format!("no part matches; parts: {}", names.join(", ")));
    }

    if args.list {
        println!("{} — {} part(s)", source.title, jobs.len());
        for j in &jobs {
            let (systems, pages, how) = match &j.options.explicit {
                Some(e) => (
                    e.systems.len(),
                    e.systems.iter().map(|s| s.page + 1).max().unwrap_or(0),
                    "finale layout",
                ),
                None => (0, 0, "auto layout"),
            };
            println!(
                "  {:<28} staves={} systems={:<3} pages={} ({how})",
                j.name, j.staves, systems, pages
            );
        }
        return Ok(());
    }

    let out_dir: PathBuf = args
        .out_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(file_safe(&source.title)));
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let fonts = ChartFontBundle::shared()?;

    for job in jobs {
        let mut options = job.options.clone();
        if args.auto_layout {
            auto_layout(&mut options);
        }
        let layout = layout_part(&source.score, job.part_index, &options);
        if args.check {
            let report = engraver_score::inspect::collisions(&layout, fonts);
            println!("{:<28} {:>4} collision(s)", job.name, report.total());
            for ((a, b), n) in &report.by_kind {
                println!("    {n:>4}  {a:?} × {b:?}");
            }
            if args.part.is_some() {
                for c in &report.collisions {
                    println!(
                        "      p{} ({:>5.0},{:>5.0})  {:?} {:?}  ×  {:?} {:?}",
                        c.page, c.at.x, c.at.y, c.a.0, c.a.1, c.b.0, c.b.1
                    );
                }
            }
            continue;
        }
        let base = out_dir.join(file_safe(&format!("{} - {}", source.title, job.name)));
        if job.staves > 1 {
            eprintln!(
                "  note: {} has {} staves; only the top staff is engraved yet",
                job.name, job.staves
            );
        }
        match args.format {
            PartsFormat::Pdf => {
                let pdf = engraver_score::render::pdf(&layout, fonts)?;
                // Not `with_extension`: a title like "SorcApp 6.2" would
                // lose everything after its dot.
                let path = PathBuf::from(format!("{}.pdf", base.display()));
                std::fs::write(&path, pdf).map_err(|e| e.to_string())?;
                println!("{} ({} page(s))", path.display(), layout.pages.len());
            }
            PartsFormat::Svg | PartsFormat::Png => {
                let svgs = engraver_score::render::svg_pages(&layout, fonts);
                for (i, svg) in svgs.iter().enumerate() {
                    let stem = format!("{}.p{}", base.display(), i + 1);
                    let path = if args.format == PartsFormat::Svg {
                        let p = PathBuf::from(format!("{stem}.svg"));
                        std::fs::write(&p, svg).map_err(|e| e.to_string())?;
                        p
                    } else {
                        let p = PathBuf::from(format!("{stem}.png"));
                        std::fs::write(&p, svg_to_png(svg, args.scale)?)
                            .map_err(|e| e.to_string())?;
                        p
                    };
                    println!("{}", path.display());
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(16_456), (2015, 1, 21));
        assert_eq!(civil_from_days(20_741), (2026, 10, 15));
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(
            file_safe("Don't Answer Me - Horn 1/2"),
            "Don't Answer Me - Horn 1-2"
        );
    }
}
