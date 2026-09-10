use super::*;

fn bars(source: &str) -> Vec<Item> {
    let doc = parse(source);
    doc.lines
        .into_iter()
        .find_map(|l| match l.kind {
            LineKind::Bars(items) => Some(items),
            _ => None,
        })
        .unwrap_or_default()
}

fn only_bar(source: &str) -> Bar {
    bars(source)
        .into_iter()
        .find_map(|i| match i {
            Item::Bar(bar) => Some(bar),
            _ => None,
        })
        .expect("expected a bar")
}

fn symbols(bar: &Bar) -> Vec<Option<&str>> {
    bar.cells.iter().map(|c| c.symbol.as_deref()).collect()
}

#[test]
fn one_chord_makes_a_bar() {
    let items = bars("A B C D");
    assert_eq!(items.len(), 4);
    for item in &items {
        let Item::Bar(bar) = item else {
            panic!("{item:?}")
        };
        assert_eq!(bar.cells.len(), 1);
    }
}

#[test]
fn glue_joins_cells_across_whitespace() {
    // chordsheet.com ignores whitespace outright, so these are the same bar.
    for source in ["Dm7_G7", "Dm7 _ G7", "Dm7_ G7", "Dm7 _G7"] {
        let bar = only_bar(source);
        assert_eq!(symbols(&bar), vec![Some("Dm7"), Some("G7")], "{source}");
    }
}

#[test]
fn parentheses_inside_a_chord_are_not_repeat_brackets() {
    let bar = only_bar("Eb(b5)");
    assert_eq!(symbols(&bar), vec![Some("Eb(b5)")]);

    // …but an unbalanced one is.
    let items = bars("(Gm7)x8");
    assert!(matches!(items[0], Item::RepeatOpen { .. }));
    assert!(matches!(items[2], Item::RepeatClose { times: Some(8), .. }));
}

#[test]
fn square_brackets_are_repeat_brackets_too() {
    let items = bars("[D,, F#m,,]");
    assert!(matches!(items.first(), Some(Item::RepeatOpen { .. })));
    assert!(matches!(items.last(), Some(Item::RepeatClose { .. })));
}

#[test]
fn repeat_counts_come_in_both_orders() {
    for source in ["(A B)x3", "(A B)3x", "(A B) x 3"] {
        let items = bars(source);
        assert!(
            matches!(items.last(), Some(Item::RepeatClose { times: Some(3), .. })),
            "{source}: {items:?}"
        );
    }
}

/// One comma, one stroke — and a chord written just before a comma labels
/// that comma's stroke rather than taking a slot of its own. `Gm,,,` prints
/// three strokes with `Gm` over the first, which is what the site's own PDF
/// of *Shine On You Crazy Diamond* does.
#[test]
fn a_comma_closes_a_slot() {
    let bar = only_bar("Gm,,,");
    assert_eq!(symbols(&bar), vec![Some("Gm"), None, None]);
    assert!(bar.cells.iter().all(|c| c.stroke));

    let bar = only_bar(",,B,");
    assert_eq!(symbols(&bar), vec![None, None, Some("B")]);

    let bar = only_bar(",,,,");
    assert_eq!(bar.cells.len(), 4);
}

/// The *Africa* intro riff — eight chords in one bar, no glue anywhere. This
/// is the shape that proves both rules at once: a comma closes a slot, and an
/// uppercase root with no separator starts the next chord.
#[test]
fn a_run_of_chords_and_commas_is_one_bar() {
    let bar = only_bar("A,A,A,A,A,G#,C#C#");
    assert_eq!(
        symbols(&bar),
        vec![
            Some("A"),
            Some("A"),
            Some("A"),
            Some("A"),
            Some("A"),
            Some("G#"),
            Some("C#"),
            Some("C#")
        ]
    );
}

/// No separator is needed between chords: chordsheet.com reads a new
/// uppercase root as the next chord in the same bar.
#[test]
fn an_uppercase_root_starts_the_next_chord() {
    assert_eq!(symbols(&only_bar("AD/A")), vec![Some("A"), Some("D/A")]);
    assert_eq!(symbols(&only_bar("D/F#G")), vec![Some("D/F#"), Some("G")]);
    assert_eq!(symbols(&only_bar("EbAb")), vec![Some("Eb"), Some("Ab")]);

    // Lowercase is quality and extension, never a boundary — and a lowercase
    // root is legal too, so splitting on it would cut every minor chord.
    for whole in ["Dm7b5", "Cmaj7", "Gsus4", "F#dim7", "a(b6)", "BbM7"] {
        assert_eq!(symbols(&only_bar(whole)), vec![Some(whole)], "{whole}");
    }
}

