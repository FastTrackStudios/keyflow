#[test]
fn a_section_can_set_its_own_clef() {
    use keyflow_proto::chart::ChartClef;
    let text =
        "T - X\n#C 120bpm 4/4 treble\n\n[A] 1\n| C |\n\n[Bass] 1\n\\clef bass\n| C m{ C:34 } |\n";
    let chart = crate::text::chart::parse_chart(text).expect("parse");
    assert_eq!(chart.sections[0].clef, None);
    assert_eq!(chart.sections[1].clef, Some(ChartClef::Bass));
    assert_eq!(
        chart.sections[1].measures().len(),
        1,
        "the clef line is not a bar"
    );
}

#[test]
fn repeat_start_on_a_space_bar_stays_there() {
    use keyflow_proto::chart::notations::RepeatMark;
    let text = "T - X\n#A 120bpm 4/4\n\n[A] 4\n|: s1 | s1 | Bm | s1 :|\n";
    let chart = crate::text::chart::parse_chart(text).expect("parse");
    let starts: Vec<bool> = chart.sections[0]
        .measures()
        .iter()
        .map(|m| matches!(m.start_repeat, RepeatMark::Forward))
        .collect();
    assert_eq!(starts, vec![true, false, false, false]);
}

#[test]
fn volta_with_a_length() {
    let text = "T - X\n#A 120bpm 4/4\n\n[Hook] 4\n|: A | [1:2] Bm | E :| [2] A |\n";
    let chart = crate::text::chart::parse_chart(text).expect("parse");
    let ms = chart.sections[0].measures();
    let first = ms[1].volta_start.as_ref().expect("first ending");
    assert_eq!((first.numbers.clone(), first.length_measures), (vec![1], 2));
    let second = ms[3].volta_start.as_ref().expect("second ending");
    assert_eq!(
        (second.numbers.clone(), second.length_measures),
        (vec![2], 1)
    );
}

#[test]
fn inline_melody_keeps_ties() {
    let text = "T - X\n#A 120bpm 4/4 treble\n\n[Hook] 2\n| A m{ A:44 B:48~ ~B:48 C#:52 } | Bm m{ D:51 } |\n";
    let chart = crate::text::chart::parse_chart(text).expect("parse");
    let m = &chart.sections[0].measures()[0];
    let ties: Vec<(bool, bool)> = m.melodies[0]
        .notes
        .iter()
        .map(|n| (n.tie_start, n.tie_stop))
        .collect();
    assert_eq!(
        ties,
        vec![(false, false), (true, false), (false, true), (false, false)]
    );
}
