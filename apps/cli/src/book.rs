//! `kf book` — per-player part books in setlist order.
//!
//! A setlist is a plain text file, one song per line, in running order:
//!
//! ```text
//! title: Alan Parsons — Columbus Symphony, July 25 2026
//! 1. Standing On Higher Ground
//! 2. Don't Answer Me
//! I Wouldn't Wanna Be Like You [tacet]
//! === Intermission ===
//! Sagrada = La Sagrada Familia
//! ```
//!
//! Numbering is optional; `=`/`-` rule lines are section breaks; `[tacet]`
//! keeps the song in every book as a TACET page; `Shown = Library Name`
//! names a song differently from its library folder. Each song resolves to
//! a `.musx` with linked parts in the library (one folder per song, as in
//! "APLP Finale Scores Complete/By Song"), engraved fresh; failing that, to
//! existing part PDFs (`--pdf-parts`).
//!
//! Books are per CHAIR, not per part name: orchestrations differ song to
//! song ("Flute 1-2" here, "Flute 1" there, "Horns 2-5" elsewhere), so each
//! part is read as an instrument family plus the seats it covers, and each
//! seat's book takes, per song, the narrowest part covering it. Percussion
//! shares one book. A chair with nothing to play gets a TACET page.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use engraver_score::finale::{FinaleDocument, PrintInfo};
use engraver_score::layout::layout_part;
use keyflow::engraver::fonts::ChartFontBundle;

/// One line of a setlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetItem {
    Song {
        /// As the program prints it.
        shown: String,
        /// Library folder name to resolve against.
        lookup: String,
        tacet: bool,
    },
    Break(String),
}

#[derive(Debug, Clone, Default)]
pub struct Setlist {
    pub title: String,
    pub items: Vec<SetItem>,
}

/// Parse the setlist text format described in the module docs.
#[must_use]
pub fn parse_setlist(text: &str, fallback_title: &str) -> Setlist {
    let mut set = Setlist {
        title: fallback_title.to_string(),
        items: Vec::new(),
    };
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(t) = line.strip_prefix("title:") {
            set.title = t.trim().to_string();
            continue;
        }
        if line.starts_with("==") || line.starts_with("--") || line.starts_with('—') {
            let label = line
                .trim_matches(|c: char| matches!(c, '=' | '-' | '—') || c.is_whitespace())
                .to_string();
            set.items.push(SetItem::Break(label));
            continue;
        }
        // Optional "12." / "12)" numbering.
        let body = if line.starts_with(|c: char| c.is_ascii_digit()) {
            line.trim_start_matches(|c: char| c.is_ascii_digit())
                .trim_start_matches(['.', ')'])
                .trim()
        } else {
            line
        };
        let tacet = body.to_lowercase().contains("[tacet]");
        let body = body
            .replace("[tacet]", "")
            .replace("[TACET]", "")
            .replace("[Tacet]", "");
        let body = body.trim();
        let (shown, lookup) = match body.split_once(" = ") {
            Some((shown, lookup)) => (shown.trim(), lookup.trim()),
            None => (body, body),
        };
        set.items.push(SetItem::Song {
            shown: shown.to_string(),
            lookup: lookup.to_string(),
            tacet,
        });
    }
    set
}

