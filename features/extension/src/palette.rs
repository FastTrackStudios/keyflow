//! The palettes: the chart view's own sidebar, as MuseScore's palettes
//! panel. Sections of cells, each a thing a chart holds (a section, a key,
//! a meter, a mark), with the chart language's token for it. Inside the
//! view, not a dock of the app's: the view is docked into other apps as it
//! is, sidebar and all.
//!
//! A cell applies to the selected chord at once (`KEYFLOW_PALETTE_APPLY`,
//! [`cell`]): a quality sets its quality; an accent, a staccato or a push
//! goes on or off (a push not on a bar's first chord, which would sound in
//! the bar before). The other cells say they are not placed yet; with
//! nothing selected, the status line says so.

use base64::Engine as _;
use dioxus::prelude::*;

use crate::icons;

/// One cell: what it shows and the token it writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    /// Its text, under its glyph when it has one, else alone.
    pub label: &'static str,
    /// A SMuFL glyph of the chart's music font it shows.
    pub glyph: Option<char>,
    /// The chart language's token for it (`CH`, `#G`, `T6/8`, `>`).
    pub token: &'static str,
    /// What it is, in its tooltip.
    pub name: &'static str,
}

/// One palette: a titled section of cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub title: &'static str,
    pub items: &'static [Item],
}

const fn t(label: &'static str, token: &'static str, name: &'static str) -> Item {
    Item {
        label,
        glyph: None,
        token,
        name,
    }
}

const fn g(glyph: char, label: &'static str, token: &'static str, name: &'static str) -> Item {
    Item {
        label,
        glyph: Some(glyph),
        token,
        name,
    }
}

