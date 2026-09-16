// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! P1 fold-aware layout, asserted over the committed Micron probe pages and
//! inline sources: skipped extents, reserved link identities, heading markers
//! and fold and in-page regions, plus the P1b marker and in-page adornment
//! tokens.

use inker::{
    Block, DocumentFold, DocumentNavigation, DocumentProvenance, DocumentTrustState, Engine,
    EngineInput, FoldMarkers, FoldState, inline_text,
};
use nematic::MicronEngine;

use super::*;
use crate::style_sheet::LinkAdornment;

const NODE: &str = "923706ddc70d389bd3719258c41f6592";

macro_rules! pages {
    ($($file:literal),* $(,)?) => {
        &[$(($file, include_str!(concat!(
            "../../../../nematic/nematic/tests/fixtures/micron/nomadnet-1.4.2/",
            $file
        )))),*]
    };
}

/// Every committed page with a collapsible heading.
const FOLD_PAGES: &[(&str, &str)] = pages![
    "guide-structure.mu",
    "navigation/probe-nav-05-closed-target.mu",
    "navigation/probe-nav-06a-same-depth-collapsible.mu",
    "navigation/probe-nav-06b-nested-collapsible.mu",
    "navigation/probe-nav-07c-section-exit-fold.mu",
    "navigation/probe-nav-08-toggle-keys.mu",
    "navigation/probe-nav-11a-unnamed-fold.mu",
    "navigation/probe-nav-11c-unnamed-fold-depths.mu",
    "navigation/probe-nav-12-double-less-than-fold.mu",
    "navigation/probe-nav-13-less-than-text-fold.mu",
    "navigation/probe-nav-14c-target-closed.mu",
    "navigation/probe-nav-17-nested-closed-target.mu",
];

const LINK_PAGES: &[(&str, &str)] = pages![
    "navigation/probe-nav-03-missing-anchor.mu",
    "navigation/probe-nav-09-transport.mu",
];

fn viewport() -> Viewport {
    Viewport::new(614.0, 600.0)
}

fn micron(file: &str, source: &str) -> EngineDocument {
    MicronEngine::new()
        .render(&EngineInput::new(format!("{NODE}:/page/{file}"), source))
        .unwrap()
}

fn page(file: &str) -> EngineDocument {
    let (_, source) = FOLD_PAGES
        .iter()
        .chain(LINK_PAGES)
        .find(|(name, _)| *name == file)
        .unwrap();
    micron(file, source)
}

/// Markers off, so a folded heading shapes exactly like an unfolded one.
fn unmarked() -> DocumentStyleSheet {
    let mut style = DocumentStyleSheet::default();
    style.fold_markers = FoldMarkers {
        open: String::new(),
        closed: String::new(),
    };
    style
}

fn fold_index(doc: &EngineDocument, heading: &str) -> usize {
    doc.navigation
        .folds
        .iter()
        .position(|fold| match &doc.blocks[fold.heading] {
            Block::Presented { block, .. } => {
                matches!(&**block, Block::Heading { spans, .. } if inline_text(spans) == heading)
            },
            Block::Heading { spans, .. } => inline_text(spans) == heading,
            _ => false,
        })
        .unwrap_or_else(|| panic!("no fold {heading:?}"))
}

fn all_open(doc: &EngineDocument) -> FoldState {
    let mut state = FoldState::default();
    for fold in 0..doc.navigation.folds.len() {
        if !state.is_open(&doc.navigation, fold) {
            state.toggle(&doc.navigation, fold);
        }
    }
    state
}

fn region<'a>(packet: &'a DocumentRenderPacket, label: &str) -> &'a InteractionRegion {
    packet
        .interactions
        .iter()
        .find(|region| {
            region
                .link_semantics
                .as_ref()
                .is_some_and(|semantics| semantics.accessible_label == label)
        })
        .unwrap_or_else(|| panic!("no link region {label:?}"))
}