/// Comparison key: lowercase letters and digits only ("&" reads "and").
fn key(s: &str) -> String {
    s.to_lowercase()
        .replace('&', "and")
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

/// The library folder for a song: exact key match, else the closest
/// containment match (either way round).
fn find_song_dir(library: &Path, lookup: &str) -> Option<PathBuf> {
    let want = key(lookup);
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(library)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    let name_key = |p: &Path| key(&p.file_name().unwrap_or_default().to_string_lossy());
    if let Some(d) = dirs.iter().find(|d| name_key(d) == want) {
        return Some(d.clone());
    }
    dirs.iter()
        .filter(|d| {
            let k = name_key(d);
            k.len() >= 4 && want.len() >= 4 && (k.contains(&want) || want.contains(&k))
        })
        .min_by_key(|d| name_key(d).len().abs_diff(want.len()))
        .cloned()
}

/// The first `.musx` in a song folder that has linked parts, scores first.
fn load_score(dir: &Path) -> Option<(PathBuf, FinaleDocument)> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("musx"))
        })
        .collect();
    // Full scores before rhythm-section charts.
    files.sort_by_key(|p| {
        let n = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        (
            !n.contains("score"),
            n.contains("rs chart") || n.contains("rhythm"),
            n,
        )
    });
    for f in files {
        let Ok(bytes) = std::fs::read(&f) else {
            continue;
        };
        let Ok(doc) = FinaleDocument::from_musx(&bytes) else {
            continue;
        };
        if !doc.parts(&PrintInfo::default()).is_empty() {
            return Some((f, doc));
        }
    }
    None
}

/// Instrument families in score order: (match keyword, book display name).
/// Longer keywords are tried first, so "bass clarinet" beats "clarinet".
const FAMILIES: &[(&str, &str)] = &[
    ("piccolo trumpet", "Piccolo Trumpet"),
    ("trp piccolo", "Piccolo Trumpet"),
    ("piccolo", "Piccolo"),
    ("flute", "Flute"),
    ("oboe", "Oboe"),
    ("english horn", "English Horn"),
    ("bass clarinet", "Bass Clarinet"),
    ("clarinet", "Clarinet"),
    ("contrabassoon", "Contrabassoon"),
    ("bassoon", "Bassoon"),
    ("horn", "Horn"),
    ("trumpet", "Trumpet"),
    ("trp", "Trumpet"),
    ("bass trombone", "Bass Trombone"),
    ("trombone", "Trombone"),
    ("tuba", "Tuba"),
    ("timpani", "Timpani"),
    ("percussion", "Percussion"),
    ("harp", "Harp"),
    ("piano", "Piano"),
    ("celesta", "Celesta"),
    ("violin", "Violin"),
    ("viola", "Viola"),
    ("violoncello", "Cello"),
    ("cello", "Cello"),
    ("double bass", "Double Bass"),
    ("contrabass", "Double Bass"),
    ("string bass", "Double Bass"),
];

/// Percussion instruments a part may be named after; all share the
/// Percussion book.
const PERCUSSION: &[&str] = &[
    "perc",
    "castanet",
    "glockenspiel",
    "glock",
    "xylophone",
    "vibraphone",
    "marimba",
    "cymbal",
    "snare",
    "side drum",
    "bass drum",
    "triangle",
    "tambourine",
    "chimes",
    "tubular bells",
    "bells",
    "tam-tam",
    "gong",
    "wood block",
];

/// Not orchestra books: rhythm section, choir, chord charts.
const NOT_ORCHESTRA: &[&str] = &["chord", "rhythm", "r.s", "choir", "vox", "s-a", "click"];

/// What a part is: an instrument family and the seats it covers (empty =
/// every seat, as an unnumbered "Viola" part is for the whole section).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartRole {
    pub family: &'static str,
    pub seats: BTreeSet<u32>,
}

