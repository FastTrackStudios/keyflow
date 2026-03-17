use std::io::Read;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use keyflow::engraver::export::pdf::PdfSerializer;
use keyflow::engraver::export::svg::{SvgExportConfig, SvgSerializer};
use keyflow::engraver::fonts::ChartFontBundle;
use keyflow::engraver::layout::chart::{
    ChartLayoutConfig, ChartLayoutEngine, ChartLayoutResult, LayoutMode,
};
use keyflow::engraver::style::MStyle;
use keyflow::Chart;

/// A4 page dimensions in points.
const A4_WIDTH: f64 = 595.0;
const A4_HEIGHT: f64 = 842.0;

#[derive(Parser)]
#[command(name = "kf", about = "Keyflow chart CLI — parse, inspect, and render charts")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse chart text and show the full structure
    Parse {
        /// Path to a .kf file, or "-" for stdin
        #[arg(default_value = "-")]
        input: String,
    },
    /// Render chart to PDF
    Pdf {
        /// Path to a .kf file, or "-" for stdin
        #[arg(default_value = "-")]
        input: String,
        /// Output PDF path
        #[arg(short, long, default_value = "chart.pdf")]
        output: PathBuf,
    },
    /// Import MIDI and render the generated chart to PDF
    MidiPdf {
        /// Path to a MIDI file, or "-" for stdin
        #[arg(default_value = "-")]
        input: String,
        /// Output PDF path
        #[arg(short, long, default_value = "chart.pdf")]
        output: PathBuf,
        /// Optional path to also write the generated .kf chart text
        #[arg(long)]
        chart_output: Option<PathBuf>,
    },
    /// Render chart to SVG (one file per page)
    Svg {
        /// Path to a .kf file, or "-" for stdin
        #[arg(default_value = "-")]
        input: String,
        /// Output SVG path (page number appended for multi-page)
        #[arg(short, long, default_value = "chart.svg")]
        output: PathBuf,
    },
    /// Render chart to SVG and print to stdout (for quick inspection)
    Render {
        /// Path to a .kf file, or "-" for stdin
        #[arg(default_value = "-")]
        input: String,
    },
}

fn read_source(input: &str) -> Result<String, String> {
    if input == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("Failed to read stdin: {e}"))?;
        Ok(buf)
    } else {
        std::fs::read_to_string(input).map_err(|e| format!("Failed to read {input}: {e}"))
    }
}

fn read_bytes(input: &str) -> Result<Vec<u8>, String> {
    if input == "-" {
        let mut buf = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buf)
            .map_err(|e| format!("Failed to read stdin: {e}"))?;
        Ok(buf)
    } else {
        std::fs::read(input).map_err(|e| format!("Failed to read {input}: {e}"))
    }
}

fn parse_chart(source: &str) -> Result<Chart, String> {
    keyflow::parse(source).map_err(|e| format!("{e}"))
}

fn parse_midi_chart(input: &str) -> Result<Chart, String> {
    let bytes = read_bytes(input)?;
    keyflow::midi::parse_midi_bytes(&bytes)
}

fn generate_midi_chart_text(input: &str) -> Result<String, String> {
    let bytes = read_bytes(input)?;
    keyflow::midi::generate_chart_text_from_midi_bytes(&bytes)
}

struct LayoutPipeline {
    font_bundle: ChartFontBundle,
    engine: ChartLayoutEngine,
}

impl LayoutPipeline {
    fn new() -> Result<Self, String> {
        let font_bundle = ChartFontBundle::new()?;
        let style: &'static MStyle = Box::leak(Box::new(MStyle::new()));
        let engine = font_bundle.create_layout_engine(style);
        Ok(Self {
            font_bundle,
            engine,
        })
    }

    fn layout(
        &self,
        chart: &Chart,
    ) -> ChartLayoutResult {
        let config = ChartLayoutConfig::master_rhythm().with_page_offsets(true);
        let mode = LayoutMode::Paginated {
            page_width: A4_WIDTH,
            page_height: A4_HEIGHT,
        };
        self.engine.layout_chart_with_config(chart, &mode, &config)
    }

    fn export_svg_pages(
        &self,
        result: &ChartLayoutResult,
    ) -> Vec<String> {
        let mut pages = Vec::with_capacity(result.pages.len());
        for page in &result.pages {
            let config = SvgExportConfig::for_page(
                page.x_offset,
                page.y_offset,
                page.width,
                page.height,
            )
            .with_embedded_font(
                "Bravura",
                self.font_bundle.symbol_font_data().as_ref().clone(),
            )
            .with_embedded_font(
                "MuseJazzText",
                self.font_bundle.text_font_data().as_ref().clone(),
            )
            .with_embedded_font(
                "FreeSans",
                self.font_bundle.aux_font_data().as_ref().clone(),
            );
            let mut serializer = SvgSerializer::new(config);
            pages.push(serializer.serialize(&result.scene));
        }
        pages
    }

