// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

#[test]
fn generic_font_batch_refuses_before_mutating_existing_context() {
    let bytes =
        include_bytes!("../../../../cambium/examples/genet_web_smoke/assets/Roboto-Regular.ttf")
            .to_vec();
    let mut env = LayoutEnvironment::new();
    env.font_cx.collection =
        parley::fontique::Collection::new(parley::fontique::CollectionOptions {
            shared: false,
            system_fonts: false,
        });
    register_generic_fonts(&mut env, &[("sans-serif".into(), bytes.clone())]).unwrap();
    let prior = env
        .font_cx
        .collection
        .generic_families(GenericFamily::SansSerif)
        .collect::<Vec<_>>();
    assert!(!prior.is_empty());
    assert_eq!(
        register_generic_fonts(
            &mut env,
            &[("system-ui".into(), bytes), ("sans-serif".into(), vec![0])]
        ),
        Err(GenericFontError::InvalidFont)
    );
    assert!(
        env.font_cx
            .collection
            .generic_families(GenericFamily::SystemUi)
            .next()
            .is_none()
    );
    assert_eq!(
        env.font_cx
            .collection
            .generic_families(GenericFamily::SansSerif)
            .collect::<Vec<_>>(),
        prior
    );
}

#[test]
fn explicit_code_family_is_the_actual_shaping_request() {
    let mut env = LayoutEnvironment::new();
    let flat = flatten_inline(
        &[InlineSpan::Text("WiWi".into())],
        &LinkAdornment::None,
        &LinkAdornment::None,
        None,
    );
    let shape = |env: &mut LayoutEnvironment, monospace: bool| {
        let base = TextBaseStyle {
            font_family: "serif".into(),
            monospace,
            ..Default::default()
        };
        let mut fonts = FontInterner::new();
        let run = layout_text_block(
            env,
            &flat,
            &base,
            [0.0; 4],
            [0.0; 4],
            400.0,
            Point::new(0.0, 0.0),
            &mut fonts,
        );
        let table = fonts.into_table();
        let face = table.get(run.glyph_runs[0].font_face).unwrap();
        (
            run.total_size,
            face.data.id(),
            face.index,
            run.glyph_runs[0].font_family.clone(),
        )
    };
    let normal = shape(&mut env, false);
    let code = shape(&mut env, true);
    assert_eq!(
        normal, code,
        "configured code family reaches the same actual shaped face and metrics"
    );
}

fn link(url: &str) -> InlineSpan {
    InlineSpan::Link {
        url: url.into(),
        title: None,
        spans: vec![InlineSpan::Text("label".into())],
        predicate: None,
    }
}

#[test]
fn no_adornment_leaves_link_text_unprefixed() {
    let f = flatten_inline(
        &[link("gemini://x/")],
        &LinkAdornment::None,
        &LinkAdornment::None,
        Some("gemini"),
    );
    assert_eq!(f.text, "label");
    assert_eq!(f.links.len(), 1);
    assert_eq!(f.links[0].range, 0..5);
}

#[test]
fn in_protocol_link_gets_rightwards_double_arrow() {
    let f = flatten_inline(
        &[link("gemini://x/")],
        &LinkAdornment::SchemeArrow,
        &LinkAdornment::None,
        Some("gemini"),
    );
    assert!(f.text.starts_with("\u{21d2} "), "got {:?}", f.text);
    // The arrow is part of the link: the link byte range covers the whole
    // "⇒ label" string.
    assert_eq!(f.links.len(), 1);
    assert_eq!(&f.text[f.links[0].range.clone()], f.text.as_str());
    assert_eq!(f.links[0].accessible_label, "label");
}

#[test]
fn external_link_gets_single_rightwards_arrow() {
    let f = flatten_inline(
        &[link("https://x/")],
        &LinkAdornment::SchemeArrow,
        &LinkAdornment::None,
        Some("gemini"),
    );
    assert!(f.text.starts_with("\u{2192} "), "got {:?}", f.text);
}

#[test]
fn relative_link_is_in_protocol() {
    let f = flatten_inline(
        &[link("/page")],
        &LinkAdornment::SchemeArrow,
        &LinkAdornment::None,
        Some("gemini"),
    );
    assert!(f.text.starts_with("\u{21d2} "), "got {:?}", f.text);
}

#[test]
fn adornment_prefix_carries_link_style() {
    let f = flatten_inline(
        &[link("https://x/")],
        &LinkAdornment::SchemeArrow,
        &LinkAdornment::None,
        Some("gemini"),
    );
    // Every style range over the link (the arrow prefix + the label) is a
    // link, and one of them begins at byte 0 (the prefix).
    assert!(f.styles.iter().all(|(_, s)| s.link), "all link-styled");
    assert!(
        f.styles.iter().any(|(r, _)| r.start == 0),
        "prefix styled from 0"
    );
}

#[test]
fn in_page_link_flattens_as_an_in_protocol_link() {
    let span = InlineSpan::InPage {
        target: inker::InPageTarget {
            fragment: Some("setup".into()),
            block: Some(0),
        },
        spans: vec![InlineSpan::Text("Setup".into())],
    };
    let f = flatten_inline(
        &[span],
        &LinkAdornment::None,
        &LinkAdornment::SchemeArrow,
        Some("gemini"),
    );
    assert_eq!(f.text, "\u{21d2} Setup", "in-protocol arrow");
    assert!(f.styles.iter().all(|(_, style)| style.link), "link style");
    let [link] = f.links.as_slice() else {
        panic!("one link annotation: {:?}", f.links);
    };
    assert_eq!(link.accessible_label, "Setup");
    assert_eq!(
        link.kind,
        InteractionKind::InPage {
            block: Some(0),
            fragment: Some("setup".into()),
        }
    );
}

#[test]
fn in_page_and_network_adornments_apply_independently_in_one_run() {
    let spans = [
        link("/page"),
        InlineSpan::InPage {
            target: inker::InPageTarget::default(),
            spans: vec![InlineSpan::Text("jump".into())],
        },
    ];
    let prefixes = |link: &LinkAdornment, in_page: &LinkAdornment| {
        let f = flatten_inline(&spans, link, in_page, Some("gemini"));
        f.links
            .iter()
            .map(|l| f.text[l.range.start..l.range.end - l.accessible_label.len()].to_owned())
            .collect::<Vec<_>>()
    };
    let (on, off) = (LinkAdornment::SchemeArrow, LinkAdornment::None);
    let arrow = on.prefix_for("#", Some("gemini")).unwrap();
    assert_eq!(prefixes(&on, &off), [arrow, ""], "network only");
    assert_eq!(prefixes(&off, &on), ["", arrow], "in-page only");

    // P1c: each token carries its own glyph, verbatim, beside the other's arrow.
    let (hop, leave) = (
        LinkAdornment::Glyph("\u{00a7} ".into()),
        LinkAdornment::Glyph("<> ".into()),
    );
    assert_eq!(prefixes(&on, &hop), [arrow, "\u{00a7} "], "in-page glyph");
    assert_eq!(prefixes(&leave, &on), ["<> ", arrow], "network glyph");
    assert_eq!(prefixes(&leave, &hop), ["<> ", "\u{00a7} "], "both");
    let empty = LinkAdornment::Glyph(String::new());
    assert_eq!(
        prefixes(&empty, &empty),
        prefixes(&off, &off),
        "empty is none"
    );
}