/// Read a part name as a [`PartRole`]; `None` for non-orchestra parts.
/// "Horn in F 1-2" → Horn {1,2}; "Violin II" → Violin {2}; "Flute 1-2-3"
/// → Flute {1,2,3}; "Horn in F 2/4" → Horn {2,4}; "Castanets" → Percussion.
#[must_use]
pub fn part_role(name: &str) -> Option<PartRole> {
    let s = name
        .to_lowercase()
        .replace(['♭', '\u{266D}'], "b")
        .replace(['\n', '\r'], " ");
    if NOT_ORCHESTRA.iter().any(|k| s.contains(k)) {
        return None;
    }
    // The family keyword that appears EARLIEST names the part ("Oboe 1 /
    // English Horn" is the oboe player's doubling part); ties go to the
    // longer keyword.
    let found = FAMILIES
        .iter()
        .filter_map(|(kw, display)| {
            s.find(kw)
                .map(|pos| (pos, std::cmp::Reverse(kw.len()), *kw, *display))
        })
        .min();
    let (rest, family) = match found {
        Some((pos, _, kw, display)) if display != "Percussion" => (&s[pos + kw.len()..], display),
        _ if PERCUSSION.iter().any(|k| s.contains(k)) || found.is_some() => {
            return Some(PartRole {
                family: "Percussion",
                seats: BTreeSet::new(),
            });
        }
        _ => {
            return Some(PartRole {
                family: "Other",
                seats: BTreeSet::new(),
            });
        }
    };
    // Seat numbers after the family word, skipping a transposition
    // ("in F", "in Bb"): arabic or roman, joined by - / & or spaces.
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut seps: Vec<char> = Vec::new();
    for c in rest.chars() {
        if c.is_ascii_alphanumeric() {
            current.push(c);
        } else {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            if matches!(c, '-' | '/' | '&' | ',') {
                seps.push(c);
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    let numbers: Vec<u32> = tokens
        .iter()
        .filter_map(|t| match t.as_str() {
            "i" => Some(1),
            "ii" => Some(2),
            "iii" => Some(3),
            "iv" => Some(4),
            t => t.parse().ok(),
        })
        .collect();
    let seats: BTreeSet<u32> = match numbers.as_slice() {
        // "2-5" is a range; "1-2-3", "2/4" and "1 & 3" are lists.
        [a, b] if seps.first() == Some(&'-') && b > a => (*a..=*b).collect(),
        ns => ns.iter().copied().filter(|n| *n > 0 && *n < 20).collect(),
    };
    Some(PartRole { family, seats })
}

/// Doubled-section numbering: three horn parts "1-4", "2-5", "3-6" are
/// pairs (1&4, 2&5, 3&6 — six horns, each part read by two players), not
/// overlapping ranges. Detected per song and family: every part spans the
/// same k+1 seats and there are exactly k of them.
fn fold_paired_ranges(parts: &mut [SongPart]) {
    let families: BTreeSet<&'static str> = parts.iter().map(|p| p.role.family).collect();
    for family in families {
        let spans: Vec<(u32, u32, usize)> = parts
            .iter()
            .filter(|p| p.role.family == family)
            .filter_map(|p| {
                let (lo, hi) = (*p.role.seats.first()?, *p.role.seats.last()?);
                let contiguous = (hi - lo + 1) as usize == p.role.seats.len();
                contiguous.then_some((lo, hi, p.role.seats.len()))
            })
            .collect();
        let count = parts.iter().filter(|p| p.role.family == family).count();
        let Some(&(_, _, len)) = spans.first() else {
            continue;
        };
        let paired = len > 2
            && spans.len() == count
            && spans.iter().all(|(_, _, l)| *l == len)
            && count == len - 1;
        if paired {
            for p in parts.iter_mut().filter(|p| p.role.family == family) {
                let (lo, hi) = (
                    *p.role.seats.first().expect("span"),
                    *p.role.seats.last().expect("span"),
                );
                p.role.seats = [lo, hi].into_iter().collect();
            }
        }
    }
}

/// A part available for one song, ready to file into books.
struct SongPart {
    role: PartRole,
    name: String,
    pdf: Vec<u8>,
}

/// A book: one player's chair.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ChairId {
    rank: usize,
    family: &'static str,
    seat: Option<u32>,
}

impl ChairId {
    fn display(&self) -> String {
        match (self.family, self.seat) {
            ("Violin", Some(1)) => "Violin I".to_string(),
            ("Violin", Some(2)) => "Violin II".to_string(),
            (f, Some(n)) => format!("{f} {n}"),
            (f, None) => f.to_string(),
        }
    }
}

fn family_rank(family: &str) -> usize {
    FAMILIES
        .iter()
        .position(|(_, d)| *d == family)
        .unwrap_or(FAMILIES.len())
}

/// The parts of one song a chair reads: those of its family covering its
/// seat (or the whole section), narrowest coverage first. Percussion takes
/// every percussion part.
fn parts_for_chair<'a>(chair: &ChairId, parts: &'a [SongPart]) -> Vec<&'a SongPart> {
    let family: Vec<&SongPart> = parts
        .iter()
        .filter(|p| p.role.family == chair.family)
        .collect();
    if chair.family == "Percussion" || chair.family == "Other" {
        return family;
    }
    let covering: Vec<&SongPart> = family
        .into_iter()
        .filter(|p| match chair.seat {
            Some(seat) => p.role.seats.is_empty() || p.role.seats.contains(&seat),
            None => true,
        })
        .collect();
    let span = |p: &SongPart| {
        if p.role.seats.is_empty() {
            usize::MAX
        } else {
            p.role.seats.len()
        }
    };
    let Some(best) = covering.iter().map(|p| span(p)).min() else {
        return Vec::new();
    };
    covering.into_iter().filter(|p| span(p) == best).collect()
}

