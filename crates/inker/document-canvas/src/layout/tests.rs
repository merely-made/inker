// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;
use crate::LinkAdornment;
use crate::types::InteractionKind;
use inker::{
    Block, DocumentProvenance, DocumentTrustState, EngineDocument, InlineSpan, TableAlignment,
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
fn paragraph_with_link_emits_interaction_region() {
    let packet = layout_document(
        &doc(vec![Block::Paragraph {
            spans: vec![
                InlineSpan::Text("see ".into()),
                InlineSpan::Link {
                    url: "https://x.test/".into(),
                    title: None,
                    spans: vec![InlineSpan::Text("docs".into())],
                    predicate: None,
                },
                InlineSpan::Text(" please".into()),
            ],
        }]),
        viewport(),
        &DocumentStyleSheet::default(),
    )
    .packet;
    assert_eq!(packet.interactions.len(), 1);
    let region = &packet.interactions[0];
    match &region.kind {
        InteractionKind::Link { url } => assert_eq!(url, "https://x.test/"),
        InteractionKind::Submit { .. } => panic!("expected navigation link"),
    }
    assert_eq!(
        region
            .link_semantics
            .as_ref()
            .expect("link carries retained semantics")
            .accessible_label,
        "docs",
        "the visible arrow remains in the hit range but is not part of the semantic label"
    );
    assert!(region.bounds.size.width > 0.0);
    assert!(region.bounds.size.height > 0.0);
}

#[test]
fn duplicate_link_targets_keep_distinct_identities_and_lowered_labels() {
    let mut style = DocumentStyleSheet::default();
    style.link_adornment = LinkAdornment::None;
    let packet = layout_document(
        &doc(vec![Block::Paragraph {
            spans: vec![
                InlineSpan::Link {
                    url: "gemini://example.test/same".into(),
                    title: None,
                    spans: vec![InlineSpan::Text("First visit".into())],
                    predicate: None,
                },
                InlineSpan::Text(" and ".into()),
                InlineSpan::Link {
                    url: "gemini://example.test/same".into(),
                    title: None,
                    spans: vec![InlineSpan::Text("Second visit".into())],
                    predicate: None,
                },
            ],
        }]),
        viewport(),
        &style,
    )
    .packet;
    let links: Vec<_> = packet
        .interactions
        .iter()
        .filter(|region| matches!(region.kind, InteractionKind::Link { .. }))
        .collect();
    assert_eq!(links.len(), 2, "one region for each unwrapped link");
    let first = links[0]
        .link_semantics
        .as_ref()
        .expect("link carries retained semantics");
    let second = links[1]
        .link_semantics
        .as_ref()
        .expect("link carries retained semantics");
    assert_ne!(first.identity, second.identity, "same URL is not identity");
    assert_eq!(first.accessible_label, "First visit");
    assert_eq!(second.accessible_label, "Second visit");
}

#[test]
fn wrapped_link_rectangles_share_one_identity_and_label() {
    let mut style = DocumentStyleSheet::default();
    style.link_adornment = LinkAdornment::None;
    let label = "one two three four five six seven eight nine ten";
    let document = doc(vec![Block::Paragraph {
        spans: vec![InlineSpan::Link {
            url: "gemini://example.test/wrapped".into(),
            title: None,
            spans: vec![InlineSpan::Text(label.into())],
            predicate: None,
        }],
    }]);
    let packet = layout_document(&document, Viewport::new(130.0, 480.0), &style).packet;
    let links: Vec<_> = packet
        .interactions
        .iter()
        .filter(|region| matches!(region.kind, InteractionKind::Link { .. }))
        .collect();
    assert!(
        links.len() >= 2,
        "the narrow viewport should split the link into line rectangles"
    );
    let first = links[0]
        .link_semantics
        .as_ref()
        .expect("link carries retained semantics");
    let reflowed = layout_document(&document, viewport(), &style).packet;
    let reflowed_identity = reflowed
        .interactions
        .iter()
        .find_map(|region| region.link_semantics.as_ref())
        .expect("reflowed link carries retained semantics")
        .identity;
    assert_eq!(
        first.identity, reflowed_identity,
        "the logical link keeps its identity across a geometry-only reflow"
    );
    for region in &links[1..] {
        let semantics = region
            .link_semantics
            .as_ref()
            .expect("every wrapped rectangle carries semantics");
        assert_eq!(semantics.identity, first.identity);
        assert_eq!(semantics.accessible_label, label);
    }
}

#[test]
fn submission_span_emits_a_non_navigation_interaction() {
    let packet = layout_document(
        &doc(vec![Block::Paragraph {
            spans: vec![InlineSpan::Submit {
                target: "/guestbook/sign".into(),
                spans: vec![InlineSpan::Text("Sign the guestbook".into())],
            }],
        }]),
        viewport(),
        &DocumentStyleSheet::default(),
    )
    .packet;
    let region = &packet.interactions[0];
    assert!(matches!(
        &region.kind,
        InteractionKind::Submit { target } if target == "/guestbook/sign"
    ));
    let x = region.bounds.origin.x + 1.0;
    let y = region.bounds.origin.y + 1.0;
    assert!(packet.link_at(x, y).is_none());
    assert!(matches!(
        packet.interaction_at(x, y),
        Some(InteractionKind::Submit { .. })
    ));
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

#[test]
fn document_dedups_shared_face() {
    // Two body paragraphs shape against the same family/weight, so
    // parley returns the same face for both runs → one entry in the
    // sidecar, and both runs carry the same FontFaceId.
    let laid = layout_document(
        &doc(vec![
            Block::Paragraph {
                spans: vec![InlineSpan::Text("first".into())],
            },
            Block::Paragraph {
                spans: vec![InlineSpan::Text("second".into())],
            },
        ]),
        viewport(),
        &DocumentStyleSheet::default(),
    );
    assert_eq!(laid.fonts.len(), 1, "shared body face should intern once");
    let faces: Vec<_> = laid
        .packet
        .blocks
        .iter()
        .filter_map(|b| match &b.kind {
            RenderedBlockKind::Text { glyph_runs } => glyph_runs.first(),
            _ => None,
        })
        .map(|r| r.font_face)
        .collect();
    assert!(faces.len() >= 2, "expected a run per paragraph");
    assert!(
        faces.iter().all(|f| *f == faces[0]),
        "runs share one face id"
    );
}

#[test]
fn text_populates_font_sidecar() {
    // Mirrors genet's `emit_with_layouts_populates_font_table`: real
    // text yields a non-empty sidecar, every run's face resolves in
    // it, and the resolved face carries real bytes.
    let laid = layout_document(
        &doc(vec![Block::Paragraph {
            spans: vec![InlineSpan::Text("hello".into())],
        }]),
        viewport(),
        &DocumentStyleSheet::default(),
    );
    assert!(
        !laid.fonts.is_empty(),
        "text should populate the font sidecar"
    );
    for block in &laid.packet.blocks {
        if let RenderedBlockKind::Text { glyph_runs } = &block.kind {
            for run in glyph_runs {
                let face = laid
                    .fonts
                    .get(run.font_face)
                    .expect("run face resolves in sidecar");
                assert!(!face.data.data().is_empty(), "face carries real bytes");
            }
        }
    }
}

#[test]
fn nowrap_role_overflows_instead_of_wrapping() {
    let long = "a long single line of body text that would otherwise wrap across many lines";
    let make = || {
        doc(vec![Block::Paragraph {
            spans: vec![InlineSpan::Text(long.into())],
        }])
    };
    let narrow = Viewport::new(160.0, 800.0);

    // Wrap (default): the block is constrained to the content width and wraps tall.
    let wrapped = layout_document(&make(), narrow, &DocumentStyleSheet::default()).packet;

    // NoWrap: the body role lays out on its natural width, overflowing on one line.
    let mut sheet = DocumentStyleSheet::default();
    sheet.roles.body.wrap = crate::WrapPolicy::NoWrap;
    let unwrapped = layout_document(&make(), narrow, &sheet).packet;

    let wrapped_b = &wrapped.blocks[0].bounds.size;
    let unwrapped_b = &unwrapped.blocks[0].bounds.size;
    assert!(
        unwrapped_b.width > wrapped_b.width,
        "NoWrap block should be wider than Wrap: {} vs {}",
        unwrapped_b.width,
        wrapped_b.width
    );
    assert!(
        unwrapped_b.width > narrow.width,
        "NoWrap block overflows the viewport width for the host to scroll: {} vs {}",
        unwrapped_b.width,
        narrow.width
    );
    assert!(
        unwrapped_b.height < wrapped_b.height,
        "NoWrap is one line, so shorter than the wrapped block"
    );
}

#[test]
fn glyph_runs_carry_per_role_colors() {
    // Each run is colored by its block / inline role: a heading in
    // heading_text, body in body_text, a link in link_text, inline code in
    // code_text. parley segments the paragraph into separate runs at the
    // brush boundaries, so the link + code sub-runs get their own color.
    let palette = DocumentStyleSheet::default().colors;
    let packet = layout_document(
        &doc(vec![
            Block::Heading {
                level: 1,
                spans: vec![InlineSpan::Text("Title".into())],
            },
            Block::Paragraph {
                spans: vec![
                    InlineSpan::Text("see ".into()),
                    InlineSpan::Link {
                        url: "https://x.test/".into(),
                        title: None,
                        spans: vec![InlineSpan::Text("link".into())],
                        predicate: None,
                    },
                    InlineSpan::Text(" or ".into()),
                    InlineSpan::Code("snippet".into()),
                ],
            },
        ]),
        viewport(),
        &DocumentStyleSheet::default(),
    )
    .packet;
    let colors: Vec<[f32; 4]> = packet
        .blocks
        .iter()
        .filter_map(|b| match &b.kind {
            RenderedBlockKind::Text { glyph_runs } => Some(glyph_runs.iter().map(|r| r.color)),
            _ => None,
        })
        .flatten()
        .collect();
    assert!(
        colors.contains(&palette.heading_text),
        "heading run uses heading_text"
    );
    assert!(
        colors.contains(&palette.body_text),
        "body run uses body_text"
    );
    assert!(
        colors.contains(&palette.link_text),
        "link run uses link_text"
    );
    assert!(
        colors.contains(&palette.code_text),
        "inline code run uses code_text"
    );
}

#[test]
fn table_renders_header_body_ragged_rows_with_source_identity() {
    let packet = layout_document(
        &doc(vec![Block::Table {
            alignments: vec![
                TableAlignment::Left,
                TableAlignment::Center,
                TableAlignment::Right,
            ],
            header: vec![
                vec![InlineSpan::Text("Name".into())],
                vec![InlineSpan::Text("Status".into())],
                vec![InlineSpan::Text("Count".into())],
            ],
            rows: vec![
                vec![
                    vec![InlineSpan::Text("alpha".into())],
                    vec![InlineSpan::Text("ready".into())],
                    vec![InlineSpan::Text("3".into())],
                ],
                vec![
                    vec![InlineSpan::Text("beta".into())],
                    vec![InlineSpan::Text("queued".into())],
                ],
            ],
        }]),
        viewport(),
        &DocumentStyleSheet::default(),
    )
    .packet;

    let RenderedBlockKind::Group { children } = &packet.blocks[0].kind else {
        panic!("table should lower to a Group");
    };
    assert_eq!(
        children.len(),
        9,
        "ragged rows retain their blank cell slots"
    );
    assert!(children.iter().all(|child| child.source_block_index == 0));
    assert!(children.iter().all(|child| child.bounds.size.width > 0.0));
    assert!(children[0].bounds.origin.y < children[3].bounds.origin.y);
    assert!(children[3].bounds.origin.y < children[6].bounds.origin.y);
    assert!(packet.blocks[0].bounds.size.width > 0.0);
}

#[test]
fn table_wraps_narrow_cells_and_aligns_link_hit_regions() {
    let link = |label: &str| InlineSpan::Link {
        url: "gemini://example.test/item".into(),
        title: None,
        spans: vec![InlineSpan::Text(label.into())],
        predicate: None,
    };
    let packet = layout_document(
        &doc(vec![Block::Table {
            alignments: vec![TableAlignment::Left, TableAlignment::Right],
            header: Vec::new(),
            rows: vec![vec![
                vec![InlineSpan::Text("a very long value that must wrap".into())],
                vec![link("go")],
            ]],
        }]),
        Viewport::new(190.0, 480.0),
        &DocumentStyleSheet::default(),
    )
    .packet;

    let RenderedBlockKind::Group { children } = &packet.blocks[0].kind else {
        panic!("table should lower to a Group");
    };
    assert_eq!(children.len(), 2);
    assert!(
        children[0].bounds.size.height > DocumentStyleSheet::default().line_height(14.0),
        "long cell should wrap in the narrow table"
    );
    let second = &children[1];
    let RenderedBlockKind::Text { glyph_runs } = &second.kind else {
        panic!("table cell should remain a Text block");
    };
    let first_run = glyph_runs.first().expect("link cell has glyphs");
    assert!(
        first_run.origin.x > second.bounds.origin.x,
        "right aligned text should move inside its cell"
    );

    assert!(!packet.interactions.is_empty());
    let identity = packet.interactions[0]
        .link_semantics
        .as_ref()
        .expect("table link retains semantics")
        .identity;
    assert!(packet.interactions.iter().all(|region| {
        region
            .link_semantics
            .as_ref()
            .is_some_and(|semantics| semantics.identity == identity)
    }));
    let region = &packet.interactions[0];
    assert!(region.bounds.origin.x > second.bounds.origin.x);
    assert_eq!(
        packet.link_at(region.bounds.origin.x + 1.0, region.bounds.origin.y + 1.0),
        Some("gemini://example.test/item")
    );
}

#[test]
fn table_links_with_same_target_keep_distinct_semantic_identities() {
    let link = |label: &str| InlineSpan::Link {
        url: "gemini://example.test/same".into(),
        title: None,
        spans: vec![InlineSpan::Text(label.into())],
        predicate: None,
    };
    let packet = layout_document(
        &doc(vec![Block::Table {
            alignments: vec![TableAlignment::Left, TableAlignment::Left],
            header: Vec::new(),
            rows: vec![vec![vec![link("first")], vec![link("second")]]],
        }]),
        viewport(),
        &DocumentStyleSheet::default(),
    )
    .packet;

    assert_eq!(packet.interactions.len(), 2);
    let first = packet.interactions[0]
        .link_semantics
        .as_ref()
        .expect("first table link carries semantics");
    let second = packet.interactions[1]
        .link_semantics
        .as_ref()
        .expect("second table link carries semantics");
    assert_ne!(first.identity, second.identity);
    assert_eq!(first.accessible_label, "first");
    assert_eq!(second.accessible_label, "second");
}

#[test]
fn table_alignment_keeps_wrapped_mixed_style_links_inside_the_cell() {
    let packet = layout_document(
        &doc(vec![Block::Table {
            alignments: vec![TableAlignment::Right],
            header: Vec::new(),
            rows: vec![vec![vec![
                InlineSpan::Text("prefix ".into()),
                InlineSpan::Strong(vec![InlineSpan::Text("bold ".into())]),
                InlineSpan::Link {
                    url: "gemini://example.test/mixed".into(),
                    title: None,
                    spans: vec![InlineSpan::Text("one two three four five six".into())],
                    predicate: None,
                },
                InlineSpan::Text(" suffix".into()),
            ]]],
        }]),
        Viewport::new(150.0, 480.0),
        &DocumentStyleSheet::default(),
    )
    .packet;
    let RenderedBlockKind::Group { children } = &packet.blocks[0].kind else {
        panic!("table should lower to a Group");
    };
    let cell = &children[0];
    let RenderedBlockKind::Text { glyph_runs } = &cell.kind else {
        panic!("table cell should remain a Text block");
    };
    assert!(
        glyph_runs.len() > 1,
        "mixed content should wrap into multiple runs"
    );
    for run in glyph_runs {
        let run_max_x = run.origin.x
            + run
                .glyphs
                .iter()
                .map(|glyph| glyph.x + glyph.advance)
                .fold(0.0_f32, f32::max);
        assert!(run.origin.x >= cell.bounds.origin.x - 0.01);
        assert!(run_max_x <= cell.bounds.max_x() + 0.01);
    }
    assert!(!packet.interactions.is_empty());
    for region in &packet.interactions {
        assert!(region.bounds.origin.x >= cell.bounds.origin.x - 0.01);
        assert!(region.bounds.max_x() <= cell.bounds.max_x() + 0.01);
        assert_eq!(
            packet.link_at(region.bounds.origin.x + 1.0, region.bounds.origin.y + 1.0),
            Some("gemini://example.test/mixed")
        );
    }
}

#[test]
fn many_narrow_table_columns_overflow_without_cell_overlap() {
    let row = (0..12)
        .map(|column| vec![InlineSpan::Text(format!("unbreakable-{column}"))])
        .collect();
    let packet = layout_document(
        &doc(vec![Block::Table {
            alignments: Vec::new(),
            header: Vec::new(),
            rows: vec![row],
        }]),
        Viewport::new(120.0, 480.0),
        &DocumentStyleSheet::default(),
    )
    .packet;
    let RenderedBlockKind::Group { children } = &packet.blocks[0].kind else {
        panic!("table should lower to a Group");
    };
    assert_eq!(children.len(), 12);
    assert!(packet.blocks[0].bounds.max_x() > packet.viewport.width);
    assert!(packet.content_bounds.size.width > packet.viewport.width);
    for pair in children.windows(2) {
        assert!(
            pair[0].bounds.max_x() <= pair[1].bounds.origin.x + 0.01,
            "adjacent cells must remain disjoint: {:?} then {:?}",
            pair[0].bounds,
            pair[1].bounds
        );
    }
    for child in children {
        let RenderedBlockKind::Text { glyph_runs } = &child.kind else {
            panic!("table cell should remain a Text block");
        };
        for run in glyph_runs {
            let run_max_x = run.origin.x
                + run
                    .glyphs
                    .iter()
                    .map(|glyph| glyph.x + glyph.advance)
                    .fold(0.0_f32, f32::max);
            assert!(run_max_x <= child.bounds.max_x() + 0.01);
        }
    }
}

#[test]
fn normal_width_table_wraps_unbroken_link_inside_its_cell() {
    let label = "W".repeat(128);
    let link = InlineSpan::Link {
        url: "gemini://example.test/long".into(),
        title: None,
        spans: vec![InlineSpan::Text(label)],
        predicate: None,
    };
    let packet = layout_document(
        &doc(vec![Block::Table {
            alignments: vec![TableAlignment::Left, TableAlignment::Left],
            header: Vec::new(),
            rows: vec![vec![vec![link], vec![InlineSpan::Text("ok".into())]]],
        }]),
        Viewport::new(240.0, 480.0),
        &DocumentStyleSheet::default(),
    )
    .packet;
    let RenderedBlockKind::Group { children } = &packet.blocks[0].kind else {
        panic!("table should lower to a Group");
    };
    assert_eq!(children.len(), 2);
    let first = &children[0];
    let second = &children[1];
    assert!(first.bounds.max_x() <= second.bounds.origin.x + 0.01);
    let first_glyph_max_x = match &first.kind {
        RenderedBlockKind::Text { glyph_runs } => glyph_runs
            .iter()
            .flat_map(|run| {
                run.glyphs
                    .iter()
                    .map(|glyph| run.origin.x + glyph.x + glyph.advance)
            })
            .fold(first.bounds.origin.x, f32::max),
        _ => panic!("table cell should remain a Text block"),
    };
    assert!(
        first_glyph_max_x <= first.bounds.max_x() + 0.01,
        "unbroken first-cell glyphs must not paint into the next cell"
    );
    let link_regions = packet
        .interactions
        .iter()
        .filter(|interaction| {
            matches!(&interaction.kind, InteractionKind::Link { url } if url == "gemini://example.test/long")
        })
        .collect::<Vec<_>>();
    assert!(!link_regions.is_empty(), "long link retains hit regions");
    assert!(
        link_regions
            .iter()
            .all(|region| region.bounds.max_x() <= first.bounds.max_x() + 0.01)
    );
}