    fn export_pdf(
        &self,
        result: &ChartLayoutResult,
    ) -> Result<Vec<u8>, String> {
        let svg_pages = self.export_svg_pages(result);
        let symbol_font = self.font_bundle.symbol_font_data();
        let text_font = self.font_bundle.text_font_data();
        let aux_font = self.font_bundle.aux_font_data();

        PdfSerializer::serialize_from_svg(
            &svg_pages,
            &[
                ("Bravura", symbol_font.as_slice()),
                ("MuseJazzText", text_font.as_slice()),
                ("FreeSans", aux_font.as_slice()),
            ],
        )
        .map_err(|e| format!("Failed to export PDF: {e}"))
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Commands::Parse { input } => {
            let source = read_source(&input)?;
            let chart = parse_chart(&source)?;

            // Print metadata
            println!("=== Chart Structure ===\n");
            println!("Title:    {}", chart.metadata.title.as_deref().unwrap_or("(none)"));
            println!("Artist:   {}", chart.metadata.artist.as_deref().unwrap_or("(none)"));
            if let Some(tempo) = &chart.tempo {
                println!("Tempo:    {}", tempo);
            }
            if let Some(ts) = &chart.initial_time_signature {
                println!("Time Sig: {}/{}", ts.numerator, ts.denominator);
            }
            if let Some(key) = &chart.initial_key {
                println!("Key:      {}", key);
            }
            println!();

            // Print round-tripped display format
            println!("=== Display (round-trip) ===\n");
            println!("{}", chart);

            // Print detailed section breakdown
            println!("\n=== Section Detail ===\n");
            for (i, section) in chart.sections.iter().enumerate() {
                let s = &section.section;
                let comment = s.comment.as_deref().unwrap_or("");
                let comment_str = if comment.is_empty() {
                    String::new()
                } else {
                    format!(" \"{}\"", comment)
                };
                println!(
                    "Section {}: {:?}{} ({} measures)",
                    i,
                    s.section_type,
                    comment_str,
                    section.measures().len()
                );
                for (j, measure) in section.measures().iter().enumerate() {
                    let chords: Vec<String> = measure
                        .chords
                        .iter()
                        .map(|c| c.full_symbol.clone())
                        .collect();
                    let melody_info = if measure.melodies.is_empty() {
                        String::new()
                    } else {
                        let total_notes: usize = measure.melodies.iter().map(|m| m.notes.len()).sum();
                        format!(" + {} melody ({} notes)", measure.melodies.len(), total_notes)
                    };
                    println!("  Measure {}: [{}]{}", j, chords.join(", "), melody_info);
                }
                println!();
            }

            Ok(())
        }

        Commands::Pdf { input, output } => {
            let source = read_source(&input)?;
            let chart = parse_chart(&source)?;
            let pipeline = LayoutPipeline::new()?;
            let layout = pipeline.layout(&chart);

            println!(
                "Layout: {} page(s), {:.0}x{:.0} pt",
                layout.pages.len(),
                layout.total_width,
                layout.total_height
            );

            let pdf_bytes = pipeline.export_pdf(&layout)?;
            std::fs::write(&output, &pdf_bytes)
                .map_err(|e| format!("Failed to write {}: {e}", output.display()))?;
            println!("Wrote {} ({} bytes)", output.display(), pdf_bytes.len());
            Ok(())
        }

        Commands::MidiPdf {
            input,
            output,
            chart_output,
        } => {
            let chart_text = generate_midi_chart_text(&input)?;
            if let Some(chart_output) = chart_output {
                std::fs::write(&chart_output, &chart_text)
                    .map_err(|e| format!("Failed to write {}: {e}", chart_output.display()))?;
                println!("Wrote {}", chart_output.display());
            }
            let chart = parse_chart(&chart_text)?;

            let pipeline = LayoutPipeline::new()?;
            let layout = pipeline.layout(&chart);

            println!(
                "Layout: {} page(s), {:.0}x{:.0} pt",
                layout.pages.len(),
                layout.total_width,
                layout.total_height
            );

            let pdf_bytes = pipeline.export_pdf(&layout)?;
            std::fs::write(&output, &pdf_bytes)
                .map_err(|e| format!("Failed to write {}: {e}", output.display()))?;
            println!("Wrote {} ({} bytes)", output.display(), pdf_bytes.len());
            Ok(())
        }

        Commands::Svg { input, output } => {
            let source = read_source(&input)?;
            let chart = parse_chart(&source)?;
            let pipeline = LayoutPipeline::new()?;
            let layout = pipeline.layout(&chart);

            let svg_pages = pipeline.export_svg_pages(&layout);

            if svg_pages.len() == 1 {
                std::fs::write(&output, &svg_pages[0])
                    .map_err(|e| format!("Failed to write {}: {e}", output.display()))?;
                println!("Wrote {}", output.display());
            } else {
                let stem = output
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("chart");
                let ext = output
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("svg");
                let dir = output.parent().unwrap_or(std::path::Path::new("."));

                for (i, svg) in svg_pages.iter().enumerate() {
                    let page_path = dir.join(format!("{}-p{}.{}", stem, i + 1, ext));
                    std::fs::write(&page_path, svg)
                        .map_err(|e| format!("Failed to write {}: {e}", page_path.display()))?;
                    println!("Wrote {}", page_path.display());
                }
            }
            Ok(())
        }

        Commands::Render { input } => {
            let source = read_source(&input)?;
            let chart = parse_chart(&source)?;
            let pipeline = LayoutPipeline::new()?;
            let layout = pipeline.layout(&chart);
            let svg_pages = pipeline.export_svg_pages(&layout);

            for (i, svg) in svg_pages.iter().enumerate() {
                if svg_pages.len() > 1 {
                    eprintln!("--- Page {} ---", i + 1);
                }
                println!("{}", svg);
            }
            Ok(())
        }
    }
}
