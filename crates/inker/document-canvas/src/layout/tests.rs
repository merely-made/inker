// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;
use crate::LinkAdornment;
use crate::types::InteractionKind;
use inker::{
    Block, BlockAlignment, BlockPresentation, DocumentProvenance, DocumentTrustState,
    EngineDocument, InlinePresentation, InlineSpan, MenuItemKind, MenuRow, TableAlignment,
};

fn doc(blocks: Vec<Block>) -> EngineDocument {
    EngineDocument {
        address: "doc:test".into(),
        title: None,
        content_type: "text/plain".into(),
        lang: None,
        provenance: DocumentProvenance::default(),
        trust: DocumentTrustState::Unknown,
        diagnostics: Vec::new(),
        navigation: Default::default(),
        blocks,
    }
}

fn viewport() -> Viewport {
    Viewport::new(640.0, 480.0)
}

#[test]
fn empty_document_lays_out_to_empty_block_list() {
    let packet = layout_document(&doc(vec![]), viewport(), &DocumentStyleSheet::default()).packet;
    assert!(packet.blocks.is_empty());
    assert!(packet.interactions.is_empty());
    assert_eq!(packet.viewport.width, 640.0);
}

#[test]
fn single_paragraph_produces_one_text_block() {
    let packet = layout_document(
        &doc(vec![Block::Paragraph {
            spans: vec![InlineSpan::Text("Hello, world.".into())],
        }]),
        viewport(),
        &DocumentStyleSheet::default(),
    )
    .packet;
    assert_eq!(packet.blocks.len(), 1);
    let block = &packet.blocks[0];
    assert_eq!(block.source_block_index, 0);
    let RenderedBlockKind::Text { glyph_runs } = &block.kind else {
        panic!("expected Text kind, got {:?}", block.kind);
    };
    assert!(!glyph_runs.is_empty(), "expected at least one glyph run");
}

#[test]
fn heading_is_taller_than_paragraph() {
    let style = DocumentStyleSheet::default();
    let packet = layout_document(
        &doc(vec![
            Block::Heading {
                level: 1,
                spans: vec![InlineSpan::Text("Title".into())],
            },
            Block::Paragraph {
                spans: vec![InlineSpan::Text("Body.".into())],
            },
        ]),
        viewport(),
        &style,
    )
    .packet;
    assert_eq!(packet.blocks.len(), 2);
    let heading = &packet.blocks[0];
    let paragraph = &packet.blocks[1];
    assert!(
        heading.bounds.size.height > paragraph.bounds.size.height,
        "heading {:?} should be taller than paragraph {:?}",
        heading.bounds,
        paragraph.bounds
    );
}

#[test]
fn list_emits_group_block_with_children() {
    let packet = layout_document(
        &doc(vec![Block::List {
            ordered: false,
            items: vec![
                vec![Block::Paragraph {
                    spans: vec![InlineSpan::Text("first".into())],
                }],
                vec![Block::Paragraph {
                    spans: vec![InlineSpan::Text("second".into())],
                }],
            ],
        }]),
        viewport(),
        &DocumentStyleSheet::default(),
    )
    .packet;
    let RenderedBlockKind::Group { children } = &packet.blocks[0].kind else {
        panic!("expected Group kind");
    };
    assert_eq!(children.len(), 2);
}

#[test]
fn quote_emits_group_block_with_indented_children() {
    let style = DocumentStyleSheet::default();
    let packet = layout_document(
        &doc(vec![Block::Quote {
            blocks: vec![Block::Paragraph {
                spans: vec![InlineSpan::Text("quoted text".into())],
            }],
        }]),
        viewport(),
        &style,
    )
    .packet;
    let RenderedBlockKind::Group { children } = &packet.blocks[0].kind else {
        panic!("expected Group kind");
    };
    assert_eq!(children.len(), 1);
    // Indent should push the child's left edge inward.
    assert!(
        children[0].bounds.origin.x >= style.horizontal_padding + style.indent_per_level,
        "quote child should be indented; got x={}",
        children[0].bounds.origin.x
    );
}

#[test]
fn capped_content_column_is_centred_and_preserves_narrow_view_padding() {
    let mut style = DocumentStyleSheet::default();
    style.horizontal_padding = 32.0;
    style.max_content_width = Some(720.0);
    let document = doc(vec![Block::Paragraph {
        spans: vec![InlineSpan::Text("A readable line.".into())],
    }]);

    let wide = layout_document(&document, Viewport::new(1_680.0, 480.0), &style).packet;
    assert_eq!(wide.blocks[0].bounds.origin.x, 480.0);

    let narrow = layout_document(&document, Viewport::new(400.0, 480.0), &style).packet;
    assert_eq!(narrow.blocks[0].bounds.origin.x, 32.0);
}

