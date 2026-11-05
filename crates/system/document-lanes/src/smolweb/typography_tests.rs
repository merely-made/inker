/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;
#[test]
fn prose_measure_reflows_to_the_appearance_without_changing_its_source() {
    let source = Arc::new(EngineDocument {
        address: "mere://reading/test".into(),
        title: Some("Reading".into()),
        content_type: "text/plain".into(),
        lang: None,
        provenance: Default::default(),
        trust: Default::default(),
        diagnostics: Vec::new(),
        navigation: Default::default(),
        blocks: vec![Block::Paragraph {
            spans: vec![inker::InlineSpan::Text(
                "A measured line keeps the next sentence in reach. ".repeat(80),
            )],
        }],
    });
    let mut doc =
        SmolwebDocument::from_shared_document_with_theme(source.clone(), SmolwebTheme::Plain);
    let code = doc.style_sheet().resolve(document_canvas::BlockRole::Code);
    doc.set_font_family("serif");
    assert_eq!(
        doc.style_sheet().resolve(document_canvas::BlockRole::Code),
        code,
        "a prose face cannot replace the code role"
    );
    doc.frame(1200, 600);
    let wide = doc.layout.as_ref().unwrap().packet.clone();
    let paragraph = &wide.blocks[0].bounds;
    assert!(
        paragraph.origin.x > 200.0,
        "wide reading is centered with a bounded measure"
    );
    assert!(paragraph.size.width < 750.0);
    doc.frame(320, 600);
    let narrow = &doc.layout.as_ref().unwrap().packet;
    assert!(
        narrow.content_bounds.size.height > wide.content_bounds.size.height,
        "narrow reading must reflow"
    );
    for block in &narrow.blocks {
        assert!(block.bounds.origin.x >= 0.0);
        assert!(
            block.bounds.origin.x + block.bounds.size.width <= 320.1,
            "prose does not require horizontal scrolling"
        );
    }
    assert!(Arc::ptr_eq(&source, &doc.document));
}

#[test]
fn restyling_reflows_without_replacing_source_or_session_state() {
    let source = Arc::new(EngineDocument {
        address: "gemini://x.test/".into(),
        title: Some("Held content".into()),
        content_type: "text/gemini".into(),
        lang: None,
        provenance: Default::default(),
        trust: Default::default(),
        diagnostics: Vec::new(),
        navigation: Default::default(),
        blocks: vec![Block::Paragraph {
            spans: vec![
                inker::InlineSpan::Text("A retained paragraph. ".repeat(120)),
                inker::InlineSpan::Link {
                    url: "gemini://x.test/next".into(),
                    title: None,
                    spans: vec![inker::InlineSpan::Text("Next".into())],
                    predicate: None,
                },
            ],
        }],
    });
    let mut doc = SmolwebDocument::from_shared_document_with_theme(source, SmolwebTheme::Plain);
    doc.frame(300, 200);
    doc.focus_move(SessionFocusDirection::Forward, 300, 200);
    doc.scroll_by(0.0, 40.0);
    let focus = doc.focus.clone();
    let source = doc.document.clone();
    let scroll = doc.scroll_y;
    let folds = doc.folds.clone();
    let old_height = doc
        .layout
        .as_ref()
        .unwrap()
        .packet
        .content_bounds
        .size
        .height;
    let mut style = doc.style_sheet().clone();
    style.roles.body.size = document_canvas::SizeSpec::Absolute(24.0);
    style.roles.body.line_height_ratio = Some(1.8);
    assert!(doc.set_style_sheet(style.clone()));
    assert!(doc.layout.is_none());
    assert!(Arc::ptr_eq(&source, &doc.document));
    assert_eq!(doc.scroll_y, scroll);
    assert_eq!(doc.folds, folds);
    assert_eq!(doc.focus, focus);
    doc.frame(300, 200);
    assert!(
        doc.layout
            .as_ref()
            .unwrap()
            .packet
            .content_bounds
            .size
            .height
            > old_height
    );
    let body_size = |doc: &SmolwebDocument| {
        doc.layout
            .as_ref()
            .unwrap()
            .packet
            .blocks
            .iter()
            .find_map(|block| match &block.kind {
                document_canvas::RenderedBlockKind::Text { glyph_runs } => {
                    glyph_runs.first().map(|run| run.font_size)
                },
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(
        body_size(&doc),
        24.0,
        "hosts which did not opt into reading controls retain explicit styles"
    );
    let mut other = doc.new_appearance();
    let mut preferences = tabard::ReadingPreferences::default();
    preferences.step_text(true);
    doc.set_reading_preferences(preferences).unwrap();
    doc.frame(300, 200);
    other.frame(300, 200);
    assert_eq!(body_size(&doc), 20.0);
    assert_eq!(
        body_size(&other),
        24.0,
        "a fresh projection retains the host baseline until it selects reading intent"
    );
    assert!(!doc.set_style_sheet(style));
}
