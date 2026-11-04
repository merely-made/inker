// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

#[test]
fn role_leading_is_optional_and_old_styles_round_trip_unchanged() {
    let baseline = DocumentStyleSheet::default();
    let json = serde_json::to_string(&baseline).unwrap();
    let loaded: DocumentStyleSheet = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded, baseline);
    assert!(
        !serde_json::to_string(&baseline.roles.body)
            .unwrap()
            .contains("line_height_ratio")
    );
    let mut edited = baseline.clone();
    edited.roles.body.line_height_ratio = Some(1.8);
    edited.roles.heading.line_height_ratio = Some(1.1);
    assert_eq!(edited.resolve(BlockRole::Body).line_height_ratio, 1.8);
    assert_eq!(edited.resolve(BlockRole::Heading(3)).line_height_ratio, 1.1);
    assert_eq!(
        edited.resolve(BlockRole::Code),
        baseline.resolve(BlockRole::Code)
    );
    assert_eq!(
        serde_json::from_str::<DocumentStyleSheet>(&serde_json::to_string(&edited).unwrap())
            .unwrap(),
        edited
    );
}

// --- The default sheet reproduces the built-in document look ---
//
// These assertions pin the exact values the layout renderers carried
// before the sheet existed (the byte-identical contract for the routing
// that consumes them in `layout.rs`).

#[test]
fn body_role_matches_paragraph_defaults() {
    let sheet = DocumentStyleSheet::default();
    let r = sheet.resolve(BlockRole::Body);
    assert_eq!(r.font_size, 14.0);
    assert_eq!(r.font_family, "system-ui");
    assert!(!r.bold);
    assert!(!r.italic);
    assert!(!r.monospace);
    assert_eq!(r.line_height_ratio, 1.4);
    assert_eq!(r.spacing_above, 0.0);
    assert_eq!(r.spacing_below, 12.0);
    assert_eq!(r.color, ColorVocabulary::default().body_text);
    assert_eq!(r.wrap, WrapPolicy::Wrap);
}

#[test]
fn heading_role_matches_heading_defaults_every_level() {
    let sheet = DocumentStyleSheet::default();
    let expected = [28.0, 22.0, 18.0, 16.0, 14.0, 12.0];
    for level in 1..=6u8 {
        let r = sheet.resolve(BlockRole::Heading(level));
        assert_eq!(r.font_size, expected[level as usize - 1], "h{level} size");
        assert_eq!(r.font_family, "system-ui");
        assert!(r.bold, "h{level} bold");
        assert!(!r.italic);
        assert!(!r.monospace);
        assert_eq!(r.spacing_above, 16.0);
        assert_eq!(r.spacing_below, 8.0);
        assert_eq!(r.color, ColorVocabulary::default().heading_text);
    }
}

#[test]
fn heading_level_clamps_to_h1_h6() {
    let sheet = DocumentStyleSheet::default();
    // Level 0 clamps to h1, level 7+ clamps to h6.
    assert_eq!(sheet.resolve(BlockRole::Heading(0)).font_size, 28.0);
    assert_eq!(sheet.resolve(BlockRole::Heading(9)).font_size, 12.0);
    assert_eq!(sheet.heading_size(0), 28.0);
    assert_eq!(sheet.heading_size(99), 12.0);
}

#[test]
fn code_role_matches_code_block_defaults() {
    let sheet = DocumentStyleSheet::default();
    let r = sheet.resolve(BlockRole::Code);
    assert_eq!(r.font_size, 14.0);
    assert_eq!(r.font_family, "monospace");
    assert!(r.monospace);
    assert!(!r.bold);
    assert!(!r.italic);
    assert_eq!(r.spacing_below, 12.0);
    assert_eq!(r.color, ColorVocabulary::default().code_text);
}

#[test]
fn metadata_role_matches_metadata_row_defaults() {
    let sheet = DocumentStyleSheet::default();
    let r = sheet.resolve(BlockRole::Metadata);
    assert_eq!(r.font_size, 14.0);
    assert_eq!(r.font_family, "system-ui");
    assert!(!r.bold);
    assert!(!r.italic);
    assert!(!r.monospace);
    assert_eq!(r.spacing_below, 6.0);
    assert_eq!(r.color, ColorVocabulary::default().body_text);
}

#[test]
fn badge_role_matches_badge_defaults() {
    let sheet = DocumentStyleSheet::default();
    let r = sheet.resolve(BlockRole::Badge);
    // The renderer used `body_font_size * 0.85`; ScaleOfBody must match it
    // bit-for-bit.
    assert_eq!(r.font_size, 14.0 * 0.85);
    assert_eq!(r.font_family, "system-ui");
    assert!(r.italic);
    assert!(!r.bold);
    assert!(!r.monospace);
    assert_eq!(r.spacing_below, 6.0);
    assert_eq!(r.color, ColorVocabulary::default().badge_text);
}

