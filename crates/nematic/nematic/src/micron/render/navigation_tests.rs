// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

//! N1 lowering: the navigation table and in-page links, asserted by label
//! against the committed stock-client probe pages.

use std::ops::Range;

use super::*;
use inker::{inline_text, link_statements};

const NODE: &str = "923706ddc70d389bd3719258c41f6592";

macro_rules! pages {
    ($($file:literal),* $(,)?) => {
        &[$(($file, include_str!(concat!(
            "../../../tests/fixtures/micron/nomadnet-1.4.2/",
            $file
        )))),*]
    };
}

const PAGES: &[(&str, &str)] = pages![
    "guide-structure.mu",
    "navigation/index.mu",
    "navigation/probe-nav-01-duplicate-heading.mu",
    "navigation/probe-nav-02a-heading-first.mu",
    "navigation/probe-nav-02b-anchor-first.mu",
    "navigation/probe-nav-03-missing-anchor.mu",
    "navigation/probe-nav-04-next-heading-tail.mu",
    "navigation/probe-nav-05-closed-target.mu",
    "navigation/probe-nav-06a-same-depth-collapsible.mu",
    "navigation/probe-nav-06b-nested-collapsible.mu",
    "navigation/probe-nav-07a-section-exit.mu",
    "navigation/probe-nav-07b-section-exit-deep.mu",
    "navigation/probe-nav-07c-section-exit-fold.mu",
    "navigation/probe-nav-08-toggle-keys.mu",
    "navigation/probe-nav-09-control.mu",
    "navigation/probe-nav-09-transport.mu",
    "navigation/probe-nav-10-location-back.mu",
    "navigation/probe-nav-11a-unnamed-fold.mu",
    "navigation/probe-nav-11b-next-heading-unnamed.mu",
    "navigation/probe-nav-11c-unnamed-fold-depths.mu",
    "navigation/probe-nav-12-double-less-than-fold.mu",
    "navigation/probe-nav-13-less-than-text-fold.mu",
    "navigation/probe-nav-14-source.mu",
    "navigation/probe-nav-14a-target.mu",
    "navigation/probe-nav-14b-target-missing.mu",
    "navigation/probe-nav-14c-target-closed.mu",
    "navigation/probe-nav-14d-target-full-address.mu",
    "navigation/probe-nav-15a-heading-anchor-same.mu",
    "navigation/probe-nav-15b-heading-anchor-other.mu",
    "navigation/probe-nav-16-duplicate-explicit.mu",
    "navigation/probe-nav-17-nested-closed-target.mu",
];

/// Diagnostics N1 replaced with typed facts or split.
const RETIRED: [&str; 3] = [
    "Micron collapsible section retained in syntax",
    "Micron anchor retained in syntax",
    "Micron request or anchor link retained in syntax",
];

fn address(file: &str) -> String {
    format!("{NODE}:/page/{}", file.rsplit('/').next().unwrap())
}

fn lower(file: &str) -> EngineDocument {
    let (_, source) = PAGES.iter().find(|(name, _)| *name == file).unwrap();
    MicronEngine::new()
        .render(&EngineInput::new(address(file), *source))
        .unwrap()
}

fn unwrap(block: &Block) -> &Block {
    match block {
        Block::Presented { block, .. } => unwrap(block),
        other => other,
    }
}

fn label(doc: &EngineDocument, index: usize) -> String {
    match unwrap(&doc.blocks[index]) {
        Block::Heading { spans, .. } | Block::Paragraph { spans } => inline_text(spans),
        Block::Preformatted { text } | Block::Badge { text } => text.clone(),
        other => format!("{other:?}"),
    }
}

fn labels(doc: &EngineDocument, range: Range<usize>) -> Vec<String> {
    range.map(|index| label(doc, index)).collect()
}

fn diagnosed(doc: &EngineDocument, text: &str) -> bool {
    doc.diagnostics.iter().any(|diagnostic| {
        matches!(diagnostic, DocumentDiagnostic::UnsupportedConstruct(message) if message.contains(text))
    })
}

