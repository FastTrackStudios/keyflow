//! What a chart says about itself, for listing and discovery.
//!
//! A gallery needs to show a chart without engraving it and search a
//! thousand charts without parsing them: the title, who wrote it, what
//! key it is in, how fast, how long, which sections it has. All of that
//! is already in the parsed [`Chart`] — this is the projection of it that
//! a card, a search index or a filter facet actually wants.
//!
//! # Why this lives in the language and not in the gallery
//!
//! A sharing platform has two places that need these facts and they must
//! agree: the **server**, which derives them once on upload to build the
//! index, and the **browser**, which shows them on a card and may derive
//! them again for a chart it holds locally. If those two carried separate
//! implementations they would drift, and the failure mode is quiet — a
//! chart filed under the wrong key, a search that cannot find a song
//! whose card is visibly right there.
//!
//! So the answer is derived once, here, from the parse. Neither side owns
//! it; both call it.
//!
//! Nothing in this module knows what a *shared* chart is — who uploaded
//! it, when, to which project, or who may read it. That belongs to
//! whatever service is storing charts, not to the notation.

use keyflow_proto::Chart;

/// A chart's own description of itself.
///
/// Every field is derived from the chart's content. Two identical sources
/// always produce an identical summary, which is what lets the server
/// index on upload and the browser render from a cache without the two
/// disagreeing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChartSummary {
    /// The song's title, if it names one.
    pub title: Option<String>,
    /// The artist, if the title line carries one.
    pub artist: Option<String>,
    /// Anyone else the chart credits — composer, writer, arranger,
    /// lyricist — in that order, deduplicated.
    ///
    /// One field rather than four because a listing shows "by ..." and
    /// does not care which role filled it in; the roles are still on
    /// [`Chart::metadata`] for anything that does.
    pub credits: Vec<String>,
    /// The key the chart *starts* in, as it would be written.
    ///
    /// The starting key, not the current one: a chart that modulates is
    /// still "in" the key it opens in, and that is what someone scanning
    /// a list is matching against.
    pub key: Option<String>,
    /// Whether the chart changes key at any point.
    pub modulates: bool,
    /// Beats per minute, if the chart states one.
    pub tempo: Option<u32>,
    /// Section labels in the order they appear — `["IN", "VS", "CH"]`.
    ///
    /// Duplicates are kept: a chart with two verses reads differently
    /// from one with a verse, and the shape of a song is the sequence.
    pub sections: Vec<String>,
    /// Total bars across every section.
    pub measures: usize,
    /// The year, if the chart states one.
    pub year: Option<u16>,
}

impl ChartSummary {
    /// Describe a parsed chart.
    #[must_use]
    pub fn of(chart: &Chart) -> Self {
        let m = &chart.metadata;

        // Order is deliberate — composer first, because on a lead sheet
        // that is the credit printed top-right — and duplicates are
        // dropped, because a chart that names the same person as both
        // composer and lyricist should credit them once.
        let mut credits = Vec::new();
        for who in [&m.composer, &m.writer, &m.arranger, &m.lyricist] {
            if let Some(name) = who {
                let name = name.trim();
                if !name.is_empty() && !credits.iter().any(|c: &String| c == name) {
                    credits.push(name.to_string());
                }
            }
        }

        Self {
            title: non_empty(m.title.as_deref()),
            artist: non_empty(m.artist.as_deref()),
            credits,
            // `short_name`, not `Display`. `Display` renders the mode
            // in full — "A Ionian" — which is what a theory tool wants
            // and not what a chart writes or what anyone filters a
            // gallery by. `short_name` gives "A", "Am", "D Dor": the way
            // the key appears on the page.
            key: chart
                .initial_key
                .as_ref()
                .map(keyflow_proto::Key::short_name),
            modulates: !chart.key_changes.is_empty(),
            // The parser puts the tempo on the chart, not in the
            // metadata — `metadata.tempo` stays `None` for a chart that
            // plainly says `72bpm`. Reading the metadata field here is
            // the obvious wrong answer and it fails silently, as an
            // empty tempo column in the gallery.
            tempo: chart.tempo.as_ref().map(|t| t.bpm.round() as u32),
            sections: chart
                .sections
                .iter()
                // `short_display` is the label a chart writes and the
                // engraver paints in the margin — `VS 2`, `CH`, `INST` —
                // rather than the prose `Verse 2`. A listing is showing
                // the shape of the chart, so it should read the way the
                // chart does.
                .map(|s| s.section.short_display().trim().to_string())
                .filter(|label: &String| !label.is_empty())
                .collect(),
            measures: chart
                .sections
                .iter()
                .map(|s| {
                    // A section's length is its longest track: chords,
                    // melody and lyrics run in parallel over the same
                    // bars, so summing tracks would count each bar once
                    // per track.
                    s.tracks.iter().map(|t| t.measures.len()).max().unwrap_or(0)
                })
                .sum(),
            year: m.year,
        }
    }

