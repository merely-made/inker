// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

#[test]
fn parses_webview2_quoted_json_result() {
    let payload = serde_json::json!({
        "ok": true,
        "title": "Chosen section",
        "text": "  First line\n\nSecond line  ",
        "selector": "main > article",
        "links": ["https://example.test/a"]
    })
    .to_string();
    let raw = serde_json::to_string(&payload).expect("quoted result");
    let clip = parse_web_clip(&raw, "https://example.test").expect("clip");
    assert_eq!(clip.title.as_deref(), Some("Chosen section"));
    assert_eq!(clip.text, "First line\nSecond line");
    assert_eq!(clip.links, vec!["https://example.test/a"]);
}

#[test]
fn web_clip_script_uses_cursor_coordinates() {
    let script = web_clip_script(12, 34);
    assert!(script.contains("const rawX = 12;"));
    assert!(script.contains("const rawY = 34;"));
    assert!(script.contains("document.elementFromPoint"));
}

#[test]
fn markdown_fallback_preserves_blocks_and_links() {
    let mut registry = EngineRegistry::new();
    for engine in nematic::engines() {
        registry.register(engine);
    }
    let clip = fragment_from_body(
        "https://example.test/note.md",
        None,
        Some("text/markdown"),
        "# Heading\n\nA [reference](https://example.test/ref).",
        &registry,
        &EngineRoutePolicy::default(),
    );

    let blocks = clip.blocks.as_ref().expect("parsed blocks");
    assert!(matches!(blocks.first(), Some(Block::Heading { .. })));
    assert_eq!(clip.links, vec!["https://example.test/ref"]);
    assert!(clip.text.contains("Heading"));
    assert!(clip.text.contains("reference"));
}

#[test]
fn micron_clip_preserves_text_and_links_inside_source_presentation() {
    let mut registry = EngineRegistry::new();
    for engine in nematic::engines() {
        registry.register(engine);
    }
    // The host explicitly admits Micron for this native node address.
    let mut policy = EngineRoutePolicy::default();
    policy.fallback.engine_id = nematic::micron::ENGINE_MICRON.into();
    let clip = fragment_from_body(
        "0123456789abcdef0123456789abcdef:/page/index.mu",
        None,
        Some("text/x-micron"),
        "`c`Ff00`_`[Reference`:/page/ref.mu]\n",
        &registry,
        &policy,
    );
    assert_eq!(clip.text, "Reference");
    assert_eq!(
        clip.links,
        vec!["0123456789abcdef0123456789abcdef:/page/ref.mu"]
    );
    assert!(
        clip.blocks
            .as_ref()
            .unwrap()
            .iter()
            .any(|block| { matches!(block, Block::Presented { .. }) })
    );
}

#[test]
fn html_fallback_extracts_main_text_through_fleece() {
    let clip = fragment_from_body(
        "https://example.test/article",
        Some("Fallback title".to_string()),
        Some("text/html; charset=utf-8"),
        r#"<nav>Site chrome</nav><main><h1>Article heading</h1><p>The retained article paragraph.</p></main><footer>Footer chrome</footer>"#,
        &EngineRegistry::new(),
        &EngineRoutePolicy::default(),
    );

    assert!(clip.blocks.is_none());
    assert!(clip.text.contains("Article heading"));
    assert!(clip.text.contains("The retained article paragraph."));
    assert!(!clip.text.contains("Site chrome"));
    assert!(!clip.text.contains("Footer chrome"));
}

#[test]
fn micron_in_page_link_clips_as_label_text_not_a_link() {
    let mut registry = EngineRegistry::new();
    for engine in nematic::engines() {
        registry.register(engine);
    }
    let mut policy = EngineRoutePolicy::default();
    policy.fallback.engine_id = nematic::micron::ENGINE_MICRON.into();
    let clip = fragment_from_body(
        "0123456789abcdef0123456789abcdef:/page/index.mu",
        None,
        Some("text/x-micron"),
        "`[Reference`:/page/ref.mu] `[Setup notes`#setup]\n>Setup\n",
        &registry,
        &policy,
    );
    assert!(clip.text.contains("Setup notes"), "{}", clip.text);
    assert_eq!(
        clip.links,
        vec!["0123456789abcdef0123456789abcdef:/page/ref.mu"]
    );
    assert!(!clip.links.iter().any(|link| link.contains("#setup")));
}

#[test]
fn submission_target_is_not_imported_as_a_navigation_link() {
    let blocks = vec![Block::Paragraph {
        spans: vec![InlineSpan::Submit {
            target: "spartan://example.test/write".to_string(),
            spans: vec![InlineSpan::Link {
                url: "gemini://example.test/help".to_string(),
                title: None,
                spans: vec![InlineSpan::Text("help".to_string())],
                predicate: None,
            }],
        }],
    }];

    assert_eq!(block_links(&blocks), vec!["gemini://example.test/help"]);
}

#[test]
fn cropped_visual_uses_element_rect() {
    let rgba = vec![255u8; 4 * 4 * 4];
    let png = encode_png_rgba(&rgba, 4, 4).expect("png");
    let mut clip = ClipFragment {
        source_url: "https://example.test/post".to_string(),
        title: Some("Visual".to_string()),
        text: "A useful paragraph.".to_string(),
        html: None,
        selector: None,
        links: Vec::new(),
        rect: Some(ClipRect {
            x: 1.0,
            y: 1.0,
            width: 2.0,
            height: 2.0,
        }),
        visual: None,
        blocks: None,
    };

    assert!(attach_cropped_visual(&mut clip, &png, (4, 4)));
    let visual = clip.visual.as_ref().expect("visual");
    assert_eq!((visual.width, visual.height), (2, 2));
    assert!(visual.data_uri.starts_with("data:image/png;base64,"));
}