fn span_lists(block: &Block) -> Vec<&Vec<InlineSpan>> {
    match unwrap(block) {
        Block::Heading { spans, .. } | Block::Paragraph { spans } => vec![spans],
        Block::Table { header, rows, .. } => header.iter().chain(rows.iter().flatten()).collect(),
        _ => Vec::new(),
    }
}

/// (link label, target) for every in-page link, in block order.
fn in_page(doc: &EngineDocument) -> Vec<(String, InPageTarget)> {
    doc.blocks
        .iter()
        .flat_map(span_lists)
        .flatten()
        .filter_map(|span| match span {
            InlineSpan::InPage { target, spans } => Some((inline_text(spans), target.clone())),
            _ => None,
        })
        .collect()
}

/// (target block label, fragment) of the in-page link labelled `link`.
fn landing(doc: &EngineDocument, link: &str) -> (Option<String>, Option<String>) {
    let (_, target) = in_page(doc)
        .into_iter()
        .find(|(label, _)| label == link)
        .unwrap_or_else(|| panic!("no in-page link {link:?}"));
    (target.block.map(|block| label(doc, block)), target.fragment)
}

fn fold<'a>(doc: &'a EngineDocument, heading: &str) -> &'a DocumentFold {
    doc.navigation
        .folds
        .iter()
        .find(|fold| label(doc, fold.heading) == heading)
        .unwrap_or_else(|| panic!("no fold {heading:?}"))
}

fn folds_around(doc: &EngineDocument, block: usize) -> Vec<String> {
    doc.navigation
        .folds_containing(block)
        .map(|fold| label(doc, fold.heading))
        .collect()
}

fn position(doc: &EngineDocument, text: &str) -> usize {
    (0..doc.blocks.len())
        .find(|&index| label(doc, index) == text)
        .unwrap_or_else(|| panic!("no block {text:?}"))
}

#[test]
fn every_probe_page_keeps_a_consistent_table_and_no_retired_diagnostic() {
    for (file, source) in PAGES {
        let doc = lower(file);
        let navigation = &doc.navigation;
        let len = doc.blocks.len();
        assert!(navigation.is_current(&doc.blocks), "{file}: block count");
        let lines = syntax::parse(source).lines;
        for fold in &navigation.folds {
            assert!(
                matches!(unwrap(&doc.blocks[fold.heading]), Block::Heading { .. }),
                "{file}: fold at line {} is a heading block",
                fold.source_line
            );
            assert_eq!(fold.extent.start, fold.heading + 1, "{file}");
            assert!(fold.extent.end <= len, "{file}");
            for other in &navigation.folds {
                let (a, b) = (&fold.extent, &other.extent);
                assert!(
                    a.end <= b.start
                        || b.end <= a.start
                        || (a.start <= b.start && b.end <= a.end)
                        || (b.start <= a.start && a.end <= b.end),
                    "{file}: extents {a:?} and {b:?} nest"
                );
            }
        }
        for anchor in &navigation.anchors {
            let Some(block) = anchor.block else { continue };
            assert!(block < len, "{file}: anchor {} in range", anchor.name);
            if matches!(lines[anchor.source_line].kind, LineKind::Heading { .. }) {
                assert!(
                    matches!(unwrap(&doc.blocks[block]), Block::Heading { .. }),
                    "{file}: heading anchor {} lands on its heading",
                    anchor.name
                );
            }
        }
        for (link, target) in in_page(&doc) {
            assert!(
                target.block.is_none_or(|block| block < len),
                "{file}: {link} in range"
            );
            assert_eq!(
                target
                    .fragment
                    .as_deref()
                    .and_then(|name| navigation.resolve(name)),
                target.block,
                "{file}: {link} has a fragment exactly when it has a target, and it resolves back"
            );
        }
        for retired in RETIRED {
            assert!(!diagnosed(&doc, retired), "{file}: {retired}");
        }
    }
}

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

