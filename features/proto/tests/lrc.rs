//! `.lrc` files as LRCLIB and the karaoke tools actually write them. The
//! module's own tests cover the stamp grammar; these read whole files.

use keyflow_proto::lrc;

const FIXTURE: &str = include_str!("fixtures/sample.lrc");

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

/// A realistic file: header tags, a blank instrumental marker, a chorus
/// written once with three stamps, and lines out of order.
#[test]
fn reads_a_whole_file_in_time_order() {
    let lrc = lrc::parse(FIXTURE);
    assert_eq!(lrc.title.as_deref(), Some("Sample Song"));
    assert_eq!(lrc.artist.as_deref(), Some("The Examples"));
    assert_eq!(lrc.album.as_deref(), Some("Fixtures"));

    let starts: Vec<f32> = lrc.lines.iter().map(|l| l.start).collect();
    assert!(
        starts.windows(2).all(|w| w[0] <= w[1]),
        "sorted: {starts:?}"
    );

    let chorus: Vec<f32> = lrc
        .lines
        .iter()
        .filter(|l| l.text == "This is the chorus")
        .map(|l| l.start)
        .collect();
    assert_eq!(chorus.len(), 3);
    assert!(approx(chorus[0], 30.0) && approx(chorus[1], 90.0) && approx(chorus[2], 150.5));

    // The instrumental gap survives as an empty line.
    assert!(
        lrc.lines
            .iter()
            .any(|l| l.text.is_empty() && approx(l.start, 45.0))
    );
    // Unknown tags (`length`, `by`, `re`) are not lyric lines.
    assert!(lrc.lines.iter().all(|l| !l.text.contains("Fixture Author")));
}

/// Times past an hour are minutes > 59, not an hours field.
#[test]
fn long_minutes() {
    let lrc = lrc::parse("[75:00.00]Encore\n");
    assert!(approx(lrc.lines[0].start, 4500.0));
}

/// Tag names are case-insensitive, and values keep their inner spaces.
#[test]
fn tags_are_case_insensitive() {
    let lrc = lrc::parse("[TI: Bye Bye Bye ]\n[Ar:NSYNC]\n");
    assert_eq!(lrc.title.as_deref(), Some("Bye Bye Bye"));
    assert_eq!(lrc.artist.as_deref(), Some("NSYNC"));
    assert!(lrc.lines.is_empty());
}

/// `[offset:]` is milliseconds, and a positive offset means the lyrics come
/// in *sooner* (the de-facto LRC convention). `Lrc::offset` documents that
/// it is "already applied to `lines`", so the line times move with it.
#[test]
fn offset_tag_shifts_the_lines() {
    let lrc = lrc::parse("[offset:+500]\n[00:10.00]Line\n");
    assert!(approx(lrc.offset, 0.5));
    assert!(
        approx(lrc.lines[0].start, 9.5),
        "got {}",
        lrc.lines[0].start
    );

    let late = lrc::parse("[offset:-250]\n[00:10.00]Line\n");
    assert!(approx(late.lines[0].start, 10.25));
}

/// Enhanced word times shift with the offset too, and text that comes
/// before the first word stamp does not invent a word.
#[test]
fn enhanced_lines_keep_their_words() {
    let lrc = lrc::parse("[00:05.00]<00:05.00>Hold <00:05.40>on <00:05.90>tight\n");
    let words: Vec<(&str, f32)> = lrc.lines[0]
        .words
        .iter()
        .map(|w| (w.text.as_str(), w.start))
        .collect();
    assert_eq!(words.len(), 3);
    assert_eq!(words[0].0, "Hold");
    assert!(approx(words[2].1, 5.9));
    assert_eq!(lrc.lines[0].text, "Hold on tight");

    let shifted = lrc::parse("[offset:1000]\n[00:05.00]<00:05.00>Hold <00:05.40>on\n");
    assert!(approx(shifted.lines[0].words[1].start, 4.4));
}
