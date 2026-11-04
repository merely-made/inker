// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

#[test]
fn source_presentation_controls_inline_paint_and_block_geometry() {
    let document = doc(vec![Block::Presented {
        presentation: BlockPresentation {
            alignment: BlockAlignment::Center,
            indent_level: 2,
        },
        block: Box::new(Block::Paragraph {
            spans: vec![InlineSpan::Presented {
                presentation: InlinePresentation {
                    foreground: Some([0x11, 0x22, 0x33]),
                    background: Some([0xaa, 0xbb, 0xcc]),
                    underline: true,
                },
                spans: vec![InlineSpan::Text("source paint".into())],
            }],
        }),
    }]);
    let style = DocumentStyleSheet::default();
    let respecting = layout_document(&document, viewport(), &style).packet;
    let RenderedBlockKind::Text { glyph_runs } = &respecting.blocks[0].kind else {
        panic!("expected presented paragraph text");
    };
    let run = glyph_runs.first().expect("text run");
    assert_eq!(
        run.color,
        [
            0x11 as f32 / 255.0,
            0x22 as f32 / 255.0,
            0x33 as f32 / 255.0,
            1.0
        ]
    );
    assert_eq!(
        run.background,
        Some([
            0xaa as f32 / 255.0,
            0xbb as f32 / 255.0,
            0xcc as f32 / 255.0,
            1.0
        ])
    );
    assert!(run.underline);
    assert!(
        respecting.blocks[0].bounds.origin.x
            >= style.horizontal_padding + 2.0 * style.indent_per_level,
        "section depth indents the content column"
    );
    assert!(
        run.origin.x > respecting.blocks[0].bounds.origin.x + 10.0,
        "center alignment moves the shaped run within the indented content column"
    );

    let mut reader_style = style.clone();
    reader_style.source_presentation = SourcePresentation::Reader;
    let reader = layout_document(&document, viewport(), &reader_style).packet;
    let RenderedBlockKind::Text { glyph_runs } = &reader.blocks[0].kind else {
        panic!("expected reader paragraph text");
    };
    let run = glyph_runs.first().expect("text run");
    assert_eq!(run.background, None);
    assert!(!run.underline);
    assert_ne!(
        run.color,
        [
            0x11 as f32 / 255.0,
            0x22 as f32 / 255.0,
            0x33 as f32 / 255.0,
            1.0
        ]
    );
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
fn soft_breaks_reflow_by_default_and_are_kept_when_preserved() {
    // Gemtext's consecutive text lines, joined by soft breaks.
    let poem = || {
        doc(vec![Block::Paragraph {
            spans: vec![
                InlineSpan::Text("so much depends".into()),
                InlineSpan::SoftBreak,
                InlineSpan::Strong(vec![
                    InlineSpan::Text("upon".into()),
                    InlineSpan::SoftBreak,
                    InlineSpan::Text("a red wheel".into()),
                ]),
            ],
        }])
    };
    let reflowed = layout_document(&poem(), viewport(), &DocumentStyleSheet::default()).packet;
    let mut sheet = DocumentStyleSheet::default();
    sheet.soft_break = crate::SoftBreak::Preserve;
    let preserved = layout_document(&poem(), viewport(), &sheet).packet;
    let lines = |packet: &DocumentRenderPacket| line_glyph_xs(&packet.blocks[0]).len();
    assert_eq!(lines(&reflowed), 1, "reflow joins the lines with spaces");
    assert_eq!(lines(&preserved), 3, "preserve keeps every source line");
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
