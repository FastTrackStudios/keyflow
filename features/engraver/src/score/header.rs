//! The master rhythm chart's page header, for parts.
//!
//! In keyflow style a part's first page carries the same header a chart
//! does — drawn by the chart's own `add_title_header_with_count_in`, so the
//! two cannot drift: the quoted bold title centred, the part name in caps
//! on the left over its version, the writers down the right, a subtitle
//! under the title — and every page the chart's footer.
//!
//! What goes in it comes from what Finale prints on page 1, because a
//! `.musx` file's own metadata is rarely filled in (half this library's
//! scores call themselves "THE LITTLE DRUMMER CHILD", the template they
//! were started from). The largest centred text is the title; the other
//! centred lines are the subtitle; right-aligned lines are the writers; an
//! "Arranged by" / "Orchestrated by" line anywhere is the arranger, set at
//! the foot of the writers' column. A song's decisions file can override
//! any of them ([`HeaderOverrides`]).

use crate::score::layout::{HAlign, PageText, VAlign};

/// What a part's header shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PartHeader {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    /// One per line, top to bottom.
    pub writers: Vec<String>,
    pub arranger: Option<String>,
}

/// Per-song corrections to the derived header.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeaderOverrides {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    /// Writers, comma-separated.
    pub writers: Option<String>,
    pub arranger: Option<String>,
}

