//! The guide, compiled from `docs/guides/keyflow/*.md` by `build.rs`.
//!
//! The markdown is rendered to HTML at build time, so nothing here parses
//! anything. What is left at runtime is the part that has to be live: the
//! `kf+` fences, which the guides mark as "engrave this", get a real chart
//! rendered from the same engine the editor uses.

/// One piece of a guide page.
///
/// Prose and Keyflow alternate, because a Keyflow block is not text the
/// browser can lay out — it needs the chart renderer.
#[derive(PartialEq, Eq)]
pub enum Block {
    /// Prose, already rendered to HTML.
    Html(&'static str),
    /// A Keyflow fence.
    Keyflow {
        /// The chart source, verbatim from the guide.
        source: &'static str,
        /// `true` for ` ```kf+ ` — a real chart fragment, to be engraved.
        /// `false` for ` ```kf- ` — a syntax illustration, shown as source.
        engrave: bool,
    },
}

/// One page of the guide.
#[derive(PartialEq, Eq)]
pub struct GuidePage {
    /// URL segment, from the filename.
    pub slug: &'static str,
    /// Display title, from the frontmatter.
    pub title: &'static str,
    /// Sort key, from the frontmatter. Pages without one sort last.
    pub order: u32,
    /// The page body, in document order.
    pub blocks: &'static [Block],
}

include!(concat!(env!("OUT_DIR"), "/guide_generated.rs"));

/// Look up a page by its URL slug.
#[must_use]
pub fn page(slug: &str) -> Option<&'static GuidePage> {
    GUIDE_PAGES.iter().find(|p| p.slug == slug)
}

/// The first page, which `/guide` redirects to.
#[must_use]
pub fn first_page() -> &'static GuidePage {
    GUIDE_PAGES
        .first()
        .expect("build.rs refuses to generate an empty guide")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_guide_is_not_empty() {
        assert!(
            GUIDE_PAGES.len() >= 5,
            "expected the full guide to compile in"
        );
    }

    #[test]
    fn pages_are_ordered_by_frontmatter() {
        let orders: Vec<u32> = GUIDE_PAGES.iter().map(|p| p.order).collect();
        let mut sorted = orders.clone();
        sorted.sort_unstable();
        assert_eq!(orders, sorted, "guide pages must arrive in reading order");
    }

    #[test]
    fn slugs_are_unique() {
        let mut slugs: Vec<&str> = GUIDE_PAGES.iter().map(|p| p.slug).collect();
        slugs.sort_unstable();
        let before = slugs.len();
        slugs.dedup();
        assert_eq!(before, slugs.len(), "two guide pages share a slug");
    }

    #[test]
    fn every_page_has_content() {
        for p in GUIDE_PAGES {
            assert!(!p.blocks.is_empty(), "guide page `{}` is empty", p.slug);
            assert!(!p.title.is_empty(), "guide page `{}` has no title", p.slug);
        }
    }

    #[test]
    fn engraved_fences_parse_as_charts() {
        // A `kf+` fence is the guide asserting "this is real Keyflow". If one
        // stops parsing, the guide is teaching something the parser rejects,
        // and this test is the thing that notices.
        for p in GUIDE_PAGES {
            for block in p.blocks {
                if let Block::Keyflow {
                    source,
                    engrave: true,
                } = block
                {
                    assert!(
                        keyflow::text::chart::parse_chart(source).is_ok(),
                        "guide page `{}` engraves a fence that does not parse:\n{source}",
                        p.slug
                    );
                }
            }
        }
    }

    #[test]
    fn wiki_links_became_real_links() {
        // The guides are vault notes as well as site pages, so they
        // cross-link as `[[slug|Label]]`. build.rs rewrites those; if it
        // stops, every cross-reference in the guide renders as literal
        // brackets — which is exactly how this shipped the first time.
        for p in GUIDE_PAGES {
            for block in p.blocks {
                if let Block::Html(html) = block {
                    assert!(
                        !html.contains("[["),
                        "guide page `{}` still has a raw wiki link",
                        p.slug
                    );
                }
            }
        }
    }

    #[test]
    fn cross_references_point_at_guide_routes() {
        let linked = GUIDE_PAGES
            .iter()
            .flat_map(|p| p.blocks)
            .filter_map(|b| match b {
                Block::Html(h) => Some(*h),
                Block::Keyflow { .. } => None,
            })
            .filter(|h| h.contains("href=\"/guide/"))
            .count();
        assert!(
            linked > 0,
            "no page cross-links another — did the rewrite run?"
        );
    }

    #[test]
    fn lookup_matches_the_table() {
        for p in GUIDE_PAGES {
            assert_eq!(page(p.slug).map(|f| f.slug), Some(p.slug));
        }
        assert!(page("no-such-page").is_none());
    }
}