    /// Describe a chart from its source text.
    ///
    /// # Errors
    ///
    /// Returns the parse error if the source is not a valid chart.
    #[cfg(feature = "text")]
    pub fn parse(source: &str) -> Result<Self, crate::KeyflowSourceError> {
        use crate::IntoChart;
        Ok(Self::of(&source.into_chart()?))
    }

    /// A one-line credit for a card: the artist, else whoever else the
    /// chart names, else nothing.
    #[must_use]
    pub fn byline(&self) -> Option<String> {
        self.artist
            .clone()
            .or_else(|| (!self.credits.is_empty()).then(|| self.credits.join(", ")))
    }

    /// The words worth putting in a search index for this chart.
    ///
    /// Lowercased and deduplicated. Deliberately not the chord content:
    /// searching charts by the notes in them is a different feature with
    /// different indexing, and mixing the two makes "C" match everything.
    #[must_use]
    pub fn search_terms(&self) -> Vec<String> {
        let mut terms: Vec<String> = Vec::new();
        let mut push = |s: &str| {
            for word in s.split_whitespace() {
                let word = word
                    .trim_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase();
                if !word.is_empty() && !terms.contains(&word) {
                    terms.push(word);
                }
            }
        };

        if let Some(t) = &self.title {
            push(t);
        }
        if let Some(a) = &self.artist {
            push(a);
        }
        for c in &self.credits {
            push(c);
        }
        if let Some(k) = &self.key {
            push(k);
        }
        terms
    }
}

/// `Some(trimmed)` when there is something left after trimming.
fn non_empty(s: Option<&str>) -> Option<String> {
    s.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

#[cfg(all(test, feature = "text"))]
mod tests {
    use super::*;

    const MIDNIGHT: &str = "Midnight Dreams - Ada Sole\n72bpm 4/4 #A\n\nVS\nA D F#m E\n\nCH\nD E A F#m\n\nBR\nF#m E D A/C#\n";

    #[test]
    fn a_chart_describes_itself() {
        let s = ChartSummary::parse(MIDNIGHT).expect("parses");
        assert_eq!(s.title.as_deref(), Some("Midnight Dreams"));
        assert_eq!(s.artist.as_deref(), Some("Ada Sole"));
        assert_eq!(s.tempo, Some(72));
        assert_eq!(s.sections, ["VS", "CH", "BR"]);
        assert!(s.measures >= 3, "counted {} bars", s.measures);
        assert!(!s.modulates);
    }

    #[test]
    fn the_key_is_the_one_it_starts_in() {
        // A chart that modulates is still "in" the key it opens in —
        // that is what someone scanning a list matches against — and the
        // fact that it moves is a separate flag rather than a different
        // key.
        let s = ChartSummary::parse("Shifter\n4/4 #C\n\nVS\nC F\n\nCH\n#D\nD G\n").expect("parses");
        assert_eq!(s.key.as_deref(), Some("C"), "key was {:?}", s.key);
        assert!(s.modulates, "the key change was not noticed");
    }

    #[test]
    fn bars_are_counted_once_not_once_per_track() {
        // Chords, melody and lyrics run in PARALLEL over the same bars.
        // Summing tracks instead of taking the longest turns a four-bar
        // verse with a lyric line into an eight-bar one, and every
        // duration shown in the gallery doubles.
        let chords = ChartSummary::parse("4/4 #C\n\nVS\nC F G Am\n").expect("parses");
        let with_lyrics =
            ChartSummary::parse("4/4 #C\n\nVS\nC F G Am\n[lyrics]\nOne two three four\n")
                .expect("parses");
        assert_eq!(
            chords.measures, with_lyrics.measures,
            "adding a lyric track changed the bar count",
        );
    }

    #[test]
    fn the_same_source_always_describes_itself_the_same_way() {
        // The whole reason this lives in the language: the server
        // derives it on upload and the browser derives it again, and a
        // disagreement between them is a chart filed under the wrong key.
        assert_eq!(
            ChartSummary::parse(MIDNIGHT).unwrap(),
            ChartSummary::parse(MIDNIGHT).unwrap(),
        );
    }

    #[test]
    fn a_byline_prefers_the_artist_but_falls_back_to_the_credits() {
        let s = ChartSummary::parse(MIDNIGHT).unwrap();
        assert_eq!(s.byline().as_deref(), Some("Ada Sole"));

        let anon = ChartSummary::parse("4/4 #C\n\nVS\nC F\n").unwrap();
        assert_eq!(anon.byline(), None);
    }

    #[test]
    fn search_terms_are_words_not_punctuation() {
        let s = ChartSummary::parse(MIDNIGHT).unwrap();
        for term in s.search_terms() {
            assert!(
                term.chars().all(char::is_alphanumeric),
                "`{term}` is not a bare word",
            );
        }
        assert!(s.search_terms().contains(&"midnight".to_string()));
    }
}