#[test]
fn rule_emits_rule_block() {
    let packet = layout_document(
        &doc(vec![Block::Rule]),
        viewport(),
        &DocumentStyleSheet::default(),
    )
    .packet;
    assert!(matches!(packet.blocks[0].kind, RenderedBlockKind::Rule));
}

#[test]
fn image_emits_image_block_with_url_and_alt() {
    let packet = layout_document(
        &doc(vec![Block::Image {
            url: "https://x.test/pic.png".into(),
            alt: "a picture".into(),
        }]),
        viewport(),
        &DocumentStyleSheet::default(),
    )
    .packet;
    let RenderedBlockKind::Image { url, alt } = &packet.blocks[0].kind else {
        panic!("expected Image kind");
    };
    assert_eq!(url, "https://x.test/pic.png");
    assert_eq!(alt, "a picture");
    assert!(packet.interactions.iter().any(
        |region| matches!(&region.kind, InteractionKind::Link { url } if url == "https://x.test/pic.png")
    ));
}

#[test]
fn feed_entry_composes_into_group_with_h2_summary_link() {
    let packet = layout_document(
        &doc(vec![Block::FeedEntry {
            title: "Article".into(),
            date: Some("2026-05-09".into()),
            summary: Some("Summary text.".into()),
            article_url: Some("https://feed.test/x".into()),
            source_url: None,
            published: None,
            updated: None,
            guid: None,
            enclosures: Vec::new(),
            content_address: None,
        }]),
        viewport(),
        &DocumentStyleSheet::default(),
    )
    .packet;
    let RenderedBlockKind::Group { children } = &packet.blocks[0].kind else {
        panic!("expected Group kind");
    };
    // Heading + date + summary + article link = 4 children.
    assert_eq!(children.len(), 4);

    // Article URL surfaces as an interaction region.
    assert!(
        packet.interactions.iter().any(
            |r| matches!(&r.kind, InteractionKind::Link { url } if url == "https://feed.test/x")
        )
    );
}

#[test]
fn metadata_row_lays_out_label_and_value() {
    let packet = layout_document(
        &doc(vec![Block::MetadataRow {
            label: "Login".into(),
            value: "alice".into(),
        }]),
        viewport(),
        &DocumentStyleSheet::default(),
    )
    .packet;
    assert_eq!(packet.blocks.len(), 1);
    let RenderedBlockKind::Text { glyph_runs } = &packet.blocks[0].kind else {
        panic!("expected Text kind");
    };
    assert!(!glyph_runs.is_empty());
}

#[test]
fn content_bounds_grow_with_blocks() {
    let style = DocumentStyleSheet::default();
    let single = layout_document(
        &doc(vec![Block::Paragraph {
            spans: vec![InlineSpan::Text("one".into())],
        }]),
        viewport(),
        &style,
    )
    .packet;
    let several = layout_document(
        &doc(vec![
            Block::Paragraph {
                spans: vec![InlineSpan::Text("one".into())],
            },
            Block::Paragraph {
                spans: vec![InlineSpan::Text("two".into())],
            },
            Block::Paragraph {
                spans: vec![InlineSpan::Text("three".into())],
            },
        ]),
        viewport(),
        &style,
    )
    .packet;
    assert!(several.content_bounds.size.height > single.content_bounds.size.height);
}

fn menu_row(kind: MenuItemKind, label: &str, target: Option<&str>) -> MenuRow {
    MenuRow {
        kind,
        marker: None,
        label: vec![InlineSpan::Text(label.into())],
        target: target.map(Into::into),
    }
}

/// Each line's glyph x positions in reading order, from a text block's runs.
fn line_glyph_xs(block: &RenderedBlock) -> Vec<Vec<f32>> {
    let RenderedBlockKind::Text { glyph_runs } = &block.kind else {
        panic!("a text block: {:?}", block.kind);
    };
    let mut lines: Vec<(f32, Vec<f32>)> = Vec::new();
    for run in glyph_runs {
        let xs = run.glyphs.iter().map(|glyph| run.origin.x + glyph.x);
        match lines
            .iter_mut()
            .find(|(y, _)| (*y - run.origin.y).abs() < 0.5)
        {
            Some((_, line)) => line.extend(xs),
            None => lines.push((run.origin.y, xs.collect())),
        }
    }
    lines.sort_by(|a, b| a.0.total_cmp(&b.0));
    lines
        .into_iter()
        .map(|(_, mut xs)| {
            xs.sort_by(f32::total_cmp);
            xs
        })
        .collect()
}