/// Existing part PDFs for a song under `dir`: (instrument name, path).
fn pdf_parts_for(dir: &Path, lookup: &str) -> Vec<(String, PathBuf)> {
    let want = key(lookup);
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.filter_map(Result::ok) {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if !p.extension().is_some_and(|x| x.eq_ignore_ascii_case("pdf")) {
                continue;
            }
            let stem = p
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let Some((song, instrument)) = stem.rsplit_once(" - ") else {
                continue;
            };
            let song = song
                .replace("_APP", "")
                .replace("-Instrument Parts", "")
                .replace(" Orchestra Parts", "");
            if key(&song) != want || instrument.to_lowercase().contains("score") {
                continue;
            }
            let instrument = instrument.trim_end_matches(" copy").trim().to_string();
            out.push((instrument, p));
        }
    }
    // One file per instrument (the same part is often filed twice).
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out.dedup_by(|a, b| key(&a.0) == key(&b.0));
    out
}

/// A plain text page (cover, TACET) as an SVG of the parts' page size.
fn text_page(width: f64, height: f64, lines: &[(f64, f64, f64, bool, &str)]) -> String {
    use std::fmt::Write as _;
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}"><rect width="100%" height="100%" fill="white"/>"#
    );
    for (x, y, size, bold, text) in lines {
        let weight = if *bold { "bold" } else { "normal" };
        let _ = write!(
            svg,
            r#"<text x="{x}" y="{y}" font-family="FreeSerif" font-size="{size}" font-weight="{weight}" text-anchor="middle">{}</text>"#,
            esc(text)
        );
    }
    svg.push_str("</svg>");
    svg
}

fn svg_pdf(pages: &[String]) -> Result<Vec<u8>, String> {
    use keyflow::engraver::export::pdf::PdfSerializer;
    let faces: Vec<(&str, &[u8])> = engraver_score::fonts::text_faces().to_vec();
    PdfSerializer::serialize_from_svg(pages, &faces).map_err(|e| e.to_string())
}

