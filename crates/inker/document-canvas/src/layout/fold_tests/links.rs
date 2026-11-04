// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

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
            published: None,
            updated: None,
            guid: None,
            enclosures: Vec::new(),
            content_address: None,
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
