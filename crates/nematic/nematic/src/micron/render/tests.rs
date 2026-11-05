// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

const BASE: &str = "0123456789abcdef0123456789abcdef:/page/index.mu";

#[test]
fn ordinary_links_resolve_but_actions_and_anchors_never_become_fetches() {
    let source = ">Hello\n\u{60}[Read\u{60}:/page/read.mu]\n\u{60}[Send\u{60}:/page/action.mu\u{60}*]\n\u{60}[Top\u{60}#hello]";
    let doc = MicronEngine::new()
        .render(&EngineInput::new(BASE, source))
        .unwrap();
    assert_eq!(doc.title.as_deref(), Some("Hello"));
    assert_eq!(
        doc.outgoing_links(),
        vec!["0123456789abcdef0123456789abcdef:/page/read.mu".to_owned()]
    );
    assert!(!doc.diagnostics.is_empty());
}

#[test]
fn same_node_alias_is_retained_without_fabricating_a_destination() {
    let source = "\u{60}[Read\u{60}:/page/read.mu]";
    let doc = MicronEngine::new()
        .render(&EngineInput::new("file:///tmp/index.mu", source))
        .unwrap();
    assert_eq!(doc.outgoing_links(), vec![":/page/read.mu".to_owned()]);
    assert!(doc.diagnostics.iter().any(|diagnostic| matches!(diagnostic,
        DocumentDiagnostic::UnsupportedConstruct(message) if message.contains("unresolved manifest alias"))));
}

#[test]
fn table_lowering_preserves_cells_and_column_alignment() {
    let source = "\u{60}t\n| Name | Number |\n| :--- | ---: |\n| A | 2 |\n\u{60}t";
    let doc = MicronEngine::new()
        .render(&EngineInput::new(BASE, source))
        .unwrap();
    assert!(doc.blocks.iter().any(|block| matches!(block,
        Block::Table { header, rows, alignments }
            if header.len() == 2 && rows.len() == 1 && alignments == &[inker::TableAlignment::Left, inker::TableAlignment::Right])));
}

#[test]
fn captured_presentation_becomes_typed_source_wrappers() {
    let source = "\u{60}c\u{60}F123\u{60}Babc\u{60}_Styled\n>> Nested\n\u{60}tc30\n| Name | Count |\n| --- | ---: |\n| A | 2 |\n\u{60}t";
    let doc = MicronEngine::new()
        .render(&EngineInput::new(BASE, source))
        .unwrap();

    let paragraph = doc.blocks.iter().find_map(|block| match block {
        Block::Presented {
            presentation:
                BlockPresentation {
                    alignment: BlockAlignment::Center,
                    indent_level: 0,
                },
            block,
        } => match block.as_ref() {
            Block::Paragraph { spans } => Some(spans),
            _ => None,
        },
        _ => None,
    });
    let spans = paragraph.expect("centered source paragraph");
    assert!(matches!(
        spans.first(),
        Some(InlineSpan::Presented {
            presentation: InlinePresentation {
                foreground: Some([0x11, 0x22, 0x33]),
                background: Some([0xaa, 0xbb, 0xcc]),
                underline: true,
            },
            ..
        })
    ));
    assert!(doc.blocks.iter().any(|block| {
        matches!(block,
            Block::Presented {
                presentation: BlockPresentation { alignment: BlockAlignment::Center, indent_level: 1 },
                block,
            } if matches!(block.as_ref(), Block::Heading { .. }))
    }));
    assert!(doc.blocks.iter().any(|block| {
        matches!(block,
            Block::Presented {
                presentation: BlockPresentation { alignment: BlockAlignment::Center, .. },
                block,
            } if matches!(block.as_ref(), Block::Table { .. }))
    }));
    assert!(
        !doc.diagnostics.iter().any(|diagnostic| matches!(diagnostic,
            DocumentDiagnostic::UnsupportedConstruct(message)
                if message.contains("colors, underline, alignment, or section indentation"))),
        "captured reader presentation is now carried through the document model"
    );
}

#[test]
fn invalid_or_contextless_native_targets_do_not_gain_authority() {
    assert_eq!(
        resolve_target("file:///tmp/index.mu", ":/page/next.mu"),
        None
    );
    assert_eq!(resolve_target(BASE, ":/page/a\0b"), None);
    assert_eq!(resolve_target(BASE, "#section"), None);
    assert_eq!(resolve_target(BASE, "next.mu"), None);
}
