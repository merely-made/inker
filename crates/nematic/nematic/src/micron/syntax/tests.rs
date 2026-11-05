// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

#[test]
fn guide_style_fixture_matches_independent_renderer_character_attributes() {
    const SOURCE: &str =
        include_str!("../../../tests/fixtures/micron/nomadnet-1.4.2/guide-stable.mu");
    const RECEIPT: &str =
        include_str!("../../../tests/fixtures/micron/nomadnet-1.4.2/guide-stable.go.json");
    let expected: serde_json::Value = serde_json::from_str(RECEIPT).unwrap();
    let document = parse(SOURCE);
    let lines: Vec<_> = document
        .lines
        .iter()
        .filter(|line| !matches!(line.kind, LineKind::Comment | LineKind::LiteralDelimiter))
        .collect();
    let expected = expected["lines"].as_array().unwrap();
    assert_eq!(
        lines.len(),
        expected.len(),
        "physical line visibility agrees with the independent renderer"
    );
    for (index, (line, expected)) in lines.iter().zip(expected).enumerate() {
        let alignment = match line.alignment {
            Alignment::Default | Alignment::Left => "left",
            Alignment::Center => "center",
            Alignment::Right => "right",
        };
        assert_eq!(
            alignment,
            expected["align"].as_str().unwrap(),
            "line {index}"
        );
        let mut actual_characters = Vec::new();
        for span in &line.spans {
            let Span::Text { text, style } = span else {
                panic!("unexpected control in style-only fixture")
            };
            let foreground = style
                .foreground
                .map(|[r, g, b]| format!("#{r:02x}{g:02x}{b:02x}"))
                .unwrap_or("#dddddd".into());
            let background = style
                .background
                .map(|[r, g, b]| format!("#{r:02x}{g:02x}{b:02x}"))
                .unwrap_or("default".into());
            for character in text.chars() {
                actual_characters.push((
                    character,
                    style.bold,
                    style.italic,
                    style.underline,
                    foreground.clone(),
                    background.clone(),
                ));
            }
        }
        let mut expected_characters = Vec::new();
        for span in expected["spans"].as_array().unwrap() {
            for character in span["text"].as_str().unwrap().chars() {
                expected_characters.push((
                    character,
                    span["bold"].as_bool().unwrap(),
                    span["italic"].as_bool().unwrap(),
                    span["underline"].as_bool().unwrap(),
                    span["fg"].as_str().unwrap().to_owned(),
                    span["bg"].as_str().unwrap().to_owned(),
                ));
            }
        }
        assert_eq!(actual_characters, expected_characters, "line {index}");
    }
}

#[test]
fn guide_controls_keep_combined_style_and_reset() {
    let doc = parse("\u{60}!bold \u{60}*both\u{60}* bold\nstill bold\u{60}\u{60} plain");
    let Span::Text { style, text } = &doc.lines[0].spans[1] else {
        panic!("text")
    };
    assert_eq!(text, "both");
    assert!(style.bold && style.italic);
    let Span::Text { style, .. } = &doc.lines[1].spans[0] else {
        panic!("text")
    };
    assert!(style.bold && !style.italic);
    let Span::Text { style, .. } = &doc.lines[1].spans[1] else {
        panic!("text")
    };
    assert_eq!(*style, Style::default());
}

#[test]
fn link_effects_keep_request_selectors_distinct_from_page_anchors() {
    let doc = parse(
        "\u{60}[Read\u{60}:/page/read.mu] \u{60}[Send\u{60}:/page/action.mu\u{60}name|action=view]\n\u{60}[Conclusion\u{60}0123456789abcdef0123456789abcdef:/page/read.mu\u{60}anchor=conclusion]",
    );
    assert!(matches!(&doc.lines[0].spans[0],
        Span::Link { target, effect: None, .. } if target == ":/page/read.mu"));
    assert!(matches!(&doc.lines[0].spans[2],
        Span::Link { effect: Some(LinkEffect::RequestSelectors(fields)), .. } if fields == "name|action=view"));
    assert!(matches!(&doc.lines[1].spans[0],
        Span::Link { effect: Some(LinkEffect::Anchor(name)), .. } if name == "conclusion"));
}

#[test]
fn guide_field_forms_are_typed_and_invalid_options_stay_inert() {
    let doc = parse(
        "\u{60}<name\u{60}Hello>\n\u{60}<16|small\u{60}>\n\u{60}<40x5|notes\u{60}Hello>\n\u{60}<!|secret\u{60}hidden>\n\u{60}<!32|masked\u{60}hidden>\n\u{60}<?|choice|green|*\u{60}>\n\u{60}<^|colour|blue\u{60}>\n\u{60}<bogus|\u{60}value>",
    );
    assert!(matches!(&doc.lines[0].spans[0],
        Span::Field { kind: FieldKind::Text { name, width: None, rows: None, masked: false }, initial, .. }
            if name == "name" && initial == "Hello"));
    assert!(matches!(&doc.lines[1].spans[0],
        Span::Field { kind: FieldKind::Text { name, width: Some(16), rows: None, masked: false }, initial, .. }
            if name == "small" && initial.is_empty()));
    assert!(matches!(&doc.lines[2].spans[0],
        Span::Field { kind: FieldKind::Text { name, width: Some(40), rows: Some(5), masked: false }, .. }
            if name == "notes"));
    assert!(matches!(&doc.lines[3].spans[0],
        Span::Field { kind: FieldKind::Text { name, width: None, rows: None, masked: true }, .. }
            if name == "secret"));
    assert!(matches!(&doc.lines[4].spans[0],
        Span::Field { kind: FieldKind::Text { name, width: Some(32), rows: None, masked: true }, .. }
            if name == "masked"));
    assert!(matches!(&doc.lines[5].spans[0],
        Span::Field { kind: FieldKind::Checkbox { name, value, prechecked: true }, initial, .. }
            if name == "choice" && value == "green" && initial.is_empty()));
    assert!(matches!(&doc.lines[6].spans[0],
        Span::Field { kind: FieldKind::Radio { name, value, prechecked: false }, .. }
            if name == "colour" && value == "blue"));
    assert!(matches!(&doc.lines[7].spans[0], Span::Unsupported(_)));
}

