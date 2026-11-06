// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! A reader article out of supplied HTML or XHTML.
//!
//! It moved here from gazette on 2026-11-03: it parses through Genet's static
//! DOM and asks Fleece's extractor, which is this crate's job, and it keeps
//! gazette-ledger's dependencies to published crates.

use crate::{Article, extract_article};

/// Extract a reader article from supplied HTML without fetching or storing it.
///
/// Gazette's future poller owns response acquisition and persistence. This helper
/// only admits declared HTML, parses it through the static DOM, and asks Fleece
/// whether the document has a readable article.
pub fn article_from_html(content_type: Option<&str>, html: &str) -> Option<Article> {
    let document = match html_syntax(content_type)? {
        HtmlSyntax::Html => genet_static_dom::StaticDocument::parse(html),
        HtmlSyntax::Xhtml => genet_static_dom::StaticDocument::parse_xml(html),
    };
    extract_article(&document)
}

/// The title and visible body text of supplied HTML or XHTML, without
/// fetching or storing it: what a native "keep page text" records. Declared
/// HTML only, parsed through Genet's static DOM; script and style are not
/// text. The caller bounds the result.
pub fn page_text_from_html(
    content_type: Option<&str>,
    html: &str,
) -> Option<(Option<String>, String)> {
    let document = match html_syntax(content_type)? {
        HtmlSyntax::Html => genet_static_dom::StaticDocument::parse(html),
        HtmlSyntax::Xhtml => genet_static_dom::StaticDocument::parse_xml(html),
    };
    let title = crate::extract_title(&document).filter(|title| !title.trim().is_empty());
    Some((title, crate::extract_text(&document)))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HtmlSyntax {
    Html,
    Xhtml,
}

fn html_syntax(content_type: Option<&str>) -> Option<HtmlSyntax> {
    let media_type = content_type?.split(';').next()?.trim();
    if media_type.eq_ignore_ascii_case("text/html") {
        Some(HtmlSyntax::Html)
    } else if media_type.eq_ignore_ascii_case("application/xhtml+xml") {
        Some(HtmlSyntax::Xhtml)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supplied_html_article_extraction_is_readable_only() {
        let article = article_from_html(
            Some("text/html; charset=utf-8"),
            r#"<!doctype html><title>Field notes</title><main><article>
                <h1>Field notes</h1>
                <p>The first paragraph carries enough prose to be a reader article.</p>
                <p>The second paragraph confirms this is content, not site chrome.</p>
            </article></main>"#,
        )
        .expect("readable supplied HTML should produce an article");
        assert_eq!(article.title.as_deref(), Some("Field notes"));
        assert!(!article.blocks.is_empty());

        assert!(article_from_html(Some("text/html"), "<nav><a href='/'>Home</a></nav>").is_none());
    }

    #[test]
    fn supplied_html_page_text_is_the_visible_body_without_script_or_style() {
        let (title, text) = page_text_from_html(
            Some("text/html; charset=utf-8"),
            "<!doctype html><title>Field notes</title><style>.x{color:red}</style>\
             <body><p>Visible lighthouse prose.</p><script>var hidden = 1;</script></body>",
        )
        .expect("declared HTML yields text");
        assert_eq!(title.as_deref(), Some("Field notes"));
        assert!(text.contains("Visible lighthouse prose."));
        assert!(!text.contains("hidden") && !text.contains("color:red"), "{text}");
        assert!(page_text_from_html(Some("application/json"), "{}").is_none());
    }

    #[test]
    fn supplied_non_html_never_enters_the_reader_extractor() {
        assert!(
            article_from_html(
                Some("application/json"),
                r#"{"title":"not HTML","body":"<main>not parsed</main>"}"#,
            )
            .is_none()
        );
    }

    #[test]
    fn supplied_xhtml_uses_the_declared_xml_syntax() {
        assert_eq!(
            html_syntax(Some("application/xhtml+xml; charset=utf-8")),
            Some(HtmlSyntax::Xhtml),
        );
        let article = article_from_html(
            Some("application/xhtml+xml"),
            r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><title>XML field notes</title></head><body><main><article><h1>XML field notes</h1><p>The first paragraph carries enough prose to be a reader article.</p><p>The second paragraph confirms this is structured XHTML content.</p></article></main></body></html>"#,
        )
        .expect("declared XHTML should produce an article without a sniff marker");
        assert_eq!(article.title.as_deref(), Some("XML field notes"));
    }
}