#[test]
fn probes_01_02_first_declaration_wins_by_label() {
    let doc = lower("navigation/probe-nav-01-duplicate-heading.mu");
    let (target, fragment) = landing(&doc, "jump to nav probe one");
    assert_eq!(target.as_deref(), Some("Nav Probe One"), "probe 01");
    assert_eq!(fragment.as_deref(), Some("nav-probe-one"), "probe 01");
    let block = doc.navigation.resolve("nav-probe-one").unwrap();
    assert_eq!(
        label(&doc, block + 1),
        "MARKER FIRST: body under the first Nav Probe One heading.",
        "probe 01: the first heading, not the second"
    );

    let doc = lower("navigation/probe-nav-02a-heading-first.mu");
    assert_eq!(
        landing(&doc, "jump to same name").0.as_deref(),
        Some("Same Name"),
        "probe 02a: the heading declared first wins"
    );

    let doc = lower("navigation/probe-nav-02b-anchor-first.mu");
    assert_eq!(
        landing(&doc, "jump to other name"),
        (
            Some("MARKER EXPLICIT: line bound by the earlier explicit anchor.".into()),
            Some("other-name".into())
        ),
        "probe 02b: the anchor-only line lands on the block after it"
    );
}

#[test]
fn probes_03_04_missing_targets_are_silent_no_ops() {
    let doc = lower("navigation/probe-nav-03-missing-anchor.mu");
    assert_eq!(
        landing(&doc, "jump to a missing anchor"),
        (None, None),
        "probe 03: a missing anchor is inert"
    );
    assert_eq!(
        landing(&doc, "jump to a present anchor").0.as_deref(),
        Some("MARKER CONTROL: line bound by the present-control anchor."),
        "probe 03: the positive control lands"
    );
    assert!(
        doc.diagnostics.is_empty(),
        "probe 03: no diagnostic for a missing in-page anchor: {:?}",
        doc.diagnostics
    );

    let doc = lower("navigation/probe-nav-04-next-heading-tail.mu");
    assert_eq!(
        landing(&doc, "next heading from the top"),
        (Some("First Heading".into()), Some("first-heading".into())),
        "probe 04 / decision 6"
    );
    assert_eq!(
        landing(&doc, "next heading from the tail"),
        (None, None),
        "probe 04: # below the last heading is inert"
    );
    assert!(
        doc.diagnostics.is_empty(),
        "probe 04: {:?}",
        doc.diagnostics
    );
}

#[test]
fn probes_05_17_targets_know_their_closed_ancestors() {
    let doc = lower("navigation/probe-nav-05-closed-target.mu");
    for link in [
        "jump to the hidden heading",
        "jump to the hidden explicit anchor",
    ] {
        let (_, target) = in_page(&doc).into_iter().find(|(l, _)| l == link).unwrap();
        assert_eq!(
            folds_around(&doc, target.block.unwrap()),
            ["Closed Outer"],
            "probe 05: {link}"
        );
    }
    assert!(
        folds_around(&doc, position(&doc, "Sentinel After")).is_empty(),
        "probe 05: the sentinel is outside the fold"
    );

    let doc = lower("navigation/probe-nav-17-nested-closed-target.mu");
    let (target, _) = landing(&doc, "jump into two closed sections");
    assert_eq!(
        target.as_deref(),
        Some("MARKER NESTED DEEP TARGET: bound inside both closed folds.")
    );
    let block = doc.navigation.resolve("nested-deep").unwrap();
    assert_eq!(
        folds_around(&doc, block),
        ["Outer Closed", "Inner Closed"],
        "C1b probe 17: both closed folds contain the target"
    );
}

#[test]
fn probe_06b_nested_folds_keep_authored_state() {
    let doc = lower("navigation/probe-nav-06b-nested-collapsible.mu");
    let outer = fold(&doc, "Outer Closed");
    let open = fold(&doc, "Inner Authored Open");
    let closed = fold(&doc, "Inner Authored Closed");
    assert!(outer.extent.contains(&open.heading) && outer.extent.contains(&closed.heading));
    assert_eq!(
        (
            outer.initially_open,
            open.initially_open,
            closed.initially_open
        ),
        (false, true, false),
        "probe 06b"
    );
    assert!(
        doc.diagnostics.is_empty(),
        "probe 06b: {:?}",
        doc.diagnostics
    );
}

