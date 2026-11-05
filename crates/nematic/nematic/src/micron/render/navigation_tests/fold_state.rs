// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

#[test]
fn guide_structure_folds_replace_the_collapsible_diagnostic() {
    let doc = lower("guide-structure.mu");
    let open = fold(&doc, "Open fold");
    assert!(open.initially_open);
    assert_eq!(labels(&doc, open.extent.clone()), ["Open fold body."]);
    let closed = fold(&doc, "Closed fold");
    assert!(!closed.initially_open);
    assert_eq!(
        labels(&doc, closed.extent.clone()),
        ["Closed fold body.", ""],
        "Guide capture: the closed fold ends at >Table example"
    );
    assert!(
        doc.diagnostics.is_empty(),
        "the collapsible diagnostic was replaced, not joined: {:?}",
        doc.diagnostics
    );
    assert!(!matches!(doc.blocks[0], Block::Badge { .. }));
}

fn render_source(file: &str, source: &str) -> EngineDocument {
    MicronEngine::new()
        .render(&EngineInput::new(address(file), source))
        .unwrap()
}

pub(super) fn fold_index(doc: &EngineDocument, heading: &str) -> usize {
    doc.navigation
        .folds
        .iter()
        .position(|fold| label(doc, fold.heading) == heading)
        .unwrap_or_else(|| panic!("no fold {heading:?}"))
}

/// Labels of the blocks a reader sees under `state`.
fn visible(doc: &EngineDocument, state: &FoldState) -> Vec<String> {
    let hidden = state.hidden(&doc.navigation);
    (0..doc.blocks.len())
        .filter(|block| !hidden.iter().any(|range| range.contains(block)))
        .map(|block| label(doc, block))
        .collect()
}

#[test]
fn guide_structure_07c_06b_fold_state_survives_every_line_prefix() {
    for (file, heading, badge_moves_it) in [
        ("guide-structure.mu", "Closed fold", true),
        (
            "navigation/probe-nav-07c-section-exit-fold.mu",
            "Closed One A",
            true,
        ),
        (
            "navigation/probe-nav-06b-nested-collapsible.mu",
            "Inner Authored Closed",
            false,
        ),
    ] {
        let source = source_of(file);
        let complete = lower(file).navigation.fold_keys();
        let mut state = FoldState::default();
        let mut seen: Vec<FoldKey> = Vec::new();
        let mut headings = Vec::new();
        let mut prefix = String::new();
        for line in source.split_inclusive('\n') {
            prefix.push_str(line);
            let doc = render_source(file, &prefix);
            state.reconcile(&doc.navigation);
            let keys = doc.navigation.fold_keys();
            assert!(
                keys.iter().all(|key| complete.contains(key)),
                "{file}: a line prefix made no key the page lacks"
            );
            assert!(
                seen.iter().all(|key| keys.contains(key)),
                "{file}: an arrived key stays in every later prefix"
            );
            seen.clone_from(&keys);
            let Some(fold) = doc
                .navigation
                .folds
                .iter()
                .position(|fold| label(&doc, fold.heading) == heading)
            else {
                continue;
            };
            if headings.is_empty() {
                assert!(state.toggle(&doc.navigation, fold), "{file}: opened");
            }
            assert!(
                state.is_open(&doc.navigation, fold),
                "{file}: {heading} stays open at prefix {prefix:?}"
            );
            headings.push(doc.navigation.folds[fold].heading);
        }
        headings.dedup();
        assert_eq!(
            headings.len() > 1,
            badge_moves_it,
            "{file}: control, the badge moves the heading's block exactly where expected"
        );
    }
}