/// Concatenate PDFs, in order, into one document.
fn merge_pdfs(parts: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    use lopdf::{Document, Object, ObjectId};
    let mut max_id = 1;
    // Pages in reading order (a BTreeMap by id would reorder them).
    let mut pages: Vec<(ObjectId, Object)> = Vec::new();
    let mut objects: BTreeMap<ObjectId, Object> = BTreeMap::new();
    for bytes in parts {
        let mut doc = Document::load_mem(bytes).map_err(|e| format!("read pdf: {e}"))?;
        doc.renumber_objects_with(max_id);
        max_id = doc.max_id + 1;
        for (_, id) in doc.get_pages() {
            let page = doc.get_object(id).map_err(|e| e.to_string())?.clone();
            pages.push((id, page));
        }
        objects.extend(doc.objects);
    }
    let mut out = Document::with_version("1.5");
    let mut catalog: Option<(ObjectId, Object)> = None;
    let mut pages_root: Option<(ObjectId, Object)> = None;
    for (id, object) in &objects {
        match object.type_name().unwrap_or(b"") {
            b"Catalog" => {
                if catalog.is_none() {
                    catalog = Some((*id, object.clone()));
                }
            }
            b"Pages" => {
                if pages_root.is_none() {
                    pages_root = Some((*id, object.clone()));
                }
            }
            b"Page" | b"Outlines" | b"Outline" => {}
            _ => {
                out.objects.insert(*id, object.clone());
            }
        }
    }
    let (pages_id, pages_obj) = pages_root.ok_or("no page tree")?;
    let (catalog_id, catalog_obj) = catalog.ok_or("no catalog")?;
    for (id, page) in &pages {
        let mut dict = page.as_dict().map_err(|e| e.to_string())?.clone();
        dict.set("Parent", pages_id);
        out.objects.insert(*id, Object::Dictionary(dict));
    }
    let mut pages_dict = pages_obj.as_dict().map_err(|e| e.to_string())?.clone();
    pages_dict.set("Count", pages.len() as u32);
    pages_dict.set(
        "Kids",
        pages
            .iter()
            .map(|(id, _)| Object::Reference(*id))
            .collect::<Vec<_>>(),
    );
    out.objects.insert(pages_id, Object::Dictionary(pages_dict));
    let mut catalog_dict = catalog_obj.as_dict().map_err(|e| e.to_string())?.clone();
    catalog_dict.set("Pages", pages_id);
    catalog_dict.remove(b"Outlines");
    out.objects
        .insert(catalog_id, Object::Dictionary(catalog_dict));
    out.trailer.set("Root", catalog_id);
    out.max_id = out.objects.len() as u32;
    out.renumber_objects();
    out.compress();
    let mut buf = Vec::new();
    out.save_to(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

pub struct BookArgs<'a> {
    pub setlist: &'a Path,
    pub library: &'a Path,
    pub pdf_parts: Option<&'a Path>,
    pub out_dir: Option<&'a Path>,
    pub chair: Option<&'a str>,
    pub prefer_pdf: bool,
    pub dry_run: bool,
    /// See [`crate::parts::auto_layout`].
    pub auto_layout: bool,
    pub marks: engraver_score::section_marks::MarkStyle,
    /// Per-song `.roles` files, named for the song's folder in the library
    /// (`Don't Answer Me.roles`).
    pub roles_dir: Option<&'a Path>,
}

pub fn cmd_book(args: &BookArgs<'_>) -> Result<(), String> {
    let text = std::fs::read_to_string(args.setlist)
        .map_err(|e| format!("{}: {e}", args.setlist.display()))?;
    let stem = args
        .setlist
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let set = parse_setlist(&text, &stem);
    let fonts = ChartFontBundle::shared()?;

    let songs: Vec<(String, String, bool)> = set
        .items
        .iter()
        .filter_map(|i| match i {
            SetItem::Song {
                shown,
                lookup,
                tacet,
            } => Some((shown.clone(), lookup.clone(), *tacet)),
            SetItem::Break(_) => None,
        })
        .collect();

    // Page size for covers/tacets: the first engraved part's, else 9×12.
    let mut page_size: Option<(f64, f64)> = None;
    // Per song: the parts available to file into books.
    let mut song_parts: Vec<Vec<SongPart>> = Vec::new();

    for (idx, (shown, lookup, tacet)) in songs.iter().enumerate() {
        let mut parts: Vec<SongPart> = Vec::new();
        let mut source = String::from("no parts found");
        if *tacet {
            source = "tacet".into();
        } else {
            if !args.prefer_pdf
                && let Some(dir) = find_song_dir(args.library, lookup)
                && let Some((path, doc)) = load_score(&dir)
            {
                let print = PrintInfo {
                    date: crate::parts::file_date(&path),
                    file_name: path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                };
                let roles = match args.roles_dir {
                    Some(rd) => {
                        let file = rd.join(format!(
                            "{}.roles",
                            dir.file_name().unwrap_or_default().to_string_lossy()
                        ));
                        if file.exists() {
                            crate::parts::read_roles(&file)?
                        } else {
                            engraver_score::section_marks::TextRoles::default()
                        }
                    }
                    None => engraver_score::section_marks::TextRoles::default(),
                };
                for job in doc.parts(&print) {
                    let Some(role) = part_role(&job.name) else {
                        continue;
                    };
                    let pdf = if args.dry_run {
                        Vec::new()
                    } else {
                        page_size.get_or_insert((job.options.page_width, job.options.page_height));
                        let mut options = job.options.clone();
                        options.marks = args.marks;
                        options.text_roles = roles.for_part(&job.name);
                        if args.auto_layout {
                            crate::parts::auto_layout(&mut options);
                        }
                        let layout = layout_part(&doc.score, job.part_index, &options);
                        engraver_score::render::pdf(&layout, fonts)?
                    };
                    parts.push(SongPart {
                        role,
                        name: job.name.clone(),
                        pdf,
                    });
                }
                source = format!(
                    "engraved from {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                );
            }
            if parts.is_empty()
                && let Some(pdf_dir) = args.pdf_parts
            {
                for (name, path) in pdf_parts_for(pdf_dir, lookup) {
                    let Some(role) = part_role(&name) else {
                        continue;
                    };
                    let pdf = if args.dry_run {
                        Vec::new()
                    } else {
                        std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?
                    };
                    parts.push(SongPart { role, name, pdf });
                }
                if !parts.is_empty() {
                    source = "existing part PDFs".into();
                }
            }
        }
        fold_paired_ranges(&mut parts);
        let names: Vec<&str> = parts.iter().map(|p| p.name.as_str()).collect();
        println!("{:>2}. {shown}: {source} — {}", idx + 1, names.join(", "));
        song_parts.push(parts);
    }

    // Chairs: per family, one per seat number seen in the set (seat-less
    // families get a single book).
    let mut seats: BTreeMap<&'static str, BTreeSet<u32>> = BTreeMap::new();
    for part in song_parts.iter().flatten() {
        let entry = seats.entry(part.role.family).or_default();
        if !matches!(part.role.family, "Percussion" | "Other") {
            entry.extend(part.role.seats.iter().copied());
        }
    }
    let mut chairs: Vec<ChairId> = Vec::new();
    for (family, set_seats) in &seats {
        let rank = family_rank(family);
        if set_seats.is_empty() {
            chairs.push(ChairId {
                rank,
                family,
                seat: None,
            });
        } else {
            let max = *set_seats.iter().max().unwrap_or(&1);
            for seat in 1..=max {
                chairs.push(ChairId {
                    rank,
                    family,
                    seat: Some(seat),
                });
            }
        }
    }
    chairs.sort();
    if let Some(filter) = args.chair {
        let f = filter.to_lowercase();
        chairs.retain(|c| c.display().to_lowercase().contains(&f));
    }

    println!("\n{} book(s):", chairs.len());
    for chair in &chairs {
        let reads: Vec<String> = song_parts
            .iter()
            .enumerate()
            .filter_map(|(i, parts)| {
                let got = parts_for_chair(chair, parts);
                (!got.is_empty()).then(|| {
                    let names: Vec<&str> = got.iter().map(|p| p.name.as_str()).collect();
                    format!("{}:{}", i + 1, names.join("+"))
                })
            })
            .collect();
        println!(
            "  {:<18} plays {:>2}/{}  {}",
            chair.display(),
            reads.len(),
            songs.len(),
            reads.join("  ")
        );
    }
    if args.dry_run {
        return Ok(());
    }

    let out_dir = args
        .out_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(crate::parts::file_safe(&set.title)));
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let (w, h) = page_size.unwrap_or((648.0, 864.0));

    for chair in &chairs {
        let display = chair.display();
        let mut docs: Vec<Vec<u8>> = Vec::new();
        // Cover: show title, chair, and the program with TACET marks.
        let mut lines: Vec<(f64, f64, f64, bool, String)> = vec![
            (w / 2.0, 120.0, 24.0, true, set.title.clone()),
            (w / 2.0, 160.0, 20.0, false, display.clone()),
        ];
        let mut y = 230.0;
        let mut n = 0;
        for item in &set.items {
            match item {
                SetItem::Break(label) => {
                    if !label.is_empty() {
                        lines.push((w / 2.0, y + 4.0, 13.0, true, label.clone()));
                        y += 26.0;
                    }
                }
                SetItem::Song { shown, .. } => {
                    let plays = !parts_for_chair(chair, &song_parts[n]).is_empty();
                    let entry = if plays {
                        format!("{}. {shown}", n + 1)
                    } else {
                        format!("{}. {shown} — TACET", n + 1)
                    };
                    lines.push((w / 2.0, y, 14.0, false, entry));
                    y += 22.0;
                    n += 1;
                }
            }
        }
        let refs: Vec<(f64, f64, f64, bool, &str)> = lines
            .iter()
            .map(|(x, y, s, b, t)| (*x, *y, *s, *b, t.as_str()))
            .collect();
        docs.push(svg_pdf(&[text_page(w, h, &refs)])?);

        for (idx, (title, _, _)) in songs.iter().enumerate() {
            let got = parts_for_chair(chair, &song_parts[idx]);
            if got.is_empty() {
                docs.push(svg_pdf(&[text_page(
                    w,
                    h,
                    &[
                        (w / 2.0, 90.0, 14.0, false, display.as_str()),
                        (w / 2.0, h / 2.0 - 20.0, 30.0, true, title.as_str()),
                        (w / 2.0, h / 2.0 + 30.0, 36.0, true, "TACET"),
                    ],
                )])?);
            } else {
                docs.extend(got.iter().map(|p| p.pdf.clone()));
            }
        }
        let merged = merge_pdfs(&docs)?;
        let path = out_dir.join(crate::parts::file_safe(&format!(
            "{} - {}.pdf",
            set.title, display
        )));
        std::fs::write(&path, merged).map_err(|e| e.to_string())?;
        println!("{}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_setlists() {
        let set = parse_setlist(
            "title: Columbus 2026\n1. Standing On Higher Ground\n2) Don't Answer Me\n\
             I Wouldn't Wanna Be Like You [tacet]\n=== Intermission ===\nSagrada = La Sagrada Familia\n",
            "x",
        );
        assert_eq!(set.title, "Columbus 2026");
        assert_eq!(
            set.items[1],
            SetItem::Song {
                shown: "Don't Answer Me".into(),
                lookup: "Don't Answer Me".into(),
                tacet: false
            }
        );
        assert!(matches!(&set.items[2], SetItem::Song { tacet: true, .. }));
        assert_eq!(set.items[3], SetItem::Break("Intermission".into()));
        assert_eq!(
            set.items[4],
            SetItem::Song {
                shown: "Sagrada".into(),
                lookup: "La Sagrada Familia".into(),
                tacet: false
            }
        );
    }

    fn role(name: &str) -> (&'static str, Vec<u32>) {
        let r = part_role(name).unwrap();
        (r.family, r.seats.into_iter().collect())
    }

    #[test]
    fn reads_part_roles() {
        assert_eq!(role("Violin I"), ("Violin", vec![1]));
        assert_eq!(role("Violin II"), ("Violin", vec![2]));
        assert_eq!(role("Viola"), ("Viola", vec![]));
        assert_eq!(role("Horn in F 1-2"), ("Horn", vec![1, 2]));
        assert_eq!(role("Horn in F 2-5"), ("Horn", vec![2, 3, 4, 5]));
        assert_eq!(role("Horn in F 2/4"), ("Horn", vec![2, 4]));
        assert_eq!(role("Flute 1-2-3"), ("Flute", vec![1, 2, 3]));
        assert_eq!(role("Clarinet in B♭ 1-2"), ("Clarinet", vec![1, 2]));
        assert_eq!(role("Bass Clarinet"), ("Bass Clarinet", vec![]));
        assert_eq!(role("Bass Trombone"), ("Bass Trombone", vec![]));
        assert_eq!(role("Oboe 1  English Horn"), ("Oboe", vec![1]));
        assert_eq!(role("Bassoon 1-2\nContrabassoon"), ("Bassoon", vec![1, 2]));
        assert_eq!(role("Castanets"), ("Percussion", vec![]));
        assert_eq!(role("Percussion 3-4"), ("Percussion", vec![]));
        assert_eq!(role("Double Bass"), ("Double Bass", vec![]));
        assert_eq!(role("Trp Piccolo C"), ("Piccolo Trumpet", vec![]));
        assert!(part_role("Chord Changes").is_none());
        assert!(part_role("CHOIR").is_none());
        assert!(part_role("Rhythm/Vox").is_none());
    }

    #[test]
    fn chairs_take_the_narrowest_covering_part() {
        let part = |name: &str| SongPart {
            role: part_role(name).unwrap(),
            name: name.to_string(),
            pdf: Vec::new(),
        };
        let parts = vec![part("Horn in F 1-3"), part("Horn in F 3-4")];
        let horn = |seat| ChairId {
            rank: 0,
            family: "Horn",
            seat: Some(seat),
        };
        let names = |seat| -> Vec<String> {
            parts_for_chair(&horn(seat), &parts)
                .iter()
                .map(|p| p.name.clone())
                .collect()
        };
        assert_eq!(names(2), vec!["Horn in F 1-3"]);
        assert_eq!(names(3), vec!["Horn in F 3-4"]);
        assert_eq!(names(4), vec!["Horn in F 3-4"]);
        // An unnumbered section part reaches every seat.
        let strings = vec![part("Viola")];
        let viola = ChairId {
            rank: 0,
            family: "Viola",
            seat: None,
        };
        assert_eq!(parts_for_chair(&viola, &strings).len(), 1);
    }

    #[test]
    fn doubled_sections_read_as_pairs() {
        let part = |name: &str| SongPart {
            role: part_role(name).unwrap(),
            name: name.to_string(),
            pdf: Vec::new(),
        };
        let seats = |parts: &[SongPart]| -> Vec<Vec<u32>> {
            parts
                .iter()
                .map(|p| p.role.seats.iter().copied().collect())
                .collect()
        };
        let mut six = vec![
            part("Horn in F 1-4"),
            part("Horn in F 2-5"),
            part("Horn in F 3-6"),
        ];
        fold_paired_ranges(&mut six);
        assert_eq!(seats(&six), vec![vec![1, 4], vec![2, 5], vec![3, 6]]);
        // "1-3" + "3-4" is not that pattern: ranges stay.
        let mut dam = vec![part("Horn in F 1-3"), part("Horn in F 3-4")];
        fold_paired_ranges(&mut dam);
        assert_eq!(seats(&dam), vec![vec![1, 2, 3], vec![3, 4]]);
    }

    #[test]
    fn song_keys_ignore_punctuation() {
        assert_eq!(key("Silence & I"), key("Silence And I"));
        assert_eq!(key("Don't Answer Me"), "dontanswerme");
    }
}