#[test]
fn probes_07c_12_13_less_than_lines_end_folds_and_keep_source_with_a_badge() {
    let doc = lower("navigation/probe-nav-07c-section-exit-fold.mu");
    assert!(
        matches!(doc.blocks[0], Block::Badge { .. }),
        "decision 5: the < diagnostic brings the badge"
    );
    assert!(diagnosed(&doc, "Micron leading < line retained as source"));
    for (heading, body, after) in [
        (
            "Closed One A",
            vec!["MARKER A BODY: inside the depth-one fold."],
            "MARKER A AFTER <: left depth one inside a closed fold.",
        ),
        (
            "Closed One B",
            vec![
                "MARKER B BODY: inside the depth-one fold.",
                "Plain Two B",
                "MARKER B DEPTH TWO BODY.",
            ],
            "MARKER B AFTER <: left depth two inside a closed depth-one fold.",
        ),
    ] {
        let extent = fold(&doc, heading).extent.clone();
        assert_eq!(labels(&doc, extent.clone()), body, "probe 07c: {heading}");
        assert_eq!(
            label(&doc, extent.end),
            "<",
            "probe 07c: the < source is kept"
        );
        assert!(folds_around(&doc, extent.end).is_empty());
        assert!(
            folds_around(&doc, position(&doc, after)).is_empty(),
            "probe 07c: {after}"
        );
    }

    let doc = lower("navigation/probe-nav-12-double-less-than-fold.mu");
    assert!(
        fold(&doc, "Closed Twelve A").extent.is_empty(),
        "C1b probe 12"
    );
    assert!(
        fold(&doc, "Closed Twelve B").extent.is_empty(),
        "C1b probe 12"
    );
    assert_eq!(
        labels(&doc, fold(&doc, "Closed Twelve C").extent.clone()),
        ["MARKER 12C BODY: inside the fold."],
        "C1b probe 12"
    );
    assert!(
        folds_around(
            &doc,
            position(
                &doc,
                "MARKER 12A AFTER: first line after << inside the fold."
            )
        )
        .is_empty()
    );

    let doc = lower("navigation/probe-nav-13-less-than-text-fold.mu");
    let extent = fold(&doc, "Closed Thirteen").extent.clone();
    assert_eq!(
        labels(&doc, extent.clone()),
        ["MARKER 13 BODY: inside the fold."]
    );
    assert_eq!(
        label(&doc, extent.end),
        "< TEXT ON THE LESS-THAN LINE",
        "C1b probe 13: < text is outside the fold, source kept"
    );
}

#[test]
fn probe_11_unnamed_sections_in_block_space() {
    let doc = lower("navigation/probe-nav-11a-unnamed-fold.mu");
    assert_eq!(
        labels(&doc, fold(&doc, "Closed Two C").extent.clone()),
        ["MARKER C BODY: inside the depth-two fold."],
        "C1b probe 11a: the shallower bare > ends the fold"
    );
    assert_eq!(
        labels(&doc, fold(&doc, "Closed One A").extent.clone()),
        [
            "MARKER A BODY: inside the depth-one fold.",
            "MARKER A AFTER UNNAMED: first line after a bare >>>> inside the fold.",
            "MARKER A SECOND AFTER.",
        ],
        "C1b probe 11a: a deeper bare run stays inside"
    );

    let doc = lower("navigation/probe-nav-11b-next-heading-unnamed.mu");
    assert_eq!(
        landing(&doc, "next heading from the top"),
        (
            Some("Named After Unnamed".into()),
            Some("named-after-unnamed".into())
        ),
        "C1b probe 11b: # skips the unnamed section"
    );
}

#[test]
fn probes_15_16_explicit_anchor_precedence_by_label() {
    let doc = lower("navigation/probe-nav-15a-heading-anchor-same.mu");
    assert_eq!(
        landing(&doc, "jump to setup").0.as_deref(),
        Some("Setup"),
        "C1b probe 15a: the heading row, not the body row"
    );
    let doc = lower("navigation/probe-nav-15b-heading-anchor-other.mu");
    assert_eq!(landing(&doc, "jump to setup").0.as_deref(), Some("Setup"));
    assert_eq!(
        landing(&doc, "jump to install"),
        (
            Some("MARKER 15B BODY: first row after the explicit install anchor.".into()),
            Some("install".into())
        ),
        "C1b probe 15b"
    );
    let doc = lower("navigation/probe-nav-16-duplicate-explicit.mu");
    assert_eq!(
        landing(&doc, "jump to dup").0.as_deref(),
        Some("MARKER DUP FIRST: row after the first dup declaration."),
        "C1b probe 16"
    );
}

