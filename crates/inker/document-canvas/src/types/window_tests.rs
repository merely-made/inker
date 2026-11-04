// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

fn text_block(y: f32, h: f32, run_y: f32) -> RenderedBlock {
    RenderedBlock {
        source_block_index: 0,
        bounds: Rect::from_xywh(0.0, y, 400.0, h),
        kind: RenderedBlockKind::Text {
            glyph_runs: vec![GlyphRun {
                origin: Point::new(0.0, run_y),
                font_size: 16.0,
                font_face: FontFaceId(0),
                font_family: "x".into(),
                font_weight: 400,
                font_style: TextStyle::Normal,
                glyphs: Vec::new(),
                baseline_y: 12.0,
                color: [0.0, 0.0, 0.0, 1.0],
                background: None,
                underline: false,
            }],
        },
    }
}

fn packet(blocks: Vec<RenderedBlock>, total_h: f32) -> DocumentRenderPacket {
    DocumentRenderPacket {
        viewport: Viewport::new(400.0, 600.0),
        content_bounds: Rect::from_xywh(0.0, 0.0, 400.0, total_h),
        blocks,
        interactions: Vec::new(),
    }
}

#[test]
fn window_keeps_only_intersecting_blocks_translated_to_band_origin() {
    // Three stacked blocks; a band over the middle one keeps only it, shifted so
    // the band top is y=0.
    let p = packet(
        vec![
            text_block(0.0, 100.0, 12.0),
            text_block(1000.0, 100.0, 1012.0),
            text_block(2000.0, 100.0, 2012.0),
        ],
        2100.0,
    );
    let w = p.window(950.0, 200.0); // band [950, 1150]
    assert_eq!(
        w.blocks.len(),
        1,
        "only the middle block intersects the band"
    );
    assert_eq!(
        w.blocks[0].bounds.origin.y, 50.0,
        "block translated by -band_y"
    );
    let RenderedBlockKind::Text { glyph_runs } = &w.blocks[0].kind else {
        panic!("text block");
    };
    assert_eq!(
        glyph_runs[0].origin.y, 62.0,
        "run origin translated by -band_y"
    );
    assert_eq!(w.viewport.height, 200.0, "viewport is the band height");
    assert_eq!(
        w.content_bounds.size.height, 200.0,
        "content_bounds is the band"
    );
    assert_eq!(w.viewport.width, 400.0, "width is preserved");
}

#[test]
fn window_keeps_a_block_straddling_the_band_edge() {
    // A block spanning the band's top edge is kept (partial visibility).
    let p = packet(vec![text_block(900.0, 200.0, 912.0)], 1200.0);
    let w = p.window(1000.0, 200.0); // band [1000, 1200]; block covers [900, 1100]
    assert_eq!(w.blocks.len(), 1, "the straddling block is kept");
    assert_eq!(
        w.blocks[0].bounds.origin.y, -100.0,
        "its top is above the band, at -100"
    );
}

#[test]
fn window_filters_group_children_to_the_band() {
    // A group spanning the whole document keeps only the children in the band.
    let group = RenderedBlock {
        source_block_index: 0,
        bounds: Rect::from_xywh(0.0, 0.0, 400.0, 3000.0),
        kind: RenderedBlockKind::Group {
            children: vec![
                text_block(0.0, 50.0, 12.0),
                text_block(1500.0, 50.0, 1512.0),
                text_block(2900.0, 50.0, 2912.0),
            ],
        },
    };
    let w = packet(vec![group], 3000.0).window(1400.0, 200.0); // band [1400, 1600]
    let RenderedBlockKind::Group { children } = &w.blocks[0].kind else {
        panic!("group survives");
    };
    assert_eq!(children.len(), 1, "only the in-band child is kept");
    assert_eq!(
        children[0].bounds.origin.y, 100.0,
        "child translated into the band"
    );
}