/// The palettes, in their order.
pub const PALETTES: &[Palette] = &[
    Palette {
        title: "Sections",
        items: &[
            t("Intro", "IN", "Intro"),
            t("Verse", "VS", "Verse"),
            t("Pre", "PRE", "Pre-chorus"),
            t("Chorus", "CH", "Chorus"),
            t("Bridge", "BR", "Bridge"),
            t("Inst", "INST", "Instrumental"),
            t("Solo", "SOLO", "Solo"),
            t("Inter", "INT", "Interlude"),
            t("Outro", "OUT", "Outro"),
            t("Count", "Count 2", "Count-in"),
            t("[ … ]", "[Name]", "A section of its own name"),
        ],
    },
    Palette {
        title: "Key signatures",
        items: &[
            t("C", "#C", "C major"),
            t("G", "#G", "G major"),
            t("D", "#D", "D major"),
            t("A", "#A", "A major"),
            t("E", "#E", "E major"),
            t("B", "#B", "B major"),
            t("F♯", "#F#", "F♯ major"),
            t("F", "bF", "F major"),
            t("B♭", "bBb", "B♭ major"),
            t("E♭", "bEb", "E♭ major"),
            t("A♭", "bAb", "A♭ major"),
            t("D♭", "bDb", "D♭ major"),
            t("G♭", "bGb", "G♭ major"),
            t("Am", "#Am", "A minor"),
            t("Em", "#Em", "E minor"),
            t("Bm", "#Bm", "B minor"),
            t("Dm", "bDm", "D minor"),
            t("Gm", "bGm", "G minor"),
            t("Cm", "bCm", "C minor"),
        ],
    },
    Palette {
        title: "Time signatures",
        items: &[
            t("2/4", "T2/4", "Two-four"),
            t("3/4", "T3/4", "Three-four"),
            t("4/4", "T4/4", "Four-four"),
            t("5/4", "T5/4", "Five-four"),
            t("6/4", "T6/4", "Six-four"),
            t("3/8", "T3/8", "Three-eight"),
            t("6/8", "T6/8", "Six-eight"),
            t("7/8", "T7/8", "Seven-eight"),
            t("9/8", "T9/8", "Nine-eight"),
            t("12/8", "T12/8", "Twelve-eight"),
        ],
    },
    Palette {
        title: "Tempo",
        items: &[
            g('\u{E1D5}', "= 60", "60bpm", "Quarter = 60"),
            g('\u{E1D5}', "= 80", "80bpm", "Quarter = 80"),
            g('\u{E1D5}', "= 100", "100bpm", "Quarter = 100"),
            g('\u{E1D5}', "= 120", "120bpm", "Quarter = 120"),
            g('\u{E1D5}', "= 140", "140bpm", "Quarter = 140"),
            g('\u{E1D5}', "= 160", "160bpm", "Quarter = 160"),
        ],
    },
    Palette {
        title: "Chord qualities",
        items: &[
            t("maj7", "maj7", "Major seventh"),
            t("m7", "m7", "Minor seventh"),
            t("7", "7", "Dominant seventh"),
            t("m7♭5", "m7b5", "Half-diminished"),
            t("dim7", "dim7", "Diminished seventh"),
            t("aug", "aug", "Augmented"),
            t("sus4", "sus4", "Suspended fourth"),
            t("7sus", "7sus", "Seventh, suspended"),
            t("add9", "add9", "Added ninth"),
            t("6", "6", "Sixth"),
            t("9", "9", "Ninth"),
            t("m9", "m9", "Minor ninth"),
        ],
    },
    Palette {
        title: "Articulations",
        items: &[
            g('\u{E4A0}', "", ">", "Accent"),
            g('\u{E4AC}', "", "\\marcato", "Marcato"),
            g('\u{E4A2}', "", ".", "Staccato"),
            g('\u{E4C0}', "", "\\fermata", "Fermata"),
            t("Push", "'", "Push (an eighth early)"),
            t("Push 16", "''", "Push (a sixteenth early)"),
            t("Stop", "\\stop", "Stop"),
            t("Tie", "~", "Tie into the next bar"),
        ],
    },
    Palette {
        title: "Repeats & barlines",
        items: &[
            t("|:", "|:", "Start repeat"),
            t(":|", ":|", "End repeat"),
            t("×2", "x2", "Play the line twice"),
            t("%", "%", "Repeat the previous bar"),
            g('\u{E047}', "", "segno", "Segno"),
            g('\u{E048}', "", "coda", "Coda"),
        ],
    },
    Palette {
        title: "Dynamics",
        items: &[
            g('\u{E52B}', "", "dyn pp", "Pianissimo"),
            g('\u{E520}', "", "dyn p", "Piano"),
            g('\u{E52C}', "", "dyn mp", "Mezzo-piano"),
            g('\u{E52D}', "", "dyn mf", "Mezzo-forte"),
            g('\u{E522}', "", "dyn f", "Forte"),
            g('\u{E52F}', "", "dyn ff", "Fortissimo"),
            t("<", "hairpin <", "Crescendo"),
            t(">", "hairpin >", "Diminuendo"),
        ],
    },
    Palette {
        title: "Text",
        items: &[
            t("Cue", "\"…\"", "A text cue below the staff"),
            t("Cue ↑", "^\"…\"", "A text cue above the staff"),
            t("@gtr", "@guitar \"…\"", "A guitar cue"),
            t("@keys", "@keys \"…\"", "A keys cue"),
            t("@drums", "@drums \"…\"", "A drums cue"),
            t("@vox", "@vox \"…\"", "A vocal cue"),
        ],
    },
];

/// What choosing the cell with `token` does to what is selected: a chord
/// quality cell sets the selected chord's quality, an accent, a staccato or
/// a push goes on it (or off). `Err` says why a cell does nothing yet; `None`
/// for a token no cell has.
#[must_use]
pub(crate) fn cell(token: &str) -> Option<Result<crate::Then, String>> {
    let (palette, item) = PALETTES
        .iter()
        .find_map(|p| p.items.iter().find(|i| i.token == token).map(|i| (p, i)))?;
    Some(match palette.title {
        "Chord qualities" => Ok(crate::Then::Quality(token.to_owned())),
        "Articulations" if keyflow::edit::chords::MARKS.contains(&token) => {
            Ok(crate::Then::Mark(token.to_owned()))
        }
        "Articulations" => Err(format!(
            "{}: written after the chord in the bar ({}), which the palette does not place yet",
            item.name, item.token
        )),
        _ => Err(format!(
            "{}: the palette does not place it on the chart yet; type it in the chart's text ({})",
            item.name, item.token
        )),
    })
}