#[test]
fn menu_label_column_starts_at_one_x_in_every_row() {
    let labels = [
        "  ___  welcome  ___",
        "home",
        "Phlog",
        "About",
        "Search the hole",
        "Elsewhere",
    ];
    let link = |url: &str, label: &str| InlineSpan::Link {
        url: url.into(),
        title: None,
        spans: vec![InlineSpan::Text(label.into())],
        predicate: None,
    };
    let rows = vec![
        menu_row(MenuItemKind::Info, labels[0], None),
        // An info row has an empty type label; its text lines up with the
        // others only if the type column is padded.
        MenuRow {
            kind: MenuItemKind::Info,
            marker: Some('i'),
            label: vec![link("gopher://h/1/", labels[1])],
            target: None,
        },
        menu_row(
            MenuItemKind::Directory,
            labels[2],
            Some("gopher://h/1/phlog"),
        ),
        menu_row(
            MenuItemKind::Document,
            labels[3],
            Some("gopher://h/0/about.txt"),
        ),
        MenuRow {
            kind: MenuItemKind::Search,
            marker: Some('7'),
            label: vec![InlineSpan::Submit {
                target: "gopher://h/7/find".into(),
                spans: vec![InlineSpan::Text(labels[4].into())],
            }],
            target: None,
        },
        menu_row(MenuItemKind::External, labels[5], Some("https://x.test/")),
    ];
    let mut style = DocumentStyleSheet::default();
    style.link_adornment = LinkAdornment::None;
    let packet = layout_document(&doc(vec![Block::Menu { rows }]), viewport(), &style).packet;
    assert_eq!(packet.blocks.len(), 1, "a menu is one block");

    let lines = line_glyph_xs(&packet.blocks[0]);
    assert_eq!(lines.len(), labels.len(), "one line per row");
    let label_xs: Vec<f32> = lines
        .iter()
        .zip(labels)
        .map(|(xs, label)| xs[xs.len() - label.chars().count()])
        .collect();
    for x in &label_xs {
        assert!(
            (x - label_xs[0]).abs() < 0.01,
            "label column drifts: {label_xs:?}"
        );
    }
    assert!(label_xs[0] > lines[0][0], "the type column comes first");

    assert_eq!(
        packet.interactions.len(),
        5,
        "four links and one submission"
    );
    assert!(matches!(
        &packet.interactions[3].kind,
        InteractionKind::Submit { target } if target == "gopher://h/7/find"
    ));
}

#[test]
fn a_long_menu_row_overflows_instead_of_wrapping() {
    let long = "a menu label long enough to wrap anywhere it was allowed to wrap at all";
    let packet = layout_document(
        &doc(vec![Block::Menu {
            rows: vec![
                menu_row(MenuItemKind::Directory, long, Some("gopher://h/1/a")),
                menu_row(MenuItemKind::Directory, "short", Some("gopher://h/1/b")),
            ],
        }]),
        Viewport::new(160.0, 800.0),
        &DocumentStyleSheet::default(),
    )
    .packet;
    let [first, second] = packet.interactions.as_slice() else {
        panic!("two links: {:?}", packet.interactions);
    };
    assert_eq!(
        first.bounds.size.height, second.bounds.size.height,
        "the long row stays one line"
    );
    assert!(packet.blocks[0].bounds.size.width > 160.0, "and overflows");
}

#[test]
fn a_feed_entry_with_a_body_offers_its_own_document_first() {
    let packet = layout_document(
        &doc(vec![Block::FeedEntry {
            title: "Post".into(),
            date: None,
            summary: Some("Teaser.".into()),
            article_url: Some("https://x.test/post".into()),
            source_url: None,
            published: None,
            updated: None,
            guid: Some("post-1".into()),
            enclosures: Vec::new(),
            content_address: Some("gemini://x.test/feed.xml#post-1".into()),
        }]),
        viewport(),
        &DocumentStyleSheet::default(),
    )
    .packet;
    let urls: Vec<_> = packet
        .interactions
        .iter()
        .filter_map(|region| match &region.kind {
            InteractionKind::Link { url } => Some(url.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        urls,
        ["gemini://x.test/feed.xml#post-1", "https://x.test/post"]
    );
}

#[path = "tests/tables.rs"]
mod tables;

#[path = "tests/typography.rs"]
mod typography;

#[path = "tests/links.rs"]
mod links;