#[test]
fn literal_table_and_image_boundaries_retain_source() {
    let source = "\u{60}=\n\u{60}!literal\n\u{60}=\n\u{60}tc30\n| A | B |\n| --- | --- |\n| x | y |\n\u{60}t\n\u{60}(A picture\u{60}w=n\u{60}a=c\u{60}:/media/demo.webp)";
    let doc = parse(source);
    assert_eq!(doc.source, source);
    assert_eq!(doc.lines[0].kind, LineKind::LiteralDelimiter);
    assert_eq!(doc.lines[1].kind, LineKind::Literal);
    assert_eq!(doc.lines[2].kind, LineKind::LiteralDelimiter);
    assert!(
        matches!(&doc.lines[3].kind, LineKind::Table { options, body } if options == "c30" && body.contains("| x | y |"))
    );
    assert!(
        matches!(&doc.lines[4].kind, LineKind::Image { target, alt, .. } if target == ":/media/demo.webp" && alt == "A picture")
    );
}

#[test]
fn empty_and_unterminated_controls_are_inert_without_panics() {
    for source in [
        "",
        "\u{60}",
        "\u{60}[",
        "\u{60}<name",
        "\u{60}F猫",
        "\u{60}{",
        "\u{60}t\nunclosed",
        "\u{60}[label\u{60}]",
        "\u{60}[label\u{60}\u{60}]",
        "\u{60}{}",
        "\u{60}(alt\u{60})",
    ] {
        let doc = parse(source);
        assert_eq!(doc.source, source);
        assert!(
            !doc.lines
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| matches!(span, Span::Link { .. }))
        );
    }
}

#[test]
fn crlf_preserves_document_source_while_control_lexemes_parse() {
    let source = "\u{60}=\r\n\u{60}!literal\r\n\u{60}=\r\n\u{60}t\r\n| A | B |\r\n| --- | --- |\r\n| x | y |\r\n\u{60}t\r\n---\r\n";
    let doc = parse(source);
    assert_eq!(doc.source, source);
    assert_eq!(doc.lines[0].kind, LineKind::LiteralDelimiter);
    assert_eq!(doc.lines[1].kind, LineKind::Literal);
    assert!(matches!(&doc.lines[3].kind, LineKind::Table { body, .. } if body.contains("\r\n")));
    assert_eq!(doc.lines[4].kind, LineKind::Divider);
}

#[test]
fn heading_anchors_follow_visible_text_and_first_declaration_wins() {
    let doc = parse(">First Heading!\n\u{60}:first-heading\n>First Heading!\n\u{60}:later\n>Later");
    assert!(matches!(&doc.lines[0].kind,
        LineKind::Heading { anchor: Some(anchor), .. } if anchor == "first-heading"));
    assert!(
        matches!(&doc.lines[1].spans[0], Span::Anchor { name, active: false } if name == "first-heading")
    );
    assert!(matches!(
        &doc.lines[2].kind,
        LineKind::Heading { anchor: None, .. }
    ));
    assert!(
        matches!(&doc.lines[3].spans[0], Span::Anchor { name, active: true } if name == "later")
    );
    assert!(matches!(
        &doc.lines[4].kind,
        LineKind::Heading { anchor: None, .. }
    ));
}

#[test]
fn table_inline_state_flows_into_following_document_lines() {
    let doc = parse(include_str!(
        "../../../tests/fixtures/micron/nomadnet-1.4.2/probe-table-style-state.mu"
    ));
    assert!(
        doc.lines[1].style.bold,
        "table inherits the entering bold state"
    );
    let Span::Text { style, text } = &doc.lines[2].spans[0] else {
        panic!("text")
    };
    assert_eq!(text, "after");
    assert!(
        !style.bold,
        "cell control changes state for the next document line"
    );
}

#[test]
fn table_anchor_reserves_name_before_later_heading() {
    let doc = parse("\u{60}t\n| Name |\n| --- |\n| \u{60}:later Here |\n\u{60}t\n>Later");
    assert!(matches!(
        doc.lines[1].kind,
        LineKind::Heading { anchor: None, .. }
    ));
    assert_eq!(
        doc.lines[0].spans,
        vec![Span::Anchor {
            name: "later".into(),
            active: true
        }],
        "the cell anchor is retained on the table line"
    );
}

#[test]
fn anchor_shaped_variable_in_request_list_stays_a_request() {
    let doc = parse("\u{60}[Go\u{60}:/page/action.mu\u{60}anchor=here|username]");
    assert!(matches!(&doc.lines[0].spans[0],
        Span::Link { effect: Some(LinkEffect::RequestSelectors(fields)), .. } if fields == "anchor=here|username"));
}