/// Whether `item` matches the search `query` (its name, label or token).
#[must_use]
pub fn matches(item: &Item, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty()
        || [item.name, item.label, item.token]
            .iter()
            .any(|s| s.to_lowercase().contains(&q))
}

/// The glyphs' ink when the host gives no theme (a linked view's host
/// sends none): a grey that reads on the dark sheet and on a light one.
const FALLBACK_INK: &str = "#9aa0a6";

/// The theme's text colour, which the sheet's text is drawn in, as a
/// process view's host gives it; else [`FALLBACK_INK`].
fn use_ink() -> String {
    #[cfg(all(not(target_family = "wasm"), unix))]
    if let Some(ink) = architect_extensions::guest::use_theme()
        .as_ref()
        .and_then(|t| t.get("foreground").map(str::to_owned))
    {
        return ink;
    }
    FALLBACK_INK.to_owned()
}

/// Glyph `glyph` of the chart's music font as an image source, inked
/// `ink`. An image's `currentColor` has nothing to inherit and draws black,
/// dark on the dark sheet, so the colour is written into it.
fn glyph_src(glyph: char, ink: &str) -> Option<String> {
    let svg = icons::svg_inked(glyph, ink)?;
    Some(format!(
        "data:image/svg+xml;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(svg)
    ))
}

