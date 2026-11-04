// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

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
        other => panic!("expected navigation link, got {other:?}"),
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
fn in_page_span_in_a_stale_table_is_an_inert_link_region() {
    let packet = layout_document(
        &doc(vec![Block::Paragraph {
            spans: vec![InlineSpan::InPage {
                target: inker::InPageTarget {
                    fragment: Some("setup".into()),
                    block: Some(0),
                },
                spans: vec![InlineSpan::Text("Setup".into())],
            }],
        }]),
        viewport(),
        &DocumentStyleSheet::default(),
    )
    .packet;
    let [region] = packet.interactions.as_slice() else {
        panic!("one region: {:?}", packet.interactions);
    };
    assert_eq!(
        region.kind,
        InteractionKind::InPage {
            block: None,
            fragment: None
        },
        "the default table does not describe these blocks"
    );
    assert!(region.link_semantics.is_some());
    let (x, y) = (region.bounds.origin.x + 1.0, region.bounds.origin.y + 1.0);
    assert_eq!(packet.link_at(x, y), None);
}
