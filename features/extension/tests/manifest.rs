//! The extension's manifest and what its code builds agree: a host refuses
//! a linked extension whose actions or views are not its manifest's.

use std::collections::BTreeSet;

use architect_extensions::linked::Builder;
use architect_extensions::manifest::Manifest;
use keyflow_extension::{Keyflow, MANIFEST};

fn manifest() -> Manifest {
    Manifest::parse(MANIFEST).expect("extension.toml parses")
}

#[test]
fn the_manifest_parses_and_is_keyflow() {
    let m = manifest();
    assert_eq!(m.id, "keyflow");
    // Action ids must carry the extension's prefix, or it does not load.
    let prefix = m.action_prefix();
    for action in &m.actions {
        assert!(
            action.id.starts_with(&prefix),
            "{} does not start with {prefix}",
            action.id
        );
    }
}

#[test]
fn what_it_builds_is_what_its_manifest_declares() {
    let m = manifest();
    let built = Builder::of(&Keyflow);
    let declared: BTreeSet<String> = m.actions.iter().map(|a| a.id.clone()).collect();
    let actions: BTreeSet<String> = built.action_ids().into_iter().collect();
    assert_eq!(actions, declared, "actions");
    let declared: BTreeSet<String> = m.views.iter().map(|v| v.id.clone()).collect();
    let views: BTreeSet<String> = built.view_ids().into_iter().collect();
    assert_eq!(views, declared, "views");
}

#[test]
fn a_kf_file_opens_in_the_chart_view() {
    let m = manifest();
    let kinds = &m.contributes.doc_kinds;
    let chart = kinds
        .iter()
        .find(|k| k.id == "keyflow.chart")
        .expect("the keyflow.chart document kind");
    assert_eq!(chart.view.as_deref(), Some("chart"));
    assert!(chart.files.iter().any(|f| f == "kf"));
    assert!(
        m.contributes
            .openers
            .iter()
            .any(|o| o.files.iter().any(|f| f == "kf") && o.view.as_deref() == Some("chart")),
        "an opener for .kf files into the chart view"
    );
}

#[test]
fn every_tool_is_a_toggle_on_the_chart_s_own_toolbar() {
    use keyflow_extension::tools::{Face, TOOLS};
    let m = manifest();
    let bar = m
        .contributes
        .toolbars
        .iter()
        .find(|t| t.id == "note-input")
        .expect("the note-input toolbar");
    assert_eq!(bar.at, "view:keyflow/chart", "on the view's own pane");
    let buttons: Vec<_> = bar.items.iter().filter(|i| !i.action.is_empty()).collect();
    assert_eq!(buttons.len(), TOOLS.len());
    for (tool, button) in TOOLS.iter().zip(&buttons) {
        assert_eq!(button.action, tool.action, "in the tools' order");
        let action = m.actions.iter().find(|a| a.id == tool.action).unwrap();
        // A tie is an action (T enters a tied note); every other tool stays on.
        let toggle = tool.action != "KEYFLOW_TIE";
        assert_eq!(
            action.toggle, toggle,
            "{} is a toggle: {toggle}",
            tool.action
        );
        match tool.face {
            Face::Glyph(g) => assert_eq!(button.image, keyflow_extension::icons::path_of(g)),
            Face::Icon(i) => assert_eq!(button.icon, i),
        }
    }
    // Every image a button names is a file of the package.
    let files: Vec<&str> = keyflow_extension::icons::files()
        .iter()
        .map(|(p, _)| *p)
        .collect();
    for button in &buttons {
        assert!(
            button.image.is_empty() || files.contains(&button.image.as_str()),
            "{}",
            button.image
        );
    }
}

#[test]
fn every_key_of_note_input_is_bound_in_the_chart_s_view_with_musescore_s_chord() {
    let m = manifest();
    let mut want: Vec<(String, String)> = keyflow_extension::input::default_keys()
        .flat_map(|(action, key)| {
            keyflow_extension::input::chords(key)
                .into_iter()
                .map(move |chord| (chord, action.to_owned()))
        })
        .collect();
    let mut have: Vec<(String, String)> = m
        .contributes
        .key_bindings
        .iter()
        .map(|k| {
            assert_eq!(k.when, "view:chart", "{} {}", k.chord, k.action);
            (k.chord.clone(), k.action.clone())
        })
        .collect();
    want.sort();
    have.sort();
    assert_eq!(have, want, "the [[keys]] are input.rs's, ⌘ chords ⌃ too");
    for (_, action) in &want {
        assert!(
            m.actions.iter().any(|a| a.id == *action),
            "{action} is declared"
        );
    }
}