/// The sidebar's sheet, at app level (a `<style>` inside a pane does not
/// apply in Blitz). Its colours are the app's theme tokens.
const CSS: &str = r"
.kf-palettes { box-sizing: border-box; width: 248px; min-width: 248px; height: 100%;
  display: flex; flex-direction: column; border-right: 1px solid var(--border, #333);
  background: var(--background, #1e1f22); color: var(--foreground, #ddd); font-size: 12px; }
.kf-palettes-head { display: flex; flex-direction: column; gap: 6px; padding: 8px; }
.kf-palettes-title { font-weight: 600; font-size: 12px; }
.kf-palettes-search { box-sizing: border-box; width: 100%; padding: 5px 8px; border-radius: 4px;
  border: 1px solid var(--border, #333); background: var(--muted, #2b2d31);
  color: var(--foreground, #ddd); font-size: 12px; }
.kf-palettes-list { flex: 1 1 0; min-height: 0; overflow-y: auto; padding: 0 6px 8px; }
.kf-palette { margin-bottom: 6px; }
.kf-palette-head { display: flex; align-items: center; gap: 6px; padding: 5px 4px;
  cursor: pointer; font-weight: 600; }
.kf-palette-caret { width: 10px; opacity: 0.7; }
.kf-palette-grid { display: flex; flex-wrap: wrap; gap: 2px; padding: 2px 2px 4px; }
.kf-cell { box-sizing: border-box; width: 52px; height: 44px; display: flex; flex-direction: column;
  align-items: center; justify-content: center; gap: 1px; border-radius: 4px; cursor: pointer;
  background: var(--muted, #2b2d31); border: 1px solid transparent; }
.kf-cell:hover { border-color: var(--brand, #4fa3ff); }
.kf-cell-on { border-color: var(--brand, #4fa3ff); }
.kf-cell img { width: 22px; height: 22px; }
.kf-cell-label { font-size: 11px; white-space: nowrap; }
.kf-palettes-foot { padding: 6px 8px; border-top: 1px solid var(--border, #333);
  color: var(--muted-foreground, #999); font-size: 11px; min-height: 28px; }
";

/// The palettes sidebar.
#[component]
pub fn Palettes() -> Element {
    let mut query = use_signal(String::new);
    let mut closed = use_signal(std::collections::BTreeSet::<&'static str>::new);
    let mut chosen = use_signal(|| None::<Item>);
    // Drawn again in the theme's ink when it changes.
    let ink = use_ink();
    let mut drawn = use_signal(|| (String::new(), std::collections::BTreeMap::new()));
    if drawn.peek().0 != ink {
        let srcs = PALETTES
            .iter()
            .flat_map(|p| p.items)
            .filter_map(|i| i.glyph)
            .filter_map(|g| glyph_src(g, &ink).map(|s| (g, s)))
            .collect::<std::collections::BTreeMap<char, String>>();
        drawn.set((ink, srcs));
    }
    let srcs = drawn.peek().1.clone();
    let q = query();
    let foot = chosen().map_or_else(
        || "Choose a cell to apply it to the selected chord.".to_owned(),
        |i| format!("{} ({})", i.name, i.token),
    );
    rsx! {
        // Its sheet, in the document head: a process view's too, which its
        // host applies (architect#861).
        document::Style { {CSS} }
        div { class: "kf-palettes",
            div { class: "kf-palettes-head",
                div { class: "kf-palettes-title", "Palettes" }
                input {
                    class: "kf-palettes-search",
                    placeholder: "Search palettes",
                    value: "{q}",
                    oninput: move |e| query.set(e.value()),
                }
            }
            div { class: "kf-palettes-list",
                for palette in PALETTES.iter().filter(|p| p.items.iter().any(|i| matches(i, &q))) {
                    div { class: "kf-palette", key: "{palette.title}",
                        div {
                            class: "kf-palette-head",
                            onclick: move |_| {
                                let title = palette.title;
                                let mut c = closed.write();
                                if !c.remove(title) {
                                    c.insert(title);
                                }
                            },
                            span { class: "kf-palette-caret",
                                if closed().contains(palette.title) && q.trim().is_empty() { "▸" } else { "▾" }
                            }
                            "{palette.title}"
                        }
                        if !closed().contains(palette.title) || !q.trim().is_empty() {
                            div { class: "kf-palette-grid",
                                for item in palette.items.iter().filter(|i| matches(i, &q)) {
                                    div {
                                        class: if chosen() == Some(*item) { "kf-cell kf-cell-on" } else { "kf-cell" },
                                        title: "{item.name}",
                                        // Applied to what is selected at once,
                                        // through the action ⌘K and MCP run;
                                        // what it did, or why not, is the
                                        // chart's status line.
                                        onclick: move |_| {
                                            chosen.set(Some(*item));
                                            let token = item.token;
                                            spawn(async move {
                                                let _ = crate::dispatch(
                                                    "KEYFLOW_PALETTE_APPLY",
                                                    serde_json::json!({ "token": token }),
                                                )
                                                .await;
                                            });
                                        },
                                        if let Some(src) = item.glyph.and_then(|g| srcs.get(&g)) {
                                            img { src: "{src}" }
                                        }
                                        if !item.label.is_empty() {
                                            span { class: "kf-cell-label", "{item.label}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "kf-palettes-foot", "{foot}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cell_applies_or_says_why_not() {
        for item in PALETTES.iter().flat_map(|p| p.items) {
            match cell(item.token) {
                Some(Ok(_)) => {}
                Some(Err(why)) => assert!(why.contains(item.name), "{why}"),
                None => panic!("{}: no cell found by its token", item.token),
            }
        }
        assert!(matches!(cell("maj7"), Some(Ok(crate::Then::Quality(q))) if q == "maj7"));
        assert!(matches!(cell(">"), Some(Ok(crate::Then::Mark(m))) if m == ">"));
        assert!(matches!(cell("#G"), Some(Err(_))));
        assert!(cell("nonsense").is_none());
    }

    #[test]
    fn search_finds_a_cell_by_name_label_or_token() {
        let accent = PALETTES
            .iter()
            .flat_map(|p| p.items)
            .find(|i| i.name == "Accent")
            .unwrap();
        assert!(matches(accent, "acc"));
        assert!(matches(accent, ">"));
        assert!(matches(accent, ""));
        assert!(!matches(accent, "chorus"));
    }

    #[test]
    fn every_glyph_cell_draws_from_the_music_font() {
        for item in PALETTES.iter().flat_map(|p| p.items) {
            if let Some(g) = item.glyph {
                assert!(icons::svg(g).is_some(), "{}: no glyph {g:?}", item.name);
                // An `<img>`'s `currentColor` draws black: the palette's
                // glyphs are inked in a colour of their own.
                let svg = icons::svg_inked(g, FALLBACK_INK).unwrap();
                assert!(svg.contains(r##"fill="#9aa0a6""##), "{svg}");
                assert!(!svg.contains("currentColor"), "{svg}");
            }
        }
    }
}
