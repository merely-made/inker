// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The links an HTML page carries, resolved against its address. Extraction
//! only: what a caller does with them (a crawl frontier, a host's suggestions)
//! is the caller's policy. Builds for wasm, unlike `crawl`.

/// The page's links, resolved against `page_url` and deduplicated in document
/// order, for an HTML or XHTML `content_type`; empty for any other type or an
/// unparsable address. Fragment-only links to the page itself are dropped.
pub fn html_links(page_url: &str, content_type: Option<&str>, body: &str) -> Vec<String> {
    let Some(xml) = html_syntax(content_type) else {
        return Vec::new();
    };
    let Ok(base) = url::Url::parse(page_url) else {
        return Vec::new();
    };
    let document = if xml {
        genet_static_dom::StaticDocument::parse_xml(body)
    } else {
        genet_static_dom::StaticDocument::parse(body)
    };
    let mut seen = std::collections::HashSet::new();
    crate::extract_links(&document)
        .into_iter()
        .filter_map(|link| base.join(&link.href).ok())
        .filter(|url| {
            let mut bare = url.clone();
            bare.set_fragment(None);
            let mut page = base.clone();
            page.set_fragment(None);
            bare != page
        })
        .map(|url| url.to_string())
        .filter(|url| seen.insert(url.clone()))
        .collect()
}

/// `Some(true)` for XHTML, `Some(false)` for HTML, `None` otherwise.
pub(crate) fn html_syntax(content_type: Option<&str>) -> Option<bool> {
    let media_type = content_type?.split(';').next()?.trim();
    if media_type.eq_ignore_ascii_case("text/html") {
        Some(false)
    } else if media_type.eq_ignore_ascii_case("application/xhtml+xml") {
        Some(true)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_resolve_dedupe_and_skip_the_page_itself() {
        let html = r##"<p><a href="/b">b</a> <a href="c">c</a> <a href="#top">top</a>
            <a href="/b">again</a> <a href="https://other.example/">o</a></p>"##;
        assert_eq!(
            html_links(
                "https://a.example/dir/page",
                Some("text/html; charset=utf-8"),
                html
            ),
            [
                "https://a.example/b",
                "https://a.example/dir/c",
                "https://other.example/"
            ]
        );
        assert!(html_links("https://a.example/", Some("text/plain"), html).is_empty());
        assert!(html_links("not a url", Some("text/html"), html).is_empty());
    }
}
