//! The guide, as a vault.
//!
//! `docs/guides/keyflow/*.md` are wiki notes: frontmatter, `[[wikilink]]`
//! cross-references, and Keyflow fences. `build.rs` compiles them in
//! verbatim; everything that makes them a *guide* rather than a pile of
//! files happens here and in `crate::routes::guide_page`.
//!
//! Two consumers of the same text:
//!
//! - the **editor**, read-only, renders a page — which is where wikilink
//!   navigation and the ```kf fence family come from. The site used to
//!   pre-render markdown to HTML in `build.rs`; that was reimplementing,
//!   less well, something the editor already does.
//! - the **graph** turns the whole set into a force-directed map of how
//!   the concepts connect. `view-knowledge-graph` is pure and FS-free by
//!   design, so the same builder runs in the browser.

use view_knowledge_graph::model::WikiGraph;
use view_knowledge_graph::parse::WikiFile;

/// One page of the guide.
#[derive(PartialEq, Eq)]
pub struct GuidePage {
    /// URL segment, from the filename. Also the wikilink target.
    pub slug: &'static str,
    /// Display title, from the frontmatter.
    pub title: &'static str,
    /// Sort key, from the frontmatter. Pages without one sort last.
    pub order: u32,
    /// The note verbatim, frontmatter included. What the graph reads.
    pub source: &'static str,
    /// The note without its frontmatter. What the editor renders — the
    /// editor turns frontmatter into an editable property table, which is
    /// right for a vault app and noise on a published guide.
    pub body: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/guide_generated.rs"));

/// Look up a page by its URL slug.
#[must_use]
pub fn page(slug: &str) -> Option<&'static GuidePage> {
    GUIDE_PAGES.iter().find(|p| p.slug == slug)
}

/// The first page, which `/guide` opens at.
#[must_use]
pub fn first_page() -> &'static GuidePage {
    GUIDE_PAGES
        .first()
        .expect("build.rs refuses to generate an empty guide")
}

/// The guide as the graph builder wants it.
fn wiki_files() -> Vec<WikiFile> {
    GUIDE_PAGES
        .iter()
        .map(|p| WikiFile {
            name: format!("{}.md", p.slug),
            // The route, not a filesystem path: the graph surfaces this
            // for click-to-open, and in a browser "open" means navigate.
            path: format!("/guide/{}", p.slug),
            content: p.source.to_string(),
        })
        .collect()
}

/// The guide's knowledge graph — nodes, relevance-weighted edges, and
/// detected communities.
///
/// Built rather than cached: it is a pure function of text baked into the
/// binary, and the builder is fast enough to run on demand. Callers hold
/// it in a `use_memo`.
#[must_use]
pub fn graph() -> WikiGraph {
    view_knowledge_graph::build_wiki_graph(&wiki_files())
}

/// Pages that link *to* `slug`.
///
/// The wiki's answer to "what leads here" — the reverse of the links on
/// the page itself, and the navigation a reader actually wants at the
/// bottom of a concept page.
#[must_use]
pub fn backlinks(graph: &WikiGraph, slug: &str) -> Vec<&'static GuidePage> {
    let mut out: Vec<&'static GuidePage> = graph
        .edges
        .iter()
        .filter(|e| e.target == slug && e.source != slug)
        .filter_map(|e| page(&e.source))
        .collect();
    out.sort_by_key(|p| p.order);
    out.dedup_by_key(|p| p.slug);
    out
}

/// The neighbourhood around one page — the local graph.
///
/// The whole-guide graph answers "how is this body of writing shaped".
/// Beside a page you want the other question: what does *this* concept
/// touch. So this is the node, everything one hop away, and the edges
/// among that set — Obsidian's local graph, and the same thing the right
/// rail of a guide page shows.
///
/// One hop, not two: at two hops this guide is almost fully connected,
/// and a map of everything is a map of nothing.
#[must_use]
pub fn local_graph(graph: &WikiGraph, slug: &str) -> WikiGraph {
    use std::collections::HashSet;

    let mut keep: HashSet<&str> = HashSet::new();
    keep.insert(slug);
    for e in &graph.edges {
        if e.source == slug {
            keep.insert(e.target.as_str());
        } else if e.target == slug {
            keep.insert(e.source.as_str());
        }
    }

    WikiGraph {
        nodes: graph
            .nodes
            .iter()
            .filter(|n| keep.contains(n.id.as_str()))
            .cloned()
            .collect(),
        edges: graph
            .edges
            .iter()
            .filter(|e| keep.contains(e.source.as_str()) && keep.contains(e.target.as_str()))
            .cloned()
            .collect(),
        // Community colouring is a property of the WHOLE graph; carried
        // through so a node keeps the colour it has in the full view and
        // the two reads agree.
        communities: graph.communities.clone(),
    }
}