fn unindexed(blocks: &[RenderedBlock]) -> Vec<RenderedBlock> {
    fn zero(block: &mut RenderedBlock) {
        block.source_block_index = 0;
        if let RenderedBlockKind::Group { children } = &mut block.kind {
            children.iter_mut().for_each(zero);
        }
    }
    let mut blocks = blocks.to_vec();
    blocks.iter_mut().for_each(zero);
    blocks
}

#[test]
fn fold_pages_with_nothing_closed_lay_out_like_the_unfolded_document() {
    let style = unmarked();
    for (file, source) in FOLD_PAGES {
        let doc = micron(file, source);
        assert!(!doc.navigation.folds.is_empty(), "{file} has folds");
        let folded = layout_document_with_folds(&doc, viewport(), &style, &all_open(&doc)).packet;
        let mut unfolded = doc.clone();
        unfolded.navigation = DocumentNavigation::default();
        let unfolded = layout_document(&unfolded, viewport(), &style).packet;

        assert_eq!(folded.blocks, unfolded.blocks, "{file}: identical geometry");
        assert_eq!(folded.content_bounds, unfolded.content_bounds, "{file}");
        let regions = |packet: &DocumentRenderPacket| {
            packet
                .interactions
                .iter()
                .filter(|region| !matches!(region.kind, InteractionKind::Fold { .. }))
                .map(|region| (region.bounds, region.link_semantics.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(regions(&folded), regions(&unfolded), "{file}");
        assert_eq!(
            folded
                .interactions
                .iter()
                .filter(|region| matches!(region.kind, InteractionKind::Fold { .. }))
                .count(),
            doc.navigation.folds.len(),
            "{file}: one fold region per heading"
        );
    }
}

#[test]
fn closed_extents_are_skipped_like_a_filtered_copy_but_keep_block_indices() {
    let style = unmarked();
    for (file, source) in FOLD_PAGES {
        let doc = micron(file, source);
        let state = FoldState::default();
        let hidden = state.hidden(&doc.navigation);
        let kept: Vec<usize> = (0..doc.blocks.len())
            .filter(|block| !hidden.iter().any(|range| range.contains(block)))
            .collect();
        let folded = layout_document_with_folds(&doc, viewport(), &style, &state).packet;
        let mut copy = doc.clone();
        copy.blocks = kept
            .iter()
            .map(|&block| doc.blocks[block].clone())
            .collect();
        copy.navigation = DocumentNavigation::default();
        let filtered = layout_document(&copy, viewport(), &style).packet;

        assert_eq!(
            folded
                .blocks
                .iter()
                .map(|block| block.source_block_index)
                .collect::<Vec<_>>(),
            kept,
            "{file}: original indices, closed extents absent"
        );
        assert_eq!(
            unindexed(&folded.blocks),
            unindexed(&filtered.blocks),
            "{file}: identical geometry"
        );
        assert_eq!(folded.content_bounds, filtered.content_bounds, "{file}");
    }
}

#[test]
fn a_wide_table_in_a_closed_fold_does_not_widen_the_content() {
    let doc = micron(
        "wide.mu",
        "`->Wide\n`t\n| Head | Head |\n| --- | --- |\n| unbreakable-first-column-cell-is-wide | unbreakable-second-column-cell-is-wide |\n`t\n>After\nTail.\n",
    );
    let narrow = Viewport::new(320.0, 600.0);
    let style = DocumentStyleSheet::default();
    let closed = layout_document_with_folds(&doc, narrow, &style, &FoldState::default()).packet;
    assert_eq!(closed.content_bounds.size.width, 320.0);
    let opened = layout_document_with_folds(&doc, narrow, &style, &all_open(&doc)).packet;
    assert!(
        opened.content_bounds.size.width > 320.0,
        "control: the open table overflows the narrow viewport"
    );
}

#[test]
fn link_identity_after_a_closed_fold_matches_the_open_layout() {
    // No committed fixture has a link after a fold.
    let doc = micron(
        "identity.mu",
        "`[Before`:/page/before.mu]\n`->Fold\n`[Inside`:/page/inside.mu] and `[In page`#after]\n`t\n| `[Head`:/page/head.mu] |\n| --- |\n| `[Cell`:/page/cell.mu] |\n`t\n>After\n`[After link`:/page/after.mu]\n",
    );
    let style = DocumentStyleSheet::default();
    let identity = |state: &FoldState| {
        let packet = layout_document_with_folds(&doc, viewport(), &style, state).packet;
        region(&packet, "After link")
            .link_semantics
            .as_ref()
            .unwrap()
            .identity
    };
    assert!(
        !FoldState::default().hidden(&doc.navigation).is_empty(),
        "the fold is closed"
    );
    assert_eq!(identity(&FoldState::default()), identity(&all_open(&doc)));

    // Every block kind that reserves identities, inside one closed fold.
    let link = |url: &str| InlineSpan::Link {
        url: url.into(),
        title: None,
        spans: vec![InlineSpan::Text(url.into())],
        predicate: None,
    };
    let paragraph = |url: &str| Block::Paragraph {
        spans: vec![link(url)],
    };
    let blocks = vec![
        Block::Heading {
            level: 1,
            spans: vec![InlineSpan::Text("Fold".into())],
        },
        Block::Presented {
            presentation: Default::default(),
            block: Box::new(paragraph("gemini://x.test/presented")),
        },
        Block::Table {
            alignments: Vec::new(),
            header: vec![vec![link("gemini://x.test/head")]],
            rows: vec![vec![
                vec![link("gemini://x.test/r1")],
                vec![link("gemini://x.test/r2")],
            ]],
        },
        Block::Quote {
            blocks: vec![paragraph("gemini://x.test/quote")],
        },
        Block::List {
            ordered: false,
            items: vec![
                vec![paragraph("gemini://x.test/l1")],
                vec![paragraph("gemini://x.test/l2")],
            ],
        },
        Block::Image {
            url: "gemini://x.test/i.png".into(),
            alt: "image".into(),
        },
        Block::FeedHeader {
            title: "Feed".into(),
            subtitle: None,
            summary: None,
            source_url: Some("gemini://x.test/source".into()),
        },
        Block::FeedEntry {
            title: "Entry".into(),
            date: None,
            summary: None,
            article_url: Some("gemini://x.test/article".into()),
            source_url: Some("gemini://x.test/entry-source".into()),
        },
        paragraph("gemini://x.test/after"),
    ];
    let doc = EngineDocument {
        address: "gemini://x.test/".into(),
        title: None,
        content_type: "text/gemini".into(),
        lang: None,
        provenance: DocumentProvenance::default(),
        trust: DocumentTrustState::Unknown,
        diagnostics: Vec::new(),
        navigation: DocumentNavigation {
            block_count: blocks.len(),
            anchors: Vec::new(),
            folds: vec![DocumentFold {
                heading: 0,
                source_line: 0,
                source_text: "Fold".into(),
                initially_open: false,
                extent: 1..blocks.len() - 1,
            }],
        },
        blocks,
    };
    let identity = |state: &FoldState| {
        layout_document_with_folds(&doc, viewport(), &style, state)
            .packet
            .interactions
            .iter()
            .find(|region| {
                matches!(&region.kind, InteractionKind::Link { url } if url == "gemini://x.test/after")
            })
            .and_then(|region| region.link_semantics.as_ref())
            .unwrap()
            .identity
    };
    assert_eq!(identity(&FoldState::default()), identity(&all_open(&doc)));
}

#[test]
fn probe_08_a_heading_gets_a_fold_region_and_the_style_sheet_marker() {
    let doc = page("navigation/probe-nav-08-toggle-keys.mu");
    let enter = fold_index(&doc, "Enter Target");
    let fold = &doc.navigation.folds[enter];
    let closed = FoldState::default();
    let packet =
        layout_document_with_folds(&doc, viewport(), &DocumentStyleSheet::default(), &closed)
            .packet;
    let heading = packet.top_level_block(fold.heading).unwrap();
    let toggle = packet
        .interactions
        .iter()
        .find(|region| region.kind == InteractionKind::Fold { fold: enter })
        .unwrap();
    assert_eq!(toggle.bounds.origin.y, heading.bounds.origin.y);
    assert!(toggle.bounds.size.width >= heading.bounds.size.width);
    assert!(
        packet.top_level_block(fold.extent.start).is_none(),
        "the authored-closed body is not laid out"
    );

    let glyphs = |style: &DocumentStyleSheet, state: &FoldState| {
        let packet = layout_document_with_folds(&doc, viewport(), style, state).packet;
        let RenderedBlockKind::Text { glyph_runs } =
            &packet.top_level_block(fold.heading).unwrap().kind
        else {
            panic!("heading text");
        };
        glyph_runs.iter().map(|run| run.glyphs.len()).sum::<usize>()
    };
    let mut marked = unmarked();
    marked.fold_markers = FoldMarkers {
        open: "o ".into(),
        closed: "ccc ".into(),
    };
    let open = all_open(&doc);
    assert_eq!(glyphs(&marked, &closed), glyphs(&unmarked(), &closed) + 4);
    assert_eq!(glyphs(&marked, &open), glyphs(&unmarked(), &open) + 2);
}

#[test]
fn probes_09_03_in_page_links_get_link_regions_that_never_navigate() {
    let style = DocumentStyleSheet::default();
    let doc = page("navigation/probe-nav-09-transport.mu");
    let packet = layout_document(&doc, viewport(), &style).packet;
    let jump = region(&packet, "in-page anchor jump");
    assert!(matches!(
        &jump.kind,
        InteractionKind::InPage { block: Some(_), fragment: Some(fragment) }
            if fragment == "transport-target"
    ));
    let (x, y) = (
        jump.bounds.origin.x + 1.0,
        jump.bounds.origin.y + jump.bounds.size.height * 0.5,
    );
    assert_eq!(packet.link_at(x, y), None, "never a URL");
    assert!(
        packet
            .blocks
            .iter()
            .filter_map(|block| match &block.kind {
                RenderedBlockKind::Text { glyph_runs } => Some(glyph_runs),
                _ => None,
            })
            .flatten()
            .any(|run| run.origin.y == jump.bounds.origin.y
                && run.color == style.token_color(ColorToken::LinkText)),
        "probe 09: link styled"
    );

    let doc = page("navigation/probe-nav-03-missing-anchor.mu");
    let packet = layout_document(&doc, viewport(), &style).packet;
    assert_eq!(
        region(&packet, "jump to a missing anchor").kind,
        InteractionKind::InPage {
            block: None,
            fragment: None
        },
        "probe 03: a focusable no-op"
    );

    let mut stale = doc.clone();
    stale.blocks.push(Block::Rule);
    let packet = layout_document(&stale, viewport(), &style).packet;
    assert_eq!(
        region(&packet, "jump to a present anchor").kind,
        InteractionKind::InPage {
            block: None,
            fragment: None
        },
        "a stale table's targets are inert"
    );
}

/// Glyphs painted in the top-level block holding the link labelled `label`.
fn link_row_glyphs(packet: &DocumentRenderPacket, label: &str) -> usize {
    let bounds = region(packet, label).bounds;
    let y = bounds.origin.y + bounds.size.height * 0.5;
    let block = packet
        .blocks
        .iter()
        .find(|block| block.bounds.origin.y <= y && y < block.bounds.max_y())
        .unwrap_or_else(|| panic!("no block under {label:?}"));
    let RenderedBlockKind::Text { glyph_runs } = &block.kind else {
        panic!("{label:?} row is text");
    };
    glyph_runs.iter().map(|run| run.glyphs.len()).sum()
}

const TOKEN_PAGES: [&str; 3] = [
    "navigation/probe-nav-05-closed-target.mu",
    "navigation/probe-nav-09-transport.mu",
    "navigation/probe-nav-17-nested-closed-target.mu",
];

#[test]
fn p1b_probes_05_09_17_default_tokens_paint_as_p1_did() {
    let default = DocumentStyleSheet::default();
    // P1 adorned in-page links with the network token; the markers are Inker's.
    let mut p1 = default.clone();
    p1.in_page_link_adornment = p1.link_adornment;
    p1.fold_markers = FoldMarkers::default();
    let mut unadorned = default.clone();
    unadorned.in_page_link_adornment = LinkAdornment::None;
    for file in TOKEN_PAGES {
        let doc = page(file);
        for state in [FoldState::default(), all_open(&doc)] {
            let paint = |style: &DocumentStyleSheet| {
                layout_document_with_folds(&doc, viewport(), style, &state).packet
            };
            assert_eq!(paint(&default), paint(&p1), "{file}: default paints as P1");
            assert_ne!(
                paint(&unadorned),
                paint(&default),
                "{file}: control, the page exercises the in-page token"
            );
            if !doc.navigation.folds.is_empty() {
                assert_ne!(
                    paint(&unmarked()),
                    paint(&default),
                    "{file}: control, the page exercises the marker token"
                );
            }
        }
    }
}

#[test]
fn p1b_probe_09_in_page_and_network_adornments_paint_independently() {
    let doc = page("navigation/probe-nav-09-transport.mu");
    let rows = |link: LinkAdornment, in_page: LinkAdornment| {
        let mut style = DocumentStyleSheet::default();
        style.link_adornment = link;
        style.in_page_link_adornment = in_page;
        let packet = layout_document(&doc, viewport(), &style).packet;
        (
            link_row_glyphs(&packet, "in-page anchor jump"),
            link_row_glyphs(&packet, "same-node page load, positive control"),
        )
    };
    // Both scheme arrows are one glyph plus a space.
    let arrow = LinkAdornment::SchemeArrow
        .prefix_for("#", None)
        .unwrap()
        .chars()
        .count();
    let (on, off) = (LinkAdornment::SchemeArrow, LinkAdornment::None);
    let (in_page, network) = rows(off, off);
    assert_eq!(rows(on, on), (in_page + arrow, network + arrow), "both");
    assert_eq!(rows(on, off), (in_page, network + arrow), "in-page off");
    assert_eq!(rows(off, on), (in_page + arrow, network), "network off");
}

#[test]
fn top_level_block_skips_group_children_that_reuse_its_index() {
    let text = |text: &str| Block::Paragraph {
        spans: vec![InlineSpan::Text(text.into())],
    };
    let doc = EngineDocument {
        address: "doc:top-level".into(),
        title: None,
        content_type: "text/plain".into(),
        lang: None,
        provenance: DocumentProvenance::default(),
        trust: DocumentTrustState::Unknown,
        diagnostics: Vec::new(),
        navigation: DocumentNavigation::default(),
        blocks: vec![
            Block::Quote {
                blocks: vec![text("quoted zero"), text("quoted one")],
            },
            text("top-level one"),
        ],
    };
    let packet = layout_document(&doc, viewport(), &DocumentStyleSheet::default()).packet;
    let RenderedBlockKind::Group { children } = &packet.blocks[0].kind else {
        panic!("quote group");
    };
    assert_eq!(
        children[1].source_block_index, 1,
        "control: a child reuses index 1"
    );
    assert_eq!(
        packet.top_level_block(1).map(|block| block.bounds),
        Some(packet.blocks[1].bounds)
    );
}
