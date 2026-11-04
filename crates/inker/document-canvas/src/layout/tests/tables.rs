// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

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