/// Wikilink targets in a note, ignoring the `|label` half.
#[must_use]
pub fn wikilink_targets(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = source;
    while let Some(open) = rest.find("[[") {
        let after = &rest[open + 2..];
        let Some(close) = after.find("]]") else { break };
        let body = &after[..close];
        out.push(
            body.split_once('|')
                .map_or(body, |(t, _)| t)
                .trim()
                .to_owned(),
        );
        rest = &after[close + 2..];
    }
    out
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
    fn every_page_has_a_title_and_a_body() {
        for p in GUIDE_PAGES {
            assert!(!p.title.is_empty(), "guide page `{}` has no title", p.slug);
            assert!(p.source.len() > 100, "guide page `{}` looks empty", p.slug);
            assert!(
                !p.body.starts_with("---"),
                "guide page `{}` still carries its frontmatter into the editor",
                p.slug
            );
            assert!(p.body.len() > 50, "guide page `{}` has no body", p.slug);
        }
    }

    #[test]
    fn the_graph_has_a_node_per_page() {
        assert_eq!(
            graph().nodes.len(),
            GUIDE_PAGES.len(),
            "every guide page should be a node"
        );
    }

    #[test]
    fn the_graph_is_connected_by_wikilinks() {
        // The guide cross-references itself heavily — that web is the whole
        // reason it is a vault rather than a chapter list. If the edges
        // vanish, either the links broke or the builder stopped seeing them.
        let g = graph();
        assert!(
            g.edges.len() >= GUIDE_PAGES.len(),
            "only {} edges across {} pages — the wikilink web is missing",
            g.edges.len(),
            GUIDE_PAGES.len()
        );
    }

    #[test]
    fn every_wikilink_points_at_a_real_page() {
        // A dead `[[link]]` renders as a link that goes nowhere. The editor
        // will happily show it; this is what notices.
        for p in GUIDE_PAGES {
            for target in wikilink_targets(p.source) {
                assert!(
                    page(&target).is_some(),
                    "guide page `{}` links to `[[{target}]]`, which does not exist",
                    p.slug
                );
            }
        }
    }

    #[test]
    fn the_index_page_leads_somewhere() {
        let g = graph();
        let first = first_page();
        assert!(
            g.edges.iter().any(|e| e.source == first.slug),
            "the first page `{}` links to nothing — the guide has no entry path",
            first.slug
        );
    }

    #[test]
    fn the_local_graph_is_the_page_and_its_neighbours() {
        let g = graph();
        let local = local_graph(&g, "chords");

        assert!(
            local.nodes.iter().any(|n| n.id == "chords"),
            "the local graph must contain the page it is about"
        );
        assert!(
            local.nodes.len() > 1,
            "`chords` should have neighbours — it is a hub concept"
        );
        assert!(
            local.nodes.len() < g.nodes.len(),
            "a local graph of the whole guide is not local"
        );
    }

    #[test]
    fn the_local_graph_keeps_only_edges_between_kept_nodes() {
        let g = graph();
        let local = local_graph(&g, "chords");
        let ids: Vec<&str> = local.nodes.iter().map(|n| n.id.as_str()).collect();
        for e in &local.edges {
            assert!(
                ids.contains(&e.source.as_str()) && ids.contains(&e.target.as_str()),
                "edge {} -> {} dangles outside the local node set",
                e.source,
                e.target
            );
        }
    }

    #[test]
    fn an_unknown_slug_gives_an_empty_local_graph_not_a_panic() {
        let g = graph();
        assert!(local_graph(&g, "no-such-page").nodes.is_empty());
    }

    #[test]
    fn backlinks_are_the_reverse_of_links() {
        let g = graph();
        // `structure` is chapter one and is referenced from the index, so
        // something must point at it.
        let back = backlinks(&g, "structure");
        assert!(
            !back.is_empty(),
            "nothing links to `structure`, which the index should"
        );
        assert!(
            back.iter().all(|p| p.slug != "structure"),
            "a page must not be its own backlink"
        );
    }
}