#[test]
fn window_filters_and_translates_interactions() {
    let mut p = packet(vec![text_block(1000.0, 100.0, 1012.0)], 2000.0);
    p.interactions = vec![
        InteractionRegion {
            bounds: Rect::from_xywh(0.0, 1010.0, 80.0, 20.0),
            kind: InteractionKind::Link { url: "in".into() },
            link_semantics: None,
        },
        InteractionRegion {
            bounds: Rect::from_xywh(0.0, 50.0, 80.0, 20.0),
            kind: InteractionKind::Link { url: "out".into() },
            link_semantics: None,
        },
    ];
    let w = p.window(950.0, 200.0);
    assert_eq!(w.interactions.len(), 1, "only the in-band link survives");
    assert_eq!(
        w.interactions[0].bounds.origin.y, 60.0,
        "link translated into the band"
    );
    assert!(matches!(&w.interactions[0].kind, InteractionKind::Link { url } if url == "in"));
}

#[test]
fn window_rect_clips_and_translates_x_coordinates_with_link_regions() {
    let mut block = text_block(0.0, 100.0, 12.0);
    block.bounds.origin.x = 300.0;
    if let RenderedBlockKind::Text { glyph_runs } = &mut block.kind {
        glyph_runs[0].origin.x = 300.0;
    }
    let mut p = packet(vec![block], 600.0);
    p.interactions = vec![InteractionRegion {
        bounds: Rect::from_xywh(320.0, 20.0, 80.0, 20.0),
        kind: InteractionKind::Link { url: "wide".into() },
        link_semantics: None,
    }];

    let w = p.window_rect(300.0, 0.0, 120.0, 80.0);
    assert_eq!(
        w.blocks.len(),
        1,
        "wide content intersects the horizontal window"
    );
    assert_eq!(w.blocks[0].bounds.origin.x, 0.0);
    assert_eq!(
        w.interactions.len(),
        1,
        "the visible link remains accessible"
    );
    assert_eq!(w.interactions[0].bounds.origin.x, 20.0);
    assert_eq!(w.link_at(25.0, 25.0), Some("wide"));
    assert_eq!(w.link_at(5.0, 25.0), None, "coordinates are window-local");
}

#[test]
fn link_at_returns_the_innermost_link_url() {
    let mut p = packet(vec![text_block(0.0, 100.0, 12.0)], 200.0);
    p.interactions = vec![
        InteractionRegion {
            bounds: Rect::from_xywh(0.0, 10.0, 80.0, 20.0),
            kind: InteractionKind::Link {
                url: "outer".into(),
            },
            link_semantics: None,
        },
        InteractionRegion {
            bounds: Rect::from_xywh(10.0, 12.0, 40.0, 16.0),
            kind: InteractionKind::Link {
                url: "inner".into(),
            },
            link_semantics: None,
        },
    ];
    assert_eq!(
        p.link_at(20.0, 18.0),
        Some("inner"),
        "the last (innermost) match wins"
    );
    assert_eq!(
        p.link_at(70.0, 18.0),
        Some("outer"),
        "outside inner, inside outer"
    );
    assert_eq!(p.link_at(300.0, 300.0), None, "no link at an empty point");
}

#[test]
fn block_at_finds_the_deepest_block() {
    let mut child = text_block(100.0, 50.0, 112.0);
    child.source_block_index = 7;
    let group = RenderedBlock {
        source_block_index: 0,
        bounds: Rect::from_xywh(0.0, 0.0, 400.0, 300.0),
        kind: RenderedBlockKind::Group {
            children: vec![child],
        },
    };
    let p = packet(vec![group], 300.0);
    assert_eq!(
        p.block_at(10.0, 120.0).map(|b| b.source_block_index),
        Some(7),
        "a point inside the child resolves to the child, not the group"
    );
    assert_eq!(
        p.block_at(10.0, 290.0).map(|b| b.source_block_index),
        Some(0),
        "a point in the group but outside any child resolves to the group"
    );
    assert!(
        p.block_at(500.0, 500.0).is_none(),
        "no block at an outside point"
    );
}