#[test]
fn block_spacing_equals_paragraph_spacing() {
    let sheet = DocumentStyleSheet::default();
    assert_eq!(sheet.block_spacing(), 12.0);
}

#[test]
fn globals_are_built_in_defaults() {
    let sheet = DocumentStyleSheet::default();
    assert_eq!(sheet.body_font_size, 14.0);
    assert_eq!(sheet.body_font_family, "system-ui");
    assert_eq!(sheet.mono_font_family, "monospace");
    assert_eq!(sheet.line_height_ratio, 1.4);
    assert_eq!(sheet.indent_per_level, 24.0);
    assert_eq!(sheet.horizontal_padding, 16.0);
    assert_eq!(sheet.max_content_width, None);
    assert_eq!(sheet.vertical_padding, 16.0);
    assert_eq!(sheet.colors, ColorVocabulary::default());
}

#[test]
fn line_height_uses_the_shared_ratio() {
    let sheet = DocumentStyleSheet::default();
    assert_eq!(sheet.line_height(14.0), 14.0 * 1.4);
}

#[test]
fn explicit_family_and_absolute_size_resolve() {
    let mut sheet = DocumentStyleSheet::default();
    sheet.roles.body.family = FontChoice::Explicit("Iosevka".into());
    sheet.roles.body.size = SizeSpec::Absolute(20.0);
    let r = sheet.resolve(BlockRole::Body);
    assert_eq!(r.font_family, "Iosevka");
    assert_eq!(r.font_size, 20.0);
    assert!(!r.monospace, "explicit family is not implicitly monospace");
}

#[test]
fn in_page_link_adornment_defaults_to_the_link_adornment() {
    let sheet = DocumentStyleSheet::default();
    assert_eq!(sheet.link_adornment, LinkAdornment::SchemeArrow);
    assert_eq!(sheet.in_page_link_adornment, sheet.link_adornment);
    // A sheet serialized before the token existed reads back unchanged.
    let mut json = serde_json::to_value(&sheet).expect("serialize");
    json.as_object_mut()
        .unwrap()
        .remove("in_page_link_adornment")
        .expect("field present");
    let back: DocumentStyleSheet = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, sheet);
}

#[test]
fn glyph_adornment_prefixes_every_link_verbatim() {
    let glyph = LinkAdornment::Glyph("\u{00a7} ".into());
    let links = [
        ("https://x/", Some("gemini")),
        ("gemini://x/", Some("gemini")),
        ("/page", Some("gemini")),
        ("#", None),
    ];
    for (url, base) in links {
        assert_eq!(glyph.prefix_for(url, base), Some("\u{00a7} "), "{url}");
    }
    let arrow = LinkAdornment::SchemeArrow;
    assert_ne!(
        arrow.prefix_for("https://x/", Some("gemini")),
        arrow.prefix_for("/page", Some("gemini")),
        "control: these links take different arrows"
    );
    let empty = LinkAdornment::Glyph(String::new());
    assert_eq!(empty.prefix_for("/page", None), None, "empty is no prefix");
}

#[test]
fn link_adornment_serde_keeps_old_spellings_and_round_trips_glyphs() {
    let variants = [
        LinkAdornment::None,
        LinkAdornment::SchemeArrow,
        LinkAdornment::Glyph("\u{00a7} ".into()),
    ];
    for adornment in &variants {
        let json = serde_json::to_string(adornment).expect("serialize");
        let back: LinkAdornment = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(&back, adornment, "{json}");
    }
    // Sheets saved before the glyph variant spell the unit variants bare.
    let spelled = |adornment: &LinkAdornment| serde_json::to_value(adornment).unwrap();
    assert_eq!(spelled(&LinkAdornment::None), "None");
    assert_eq!(spelled(&LinkAdornment::SchemeArrow), "SchemeArrow");
    let mut old = serde_json::to_value(DocumentStyleSheet::default()).expect("serialize");
    old["link_adornment"] = "None".into();
    old["in_page_link_adornment"] = "SchemeArrow".into();
    let loaded: DocumentStyleSheet = serde_json::from_value(old.clone()).expect("old sheet");
    assert_eq!(loaded.link_adornment, LinkAdornment::None);
    assert_eq!(loaded.in_page_link_adornment, LinkAdornment::SchemeArrow);
    old["link_adornment"] = "Arrow".into();
    assert!(
        serde_json::from_value::<DocumentStyleSheet>(old).is_err(),
        "control: a spelling the enum never had is refused"
    );

    let mut themed = DocumentStyleSheet::default();
    themed.link_adornment = LinkAdornment::Glyph("<> ".into());
    themed.in_page_link_adornment = LinkAdornment::Glyph("\u{00a7} ".into());
    let json = serde_json::to_string(&themed).expect("serialize");
    let back: DocumentStyleSheet = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, themed);
}

#[test]
fn sheet_round_trips_through_serde() {
    let sheet = DocumentStyleSheet::default();
    let json = serde_json::to_string(&sheet).expect("serialize");
    let back: DocumentStyleSheet = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(sheet, back);
}
