/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;
#[test]
fn passage_survives_type_width_and_cold_appearance() {
    let body=(0..60).map(|i|format!("Paragraph {i}: a distinct passage about graph projections and the relationship between the map and reading.\n\n")).collect::<String>();
    let mut doc = SmolwebDocument::from_document_with_theme(
        EngineDocument {
            address: "gemini://example.test/anchors".into(),
            title: None,
            content_type: "text/plain".into(),
            lang: None,
            provenance: Default::default(),
            trust: Default::default(),
            diagnostics: Vec::new(),
            navigation: Default::default(),
            blocks: body
                .split("\n\n")
                .filter(|s| !s.is_empty())
                .map(|text| Block::Paragraph {
                    spans: vec![inker::InlineSpan::Text(text.into())],
                })
                .collect(),
        },
        SmolwebTheme::default(),
    );
    let _ = doc.frame(700, 300);
    doc.scroll_to(1500.);
    let anchor = doc.reading_anchor().expect("a laid-out passage");
    doc.set_reading_preferences(crate::ReadingPreferences {
        text_size: 24,
        column_em: 28,
    })
    .unwrap();
    let _ = doc.frame(440, 300);
    let resolved = crate::reading_anchor::resolve(
        &doc.document.address,
        &doc.layout.as_ref().unwrap().searchable,
        &anchor,
        1.,
    )
    .unwrap();
    assert!((doc.scroll_y() - resolved).abs() < 1.0);
    let after = doc.reading_anchor().unwrap();
    let _ = doc.frame(880, 600);
    let resolved = crate::reading_anchor::resolve(
        &doc.document.address,
        &doc.layout.as_ref().unwrap().searchable,
        &after,
        1.,
    )
    .unwrap();
    assert!((doc.scroll_y() - resolved).abs() < 1.0);
    let saved = doc.reading_anchor().unwrap();
    let mut cold = doc.new_appearance();
    cold.set_reading_preferences(doc.reading_preferences())
        .unwrap();
    cold.restore_reading_anchor(saved);
    let _ = cold.frame(880, 600);
    assert!((cold.scroll_y() - doc.scroll_y()).abs() < 1.0);
    assert!(std::sync::Arc::ptr_eq(
        &doc.source_document(),
        &cold.source_document()
    ));
}
