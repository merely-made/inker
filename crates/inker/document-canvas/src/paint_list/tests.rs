// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;
use crate::font_table::FontTable;
use crate::layout::layout_document;
use crate::style_sheet::DocumentStyleSheet;
use crate::types::Viewport;
use inker::{
    Block, DocumentProvenance, DocumentTrustState, EngineDocument, InlinePresentation, InlineSpan,
};

fn doc(blocks: Vec<Block>) -> EngineDocument {
    EngineDocument {
        address: "doc:paint-list-test".into(),
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

/// Lay out + build the paint list, returning both the list and the
/// font sidecar (so tests can assert shipped faces came from parley).
fn list_for(blocks: Vec<Block>) -> (InkerPaintList, FontTable) {
    let laid = layout_document(
        &doc(blocks),
        Viewport::new(640.0, 480.0),
        &DocumentStyleSheet::default(),
    );
    let list = paint_list_from_packet(&laid.packet, &laid.fonts, &ColorVocabulary::default());
    (list, laid.fonts)
}

fn count<F: Fn(&PaintCmd) -> bool>(list: &InkerPaintList, pred: F) -> usize {
    list.commands().iter().filter(|c| pred(c)).count()
}

#[test]
fn empty_packet_produces_no_commands() {
    let (list, _) = list_for(vec![]);
    assert_eq!(list.engine_id(), EngineId::INKER);
    assert!(list.commands().is_empty());
    assert!(list.fonts().is_empty());
    assert!(list.images().is_empty());
}

#[test]
fn source_background_and_underline_paint_around_the_shaped_run() {
    let (list, _) = list_for(vec![Block::Paragraph {
        spans: vec![InlineSpan::Presented {
            presentation: InlinePresentation {
                foreground: Some([0x11, 0x22, 0x33]),
                background: Some([0xaa, 0xbb, 0xcc]),
                underline: true,
            },
            spans: vec![InlineSpan::Text("source paint".into())],
        }],
    }]);
    let background = list
        .commands()
        .iter()
        .position(|command| matches!(command, PaintCmd::DrawRect(_)))
        .expect("source background produces a rect");
    let text = list
        .commands()
        .iter()
        .position(|command| matches!(command, PaintCmd::DrawText(_)))
        .expect("source text remains shaped");
    let underline = list
        .commands()
        .iter()
        .rposition(|command| matches!(command, PaintCmd::DrawLine(_)))
        .expect("source underline produces a line");
    assert!(
        background < text && text < underline,
        "background, text, underline paint in order"
    );
}

#[test]
fn decoded_image_emits_draw_image_and_resource_sidecar() {
    let laid = layout_document(
        &doc(vec![Block::Image {
            url: "https://x.test/pic.png".into(),
            alt: "picture".into(),
        }]),
        Viewport::new(640.0, 480.0),
        &DocumentStyleSheet::default(),
    );
    let decoded = DecodedImage {
        width: 2,
        height: 1,
        rgba8: vec![255, 0, 0, 255, 0, 0, 255, 255].into(),
    };
    let images = HashMap::from([("https://x.test/pic.png".to_string(), decoded.clone())]);
    let list = paint_list_from_packet_with_images(
        &laid.packet,
        &laid.fonts,
        &ColorVocabulary::default(),
        &images,
    );

    let draw = list
        .commands()
        .iter()
        .find_map(|command| match command {
            PaintCmd::DrawImage(image) => Some(image),
            _ => None,
        })
        .expect("decoded resource paints as an image");
    let resource = list
        .images()
        .iter()
        .find(|resource| resource.key == draw.image_key)
        .expect("draw key resolves through the image sidecar");
    assert_eq!((resource.width, resource.height), (2, 1));
    assert_eq!(resource.data, decoded.rgba8);
    assert!(std::sync::Arc::ptr_eq(&resource.data, &decoded.rgba8));
    assert!(
        !list
            .commands()
            .iter()
            .any(|command| matches!(command, PaintCmd::DrawRect(_))),
        "a resolved image does not fall back to its placeholder"
    );
}

#[test]
fn text_emits_drawtext_and_populates_fonts() {
    // With parley's real face threaded through, a paragraph renders as
    // real text (DrawText) with zero host font wiring — no placeholder
    // rect, no resolver.
    let (list, _) = list_for(vec![Block::Paragraph {
        spans: vec![InlineSpan::Text("hello".into())],
    }]);
    assert!(count(&list, |c| matches!(c, PaintCmd::DrawText(_))) >= 1);
    assert!(!list.fonts().is_empty(), "real text ships a font face");
}

#[test]
fn shipped_faces_come_from_the_sidecar() {
    // Regression guard for the wrong-face-on-fallback bug: every face
    // shipped in the side-table is byte-identical to a face in the
    // sidecar (parley's actually-shaped faces), never one re-resolved
    // from a (family, weight, style) label. Body + monospace exercises
    // more than one face.
    let (list, fonts) = list_for(vec![
        Block::Paragraph {
            spans: vec![InlineSpan::Text("body text".into())],
        },
        Block::CodeBlock {
            language: None,
            text: "fn main() {}".into(),
        },
    ]);
    let sidecar: Vec<(Vec<u8>, u32)> = fonts
        .iter()
        .map(|(_, f)| (f.data.data().to_vec(), f.index))
        .collect();
    assert!(!list.fonts().is_empty());
    for fr in list.fonts() {
        assert!(
            sidecar
                .iter()
                .any(|(d, i)| *d == *fr.data && *i == fr.index),
            "shipped face must originate from parley's sidecar, not a label re-resolve"
        );
    }
}

#[test]
fn each_run_ships_the_face_it_was_shaped_against() {
    // The crux of the fix, stated per-run and deterministically: the
    // bytes shipped for a run's DrawText are exactly the bytes of the
    // sidecar face its FontFaceId points at — i.e. parley's
    // actually-shaped face. Glyph ids and shipped face now come from
    // the same place, so they can't desync (the old bug shipped a
    // label-re-resolved face that could differ on fallback).
    let laid = layout_document(
        &doc(vec![
            Block::Paragraph {
                spans: vec![InlineSpan::Text("alpha".into())],
            },
            Block::CodeBlock {
                language: None,
                text: "beta()".into(),
            },
        ]),
        Viewport::new(640.0, 480.0),
        &DocumentStyleSheet::default(),
    );
    let list = paint_list_from_packet(&laid.packet, &laid.fonts, &ColorVocabulary::default());

    // Flat doc → glyph runs and DrawText commands correspond 1:1 in
    // emission order.
    let runs: Vec<GlyphRun> = laid
        .packet
        .blocks
        .iter()
        .flat_map(|b| match &b.kind {
            RenderedBlockKind::Text { glyph_runs } => glyph_runs.clone(),
            _ => Vec::new(),
        })
        .collect();
    let draws: Vec<_> = list
        .commands()
        .iter()
        .filter_map(|c| match c {
            PaintCmd::DrawText(t) => Some(t.clone()),
            _ => None,
        })
        .collect();
    assert!(!runs.is_empty());
    assert_eq!(runs.len(), draws.len(), "one DrawText per non-empty run");

    for (run, draw) in runs.iter().zip(draws.iter()) {
        let face = laid.fonts.get(run.font_face).expect("run face in sidecar");
        let shipped = list
            .fonts()
            .iter()
            .find(|fr| fr.key == draw.font_instance)
            .expect("DrawText key present in fonts() table");
        assert_eq!(
            *shipped.data,
            face.data.data().to_vec(),
            "shipped bytes must equal the shaped face's bytes"
        );
        assert_eq!(shipped.index, face.index, "collection index must match");
    }
}

#[test]
fn rule_emits_line_command_in_rule_color() {
    let mut colors = ColorVocabulary::default();
    colors.rule = [1.0, 0.0, 0.0, 1.0];
    let laid = layout_document(
        &doc(vec![Block::Rule]),
        Viewport::new(640.0, 480.0),
        &DocumentStyleSheet::default(),
    );
    let list = paint_list_from_packet(&laid.packet, &laid.fonts, &colors);
    let line = list
        .commands()
        .iter()
        .find_map(|c| match c {
            PaintCmd::DrawLine(l) => Some(l),
            _ => None,
        })
        .expect("expected a DrawLine for the rule");
    assert_eq!(line.color, colorf([1.0, 0.0, 0.0, 1.0]));
}

#[test]
fn draw_text_carries_per_run_role_color() {
    // Text color now rides on each glyph run (baked from the sheet at
    // layout), not a single ColorVocabulary lookup: a heading lowers in
    // heading_text, a paragraph in body_text.
    let palette = DocumentStyleSheet::default().colors;
    let laid = layout_document(
        &doc(vec![
            Block::Heading {
                level: 1,
                spans: vec![InlineSpan::Text("Title".into())],
            },
            Block::Paragraph {
                spans: vec![InlineSpan::Text("body".into())],
            },
        ]),
        Viewport::new(640.0, 480.0),
        &DocumentStyleSheet::default(),
    );
    let list = paint_list_from_packet(&laid.packet, &laid.fonts, &ColorVocabulary::default());
    let text_colors: Vec<_> = list
        .commands()
        .iter()
        .filter_map(|c| match c {
            PaintCmd::DrawText(t) => Some(t.color),
            _ => None,
        })
        .collect();
    assert_ne!(palette.heading_text, palette.body_text, "roles must differ");
    assert!(
        text_colors.contains(&colorf(palette.heading_text)),
        "heading DrawText paints in heading_text"
    );
    assert!(
        text_colors.contains(&colorf(palette.body_text)),
        "body DrawText paints in body_text"
    );
}

#[test]
fn group_recurses_into_children() {
    // Each list item is a paragraph → real text now (not a placeholder
    // rect), so at least two DrawText commands.
    let (list, _) = list_for(vec![Block::List {
        ordered: false,
        items: vec![
            vec![Block::Paragraph {
                spans: vec![InlineSpan::Text("first".into())],
            }],
            vec![Block::Paragraph {
                spans: vec![InlineSpan::Text("second".into())],
            }],
        ],
    }]);
    assert!(count(&list, |c| matches!(c, PaintCmd::DrawText(_))) >= 2);
}

#[test]
fn repeated_runs_share_one_font_side_table_entry() {
    // Two body paragraphs share one parley face → interned once; both
    // DrawText runs reference the same FontInstanceKey.
    let (list, _) = list_for(vec![
        Block::Paragraph {
            spans: vec![InlineSpan::Text("first".into())],
        },
        Block::Paragraph {
            spans: vec![InlineSpan::Text("second".into())],
        },
    ]);
    assert_eq!(list.fonts().len(), 1, "shared face should intern once");
    let keys: Vec<_> = list
        .commands()
        .iter()
        .filter_map(|c| match c {
            PaintCmd::DrawText(t) => Some(t.font_instance),
            _ => None,
        })
        .collect();
    assert!(keys.len() >= 2);
    assert!(keys.iter().all(|k| *k == keys[0]), "all runs share one key");
}

#[test]
fn viewport_rounds_to_device_int_size() {
    let laid = layout_document(
        &doc(vec![]),
        Viewport::new(640.4, 480.6),
        &DocumentStyleSheet::default(),
    );
    let list = paint_list_from_packet(&laid.packet, &laid.fonts, &ColorVocabulary::default());
    assert_eq!(list.viewport().width, 640);
    assert_eq!(list.viewport().height, 481);
}