impl HeaderOverrides {
    /// Read `title:`, `subtitle:`, `writers:` and `arranger:` lines from a
    /// song's decisions file; other lines are someone else's. An empty value
    /// clears the field (`subtitle:` with nothing after it).
    #[must_use]
    pub fn parse(src: &str) -> Self {
        let mut out = Self::default();
        for line in src.lines() {
            let line = line.trim();
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let value = Some(value.trim().to_string());
            match key.trim().to_ascii_lowercase().as_str() {
                "title" => out.title = value,
                "subtitle" => out.subtitle = value,
                "writers" => out.writers = value,
                "arranger" => out.arranger = value,
                _ => {}
            }
        }
        out
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Is this line an arranging credit?
fn is_arranger_line(line: &str) -> bool {
    let l = line.to_ascii_lowercase();
    [
        "arr.",
        "arranged",
        "arrangement",
        "orch.",
        "orchestrated",
        "orchestration",
    ]
    .iter()
    .any(|p| l.starts_with(p))
}

/// Text that labels the page rather than the song.
fn is_page_label(line: &str, part_name: &str) -> bool {
    let l = line.trim().to_ascii_lowercase();
    l.is_empty()
        // Bullets and rules, not words.
        || !l.chars().any(char::is_alphanumeric)
        // The arranger's website.
        || l.contains(".com")
        || l.contains("www.")
        || l == part_name.trim().to_ascii_lowercase()
        || l.contains("score")
        || l.contains("{page}")
        || l.starts_with("rev.")
        || l.starts_with("pg.")
}

fn unquote(s: &str) -> String {
    s.trim()
        .trim_matches(|c| c == '"' || c == '“' || c == '”')
        .trim()
        .to_string()
}

impl PartHeader {
    /// Read the header from Finale's page-1 texts.
    #[must_use]
    pub fn from_page_texts(texts: &[PageText], part_name: &str) -> Self {
        let page_one: Vec<&PageText> = texts
            .iter()
            .filter(|t| t.first_page == 1 && t.v_align == VAlign::Top)
            .collect();
        let mut out = Self::default();
        // Title: the largest centred text — a quoted one first, since
        // arrangers quote the song and not the band ("The Alan Parsons
        // Project" can be set larger than the title it sits over).
        let candidates: Vec<&PageText> = page_one
            .iter()
            .copied()
            .filter(|t| {
                t.h_align == HAlign::Center
                    && !is_page_label(&t.text, part_name)
                    && !t.text.lines().any(is_arranger_line)
            })
            .collect();
        let quoted = |t: &PageText| {
            let s = t.text.trim();
            s.starts_with(['"', '“']) && s.ends_with(['"', '”'])
        };
        let by_size = |a: &&PageText, b: &&PageText| a.font.size.total_cmp(&b.font.size);
        let title = candidates
            .iter()
            .copied()
            .filter(|t| quoted(t))
            .max_by(by_size)
            .or_else(|| candidates.iter().copied().max_by(by_size));
        if let Some(t) = title {
            out.title = Some(unquote(&t.text.replace('\n', " ")));
        }
        let mut subtitle: Vec<String> = Vec::new();
        for t in &page_one {
            if title.is_some_and(|tt| std::ptr::eq(tt, *t)) {
                continue;
            }
            for line in t.text.lines().map(str::trim) {
                if is_page_label(line, part_name) {
                    continue;
                }
                if is_arranger_line(line) {
                    out.arranger.get_or_insert_with(|| line.to_string());
                    continue;
                }
                match t.h_align {
                    HAlign::Center => subtitle.push(line.to_string()),
                    HAlign::Right => out.writers.push(line.to_string()),
                    // The left column is the part name's.
                    HAlign::Left => {}
                }
            }
        }
        if !subtitle.is_empty() {
            out.subtitle = Some(subtitle.join(" · "));
        }
        out
    }

    /// Apply a song's corrections.
    pub fn apply(&mut self, o: &HeaderOverrides) {
        let some = |v: &String| (!v.is_empty()).then(|| v.clone());
        if let Some(v) = &o.title {
            self.title = some(v);
        }
        if let Some(v) = &o.subtitle {
            self.subtitle = some(v);
        }
        if let Some(v) = &o.writers {
            self.writers = v
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
        }
        if let Some(v) = &o.arranger {
            self.arranger = some(v);
        }
    }

    /// The chart header's metadata. Its right column is one comma-separated
    /// field (a line per name): the writers, then the arranger.
    #[must_use]
    pub fn metadata(&self, part_name: &str) -> keyflow_proto::SongMetadata {
        let mut right = self.writers.clone();
        right.extend(self.arranger.clone());
        keyflow_proto::SongMetadata {
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            composer: (!right.is_empty()).then(|| right.join(", ")),
            part_name: Some(part_name.to_string()),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::layout::TextFont;

    fn text(s: &str, size: f64, h: HAlign) -> PageText {
        PageText {
            text: s.into(),
            font: TextFont::serif(size),
            first_page: 1,
            last_page: Some(1),
            h_align: h,
            v_align: VAlign::Top,
            x: 0.0,
            y: 0.0,
        }
    }

    #[test]
    fn reads_finale_page_one() {
        let texts = [
            text("Violin 1", 24.0, HAlign::Left),
            text("\"I CAN'T GET THERE FROM HERE\"", 18.0, HAlign::Center),
            text(
                "The Alan Parsons Project\nOrchestrated by Tom Brooks",
                14.0,
                HAlign::Center,
            ),
            text(
                "Patrick Read Johnson\nJared Mahone\nAlan Parsons",
                12.0,
                HAlign::Right,
            ),
        ];
        let h = PartHeader::from_page_texts(&texts, "Violin 1");
        assert_eq!(h.title.as_deref(), Some("I CAN'T GET THERE FROM HERE"));
        assert_eq!(h.subtitle.as_deref(), Some("The Alan Parsons Project"));
        assert_eq!(h.arranger.as_deref(), Some("Orchestrated by Tom Brooks"));
        assert_eq!(
            h.writers,
            ["Patrick Read Johnson", "Jared Mahone", "Alan Parsons"]
        );
        let m = h.metadata("Violin 1");
        assert_eq!(
            m.composer.as_deref(),
            Some("Patrick Read Johnson, Jared Mahone, Alan Parsons, Orchestrated by Tom Brooks")
        );
    }

    #[test]
    fn overrides_win() {
        let mut h = PartHeader {
            title: Some("Years of Glory".into()),
            subtitle: Some("Concert".into()),
            ..Default::default()
        };
        h.apply(&HeaderOverrides::parse(
            "remark: x\nsubtitle:\nwriters: PJ Olsson\narranger: Arr. Milt Olsson\n",
        ));
        assert_eq!(h.subtitle, None);
        assert_eq!(h.writers, ["PJ Olsson"]);
        assert_eq!(h.arranger.as_deref(), Some("Arr. Milt Olsson"));
    }
}