#[test]
fn probe_06b_an_edited_heading_drops_only_its_own_state() {
    let file = "navigation/probe-nav-06b-nested-collapsible.mu";
    let base = lower(file);
    let mut state = FoldState::default();
    for heading in ["Outer Closed", "Inner Authored Closed"] {
        state.toggle(&base.navigation, fold_index(&base, heading));
    }
    // Toggled twice: the stored override equals the authored state, so a
    // marker flip that dropped it is visible.
    for _ in 0..2 {
        state.toggle(&base.navigation, fold_index(&base, "Inner Authored Open"));
    }

    let inserted = render_source(file, &format!("Inserted line.\n{}", source_of(file)));
    let mut kept = state.clone();
    kept.reconcile(&inserted.navigation);
    assert_ne!(
        inserted.navigation.folds[0].source_line, base.navigation.folds[0].source_line,
        "control: the line identity moved"
    );
    for heading in [
        "Outer Closed",
        "Inner Authored Closed",
        "Inner Authored Open",
    ] {
        assert!(
            is_open(&inserted, &kept, heading),
            "an edit elsewhere keeps {heading}"
        );
    }

    for (from, to, heading) in [
        (
            "`->>Inner Authored Closed",
            "`->>Inner Authored Closed (edited)",
            "Inner Authored Closed (edited)",
        ),
        (
            "`->>Inner Authored Closed",
            "`->>`!Inner Authored Closed`!",
            "Inner Authored Closed",
        ),
        (
            "`+>>Inner Authored Open",
            "`->>Inner Authored Open",
            "Inner Authored Open",
        ),
    ] {
        let edited = render_source(file, &source_of(file).replace(from, to));
        let mut dropped = state.clone();
        dropped.reconcile(&edited.navigation);
        assert!(
            !is_open(&edited, &dropped, heading),
            "decision 11: {to:?} is back to its authored closed state"
        );
        assert!(
            is_open(&edited, &dropped, "Outer Closed"),
            "{to:?} leaves the outer heading's state"
        );
        dropped.reconcile(&base.navigation);
        assert_eq!(
            is_open(&base, &dropped, "Inner Authored Closed"),
            from == "`+>>Inner Authored Open",
            "reverting {to:?} does not restore a dropped override"
        );
    }
}

#[test]
fn probes_06b_06a_nested_state_survives_an_ancestor_close_and_reopen() {
    let doc = lower("navigation/probe-nav-06b-nested-collapsible.mu");
    let navigation = &doc.navigation;
    let mut state = FoldState::default();
    state.toggle(navigation, fold_index(&doc, "Outer Closed"));
    let shown = visible(&doc, &state);
    assert!(
        shown
            .iter()
            .any(|text| text.starts_with("MARKER INNER OPEN"))
    );
    assert!(
        !shown
            .iter()
            .any(|text| text.starts_with("MARKER INNER CLOSED")),
        "probe 06b: inner headings show their authored state"
    );

    state.toggle(navigation, fold_index(&doc, "Inner Authored Open"));
    state.toggle(navigation, fold_index(&doc, "Outer Closed"));
    assert!(!visible(&doc, &state).contains(&"Inner Authored Open".to_owned()));
    state.toggle(navigation, fold_index(&doc, "Outer Closed"));
    let shown = visible(&doc, &state);
    assert!(shown.contains(&"Inner Authored Open".to_owned()));
    assert!(
        !shown.iter().any(|text| text.starts_with("MARKER INNER")),
        "probe 06b: the reader's close survived the ancestor's close and reopen"
    );

    let doc = lower("navigation/probe-nav-06a-same-depth-collapsible.mu");
    let mut state = FoldState::default();
    state.toggle(&doc.navigation, fold_index(&doc, "Outer Closed"));
    assert_eq!(
        visible(&doc, &state)
            .into_iter()
            .skip_while(|text| text != "Outer Closed")
            .take_while(|text| text != "Sentinel After")
            .collect::<Vec<_>>(),
        [
            "Outer Closed",
            "MARKER OUTER: body directly under the closed outer heading.",
            "Inner Authored Open",
            "MARKER INNER OPEN: body under the inner heading authored open.",
            "Inner Authored Closed",
        ],
        "probe 06a control: opening the outer reveals only its own body"
    );
}
