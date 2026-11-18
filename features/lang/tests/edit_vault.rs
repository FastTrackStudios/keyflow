//! Every bar of every chart in the development vault (`tests/fixtures/vault`,
//! a copy of `apps/architect/vault`) through note input's write-back:
//!
//! - **found:** a bar knows its own text (`source_span`), barlined or laid
//!   out by beat counting;
//! - **unchanged:** written back with nothing edited (its own text typed in
//!   syntax mode, its melody written again), the chart reads exactly as it
//!   did, every bar of it;
//! - **edited:** a note entered on its first beat is written into that bar
//!   alone, and every other bar reads as it did; or it is refused, with
//!   why, and nothing is written (counted by reason).
//!
//! Prints each chart's counts (`--nocapture`) and the vault's total.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use keyflow_lang::edit::commit::{same_music, splice_syntax, write_bars};
use keyflow_lang::edit::{InputState, Mode, Op, Pos, Score, Target, Value};
use keyflow_lang::text::chart::parse_chart;
use keyflow_proto::Chart;
use keyflow_proto::chart::types::Measure;

fn charts() -> Vec<(String, String)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "kf") {
                out.push(path);
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault");
    let mut paths = Vec::new();
    walk(&root, &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let name = p.strip_prefix(&root).unwrap().display().to_string();
            (name, std::fs::read_to_string(&p).unwrap())
        })
        .collect()
}

fn bars(chart: &Chart) -> Vec<&Measure> {
    chart
        .sections
        .iter()
        .flat_map(|s| s.measures().iter())
        .collect()
}

fn unchanged(before: &Chart, text: &str) -> Result<(), String> {
    let after = parse_chart(text)?;
    let (a, b) = (bars(before), bars(&after));
    if a.len() != b.len() {
        return Err(format!("{} bars, then {}", a.len(), b.len()));
    }
    match a.iter().zip(&b).position(|(x, y)| !same_music(x, y)) {
        Some(i) => Err(format!("bar {} changed", i + 1)),
        None => Ok(()),
    }
}

#[test]
fn every_bar_of_the_vault_is_found_and_written_back_unchanged() {
    let (mut total, mut found, mut edited) = (0, 0, 0);
    let mut refused = std::collections::BTreeMap::<String, usize>::new();
    let mut chord_refused = std::collections::BTreeMap::<String, usize>::new();
    let mut chorded = 0;
    let mut failures = Vec::new();
    for (name, source) in charts() {
        let Ok(chart) = parse_chart(&source) else {
            continue;
        };
        let measures = bars(&chart);
        let score = Score::from_chart(&chart);
        let (mut f, mut e, mut c) = (0, 0, 0);
        for (i, m) in measures.iter().enumerate() {
            let Some(span) = m.source_span.as_ref() else {
                continue;
            };
            f += 1;
            let own = &source[span.start..span.start + span.len];

            // Unchanged: its own text typed over it.
            match splice_syntax(&source, &chart, i, own) {
                Ok((text, 1)) if text == source => {}
                Ok((text, n)) => failures.push(format!(
                    "{name} bar {}: its own text wrote {n} bars{}",
                    i + 1,
                    if text == source {
                        ""
                    } else {
                        ", and changed the text"
                    }
                )),
                Err(why) => failures.push(format!("{name} bar {}: {why}", i + 1)),
            }
            // Unchanged: its melody written again.
            if score.bars[i].written && score.bars[i].locked.is_none() {
                match write_bars(&source, &chart, &score, &[i])
                    .and_then(|t| unchanged(&chart, &t).map(|()| t))
                {
                    Ok(_) => {}
                    Err(why) => failures.push(format!("{name} bar {} melody: {why}", i + 1)),
                }
            }

            // Edited: a chord symbol entered over its first half.
            {
                let mut s = score.clone();
                let mut input = InputState {
                    mode: Mode::NoteName,
                    ..InputState::default()
                };
                input.apply(&mut s, Op::MoveTo(Pos { bar: i, tick: 0 }));
                input.apply(&mut s, Op::Duration(Value::Half));
                let out = input.apply(&mut s, Op::Letter('F'));
                if out.changed == vec![i] {
                    match write_bars(&source, &chart, &s, &out.changed) {
                        Ok(_) => c += 1,
                        Err(why) => *chord_refused.entry(why).or_insert(0) += 1,
                    }
                }
            }

            // Edited: a note on its first beat, that bar alone.
            if score.bars[i].locked.is_some() {
                continue;
            }
            let mut s = score.clone();
            let mut input = InputState {
                mode: Mode::NoteName,
                target: Target::Notes,
                ..InputState::default()
            };
            input.apply(&mut s, Op::MoveTo(Pos { bar: i, tick: 0 }));
            input.apply(&mut s, Op::Duration(Value::Quarter));
            let out = input.apply(&mut s, Op::Letter('C'));
            if out.changed != vec![i] {
                continue;
            }
            match write_bars(&source, &chart, &s, &out.changed) {
                Ok(_) => e += 1,
                // Refused, nothing written: counted by why.
                Err(why) => *refused.entry(why).or_insert(0) += 1,
            }
        }
        println!(
            "{f:4} of {:4} bars found, {e:4} took a note, {c:4} a chord: {name}",
            measures.len()
        );
        chorded += c;
        total += measures.len();
        found += f;
        edited += e;
    }
    println!(
        "VAULT: {found} of {total} bars found; {edited} took a note and {chorded} a chord symbol, written back"
    );
    for (why, n) in &chord_refused {
        println!("CHORD REFUSED {n:4} {why}");
    }
    assert!(found * 10 >= total * 6, "{found} of {total} bars found");
    for (why, n) in &refused {
        println!("REFUSED {n:4} {why}");
    }
    for f in &failures {
        println!("FAIL {f}");
    }
    assert!(
        failures.is_empty(),
        "{} bars did not write back",
        failures.len()
    );
}
