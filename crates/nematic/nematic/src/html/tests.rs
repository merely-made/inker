// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

fn render(body: &str) -> EngineDocument {
    HtmlFragmentEngine::new()
        .render(
            &EngineInput::new("https://clips.test/article", body).with_content_type("text/html"),
        )
        .expect("render HTML")
}

#[test]
fn lowers_reader_structure_and_links() {
    let doc = render(
        r#"<article lang="en">
                <h1>Reader <em>view</em></h1>
                <p>A <strong>safe</strong> <a href="/source">link</a>.</p>
                <ul><li>one</li><li>two</li></ul>
                <table><tr><th>Name</th><th>Value</th></tr>
                  <tr><td>A</td><td>42</td></tr></table>
                <img src="https://clips.test/image.png" alt="diagram">
            </article>"#,
    );

    assert_eq!(doc.title.as_deref(), Some("Reader view"));
    assert_eq!(doc.outgoing_links(), vec!["/source"]);
    assert!(matches!(doc.blocks[0], Block::Heading { level: 1, .. }));
    assert!(
        doc.blocks
            .iter()
            .any(|block| matches!(block, Block::List { items, .. } if items.len() == 2))
    );
    assert!(doc.blocks.iter().any(|block| matches!(block, Block::Table { header, rows, .. } if header.len() == 2 && rows.len() == 1)));
    assert!(
        doc.blocks
            .iter()
            .any(|block| matches!(block, Block::Image { alt, .. } if alt == "diagram"))
    );
}

#[test]
fn strips_active_elements_handlers_styles_and_unsafe_urls() {
    let doc = render(
        r#"<h1 onclick="steal()" style="display:none">Visible</h1>
                <script>SECRET_SCRIPT()</script>
                <style>.steal { background: url(secret) }</style>
                <iframe srcdoc="<p>SECRET_FRAME</p>"></iframe>
                <form><input value="SECRET_FORM"><button>SECRET_BUTTON</button></form>
                <p><a href="javascript:SECRET_LINK()">keep this text</a></p>
                <img src="data:text/html,SECRET_IMAGE" alt="unsafe">"#,
    );
    let flattened = format!("{doc:?}");

    assert!(flattened.contains("Visible"));
    assert!(flattened.contains("keep this text"));
    for forbidden in [
        "SECRET_SCRIPT",
        "SECRET_FRAME",
        "SECRET_FORM",
        "SECRET_BUTTON",
        "SECRET_LINK",
        "SECRET_IMAGE",
        "display:none",
        "background:",
        "onclick",
    ] {
        assert!(
            !flattened.contains(forbidden),
            "{forbidden} survived: {flattened}"
        );
    }
    assert_eq!(doc.outgoing_links(), Vec::<&str>::new());
    assert!(!doc.diagnostics.is_empty());
}