#[test]
fn annotations_and_modifiers_ride_on_the_cell() {
    let bar = only_bar(r#"<<D7#9"HOLD"^?"#);
    let cell = &bar.cells[0];
    assert_eq!(cell.symbol.as_deref(), Some("D7#9"));
    assert_eq!(cell.annotation.as_deref(), Some("HOLD"));
    assert_eq!(cell.push, Some(Push::Sixteenth));
    assert!(cell.tie);
    assert!(cell.optional);
}

#[test]
fn diamond_and_fermata_are_written_either_way() {
    assert!(only_bar("E7<>").cells[0].diamond);
    assert!(only_bar("E7 diamond").cells[0].diamond);
    assert!(only_bar("E7 fermata").cells[0].fermata);
}

#[test]
fn rests_and_blanks() {
    let bar = only_bar("r1");
    assert_eq!(bar.cells[0].rest, Some(Rest::Whole));

    let bar = only_bar("*_*_A");
    assert_eq!(symbols(&bar), vec![None, None, Some("A")]);
    assert!(bar.cells[0].is_blank());
}

#[test]
fn simile_and_reprint() {
    let items = bars("A B % %% %1 %4");
    let markers: Vec<(Option<u8>, Option<u32>)> = items
        .iter()
        .filter_map(|i| match i {
            Item::Bar(bar) if bar.is_marker() => Some((bar.simile, bar.reprint)),
            _ => None,
        })
        .collect();
    assert_eq!(
        markers,
        vec![
            (Some(1), None),
            (Some(2), None),
            (None, Some(1)),
            (None, Some(4))
        ]
    );
}

#[test]
fn endings_expand_ranges() {
    let items = bars("1.-3.+5. A");
    let Item::Ending { numbers, .. } = &items[0] else {
        panic!("{items:?}")
    };
    assert_eq!(numbers, &[1, 2, 3, 5]);
}

#[test]
fn per_bar_time_signatures() {
    let items = bars("2:4 A 3:4 B");
    assert!(matches!(
        items[0],
        Item::TimeSignature {
            numerator: 2,
            denominator: 4,
            ..
        }
    ));
    assert!(matches!(
        items[2],
        Item::TimeSignature {
            numerator: 3,
            denominator: 4,
            ..
        }
    ));
}

#[test]
fn navigation_markers_take_their_qualifiers() {
    let items = bars("D.S. al coda con rep.");
    let Item::Navigation { marker, .. } = &items[0] else {
        panic!("{items:?}")
    };
    assert_eq!(
        marker,
        &NavMarker::DalSegno {
            qualifier: Some("al coda con rep.".to_string())
        }
    );
}

#[test]
fn line_types_are_decided_by_the_first_character() {
    let doc = parse(
        "# note\n- text\n; literal  spaced\n: Boxed\n= Delimiter\n+ New Page\nRULER3\nTAB1\n",
    );
    let kinds: Vec<&LineKind> = doc.lines.iter().map(|l| &l.kind).collect();
    assert!(matches!(kinds[0], LineKind::Comment(_)));
    assert!(matches!(kinds[1], LineKind::Text { .. }));
    assert!(matches!(kinds[2], LineKind::Literal(_)));
    assert!(matches!(
        kinds[3],
        LineKind::SectionTitle {
            delimiter: false,
            ..
        }
    ));
    assert!(matches!(
        kinds[4],
        LineKind::SectionTitle {
            delimiter: true,
            ..
        }
    ));
    assert!(matches!(kinds[5], LineKind::PageBreak { .. }));
    assert!(matches!(
        kinds[6],
        LineKind::Ruler {
            style: RulerStyle::Solid,
            width: 3
        }
    ));
    assert!(matches!(kinds[7], LineKind::TabRow { lines: 1 }));
}

#[test]
fn a_double_space_pushes_text_to_the_next_column() {
    let doc = parse("- left  middle  right\n");
    let LineKind::Text { columns } = &doc.lines[0].kind else {
        panic!()
    };
    assert_eq!(columns, &["left", "middle", "right"]);
}

#[test]
fn floating_labels_attach_to_the_line_below() {
    let doc = parse("<- 5\n>= second time\nA B C D\n");
    assert_eq!(doc.lines.len(), 1);
    let line = &doc.lines[0];
    assert_eq!(line.floating.len(), 2);
    assert_eq!(line.floating[0].align, FloatAlign::Left);
    assert_eq!(line.floating[0].text, "5");
    assert_eq!(line.floating[1].align, FloatAlign::Right);
    assert!(line.floating[1].boxed);
}

#[test]
fn chord_diagram_definitions() {
    let doc = parse("// C7 x32310032410\n");
    let LineKind::DiagramDefs { slashes, defs } = &doc.lines[0].kind else {
        panic!("{:?}", doc.lines[0].kind)
    };
    assert_eq!(*slashes, 2);
    assert_eq!(defs[0].chord, "C7");
    assert_eq!(defs[0].frets, "x32310");
    assert_eq!(defs[0].fingering.as_deref(), Some("032410"));
}

#[test]
fn a_leading_dot_asks_for_diagrams_on_the_whole_line() {
    let items = bars(". D E7 A7 D");
    let count = items
        .iter()
        .filter(|i| matches!(i, Item::Bar(bar) if bar.show_diagram))
        .count();
    assert_eq!(count, 4);
}

/// The site's own "All Meta Characters" example, from
/// <https://www.chordsheet.com/manual/meta>. If this parses clean, the
/// documented language is covered.
#[test]
fn the_manual_meta_example_parses() {
    let source = "\
# A comment
= A
(A B C D)x2
A %1 C_D E
= B
(A B 1. C D 2. E F)
- simile mark one bar
A B C %
- simile mark two bars
A B %%
+ A second Page
(A B %2
C_D E_F %2 )x3
- 4 bars repeat
( E F G A
%4)
- empty bars
A * * D
";
    let doc = parse(source);
    assert_eq!(doc.section_titles().collect::<Vec<_>>(), vec!["A", "B"]);
    // Nothing fell on the floor: every non-blank line produced a line node.
    assert_eq!(doc.lines.len(), source.lines().count());
    assert!(doc.bars().count() > 20);
}