#[test]
fn probe_09_in_page_links_never_reach_the_network_path() {
    let file = "navigation/probe-nav-09-transport.mu";
    let doc = lower(file);
    assert_eq!(
        doc.outgoing_links(),
        [format!("{NODE}:/page/probe-nav-09-control.mu")],
        "probe 09: only the control page is an outgoing link"
    );
    assert!(link_statements(&doc).is_empty());
    assert_eq!(resolve_target(&address(file), "#transport-target"), None);
    assert_eq!(
        landing(&doc, "in-page anchor jump").0.as_deref(),
        Some("MARKER TRANSPORT TARGET: reached without leaving this page."),
        "probe 09"
    );

    let doc = lower("navigation/probe-nav-10-location-back.mu");
    assert_eq!(
        landing(&doc, "anchor jump on this page"),
        (
            Some("Location Target".into()),
            Some("location-target".into())
        ),
        "probe 10 / decision 1: the fragment for the address"
    );
    let doc = lower("navigation/probe-nav-14a-target.mu");
    assert_eq!(
        landing(&doc, "in-page control").0.as_deref(),
        Some("Cross Target"),
        "C1b probe 14a: the in-page control"
    );
}

#[test]
fn request_and_cross_page_anchor_links_keep_their_diagnostics() {
    let doc = lower("navigation/probe-nav-14-source.mu");
    assert!(in_page(&doc).is_empty() && doc.outgoing_links().is_empty());
    assert!(
        diagnosed(&doc, "Micron cross-page anchor link retained in syntax"),
        "probe 14: anchor= stays text plus a diagnostic until A1"
    );
    assert!(!diagnosed(&doc, "Micron request link"));

    let doc = MicronEngine::new()
        .render(&EngineInput::new(
            address("action.mu"),
            "\u{60}[Send\u{60}:/page/action.mu\u{60}name|action=view]\n\u{60}[Next\u{60}#]\n>After\n\u{60}[Past\u{60}#]",
        ))
        .unwrap();
    assert!(diagnosed(&doc, "Micron request link retained in syntax"));
    assert!(!diagnosed(&doc, "cross-page anchor"));
    assert!(matches!(doc.blocks[0], Block::Badge { .. }));
    assert_eq!(
        landing(&doc, "Next"),
        (Some("After".into()), Some("after".into())),
        "in-page targets account for the badge"
    );
    assert_eq!(
        landing(&doc, "Past"),
        (None, None),
        "# with no later heading is inert"
    );
}

#[test]
fn table_cell_anchor_and_unnamed_collapse_marker() {
    let doc = MicronEngine::new()
        .render(&EngineInput::new(
            address("cells.mu"),
            "\u{60}[To cell\u{60}#cell]\n\u{60}t\n| Name |\n| --- |\n| \u{60}:cell Here |\n\u{60}t\n\u{60}->\nAfter",
        ))
        .unwrap();
    let [(_, target)] = in_page(&doc).try_into().unwrap();
    assert_eq!(target.fragment.as_deref(), Some("cell"));
    assert!(
        matches!(
            unwrap(&doc.blocks[target.block.unwrap()]),
            Block::Table { .. }
        ),
        "design note §8: a cell anchor targets its table"
    );
    assert!(
        doc.navigation.folds.is_empty(),
        "no heading row, so no fold"
    );
    assert!(diagnosed(&doc, "collapse marker on an unnamed section"));
}

#[test]
fn serde_round_trip_keeps_the_table_and_old_packets_default_it() {
    let doc = lower("navigation/probe-nav-05-closed-target.mu");
    assert!(!doc.navigation.folds.is_empty() && !in_page(&doc).is_empty());
    let json = serde_json::to_value(&doc).unwrap();
    assert_eq!(
        serde_json::from_value::<EngineDocument>(json.clone()).unwrap(),
        doc
    );

    let mut old = json;
    old.as_object_mut().unwrap().remove("navigation");
    let old: EngineDocument = serde_json::from_value(old).unwrap();
    assert_eq!(old.navigation, DocumentNavigation::default());
}
