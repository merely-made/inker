// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! One feed entry as its own document (smolweb fidelity plan WS4, R4; Mere
//! stack seams ruling S75).
//!
//! An entry that carries its body is addressed as the feed address with the
//! entry's guid as its fragment. Asked for that address, the feed engine
//! renders the entry alone: its body lowered through the html-fragment
//! engine's passive subset, the article URL its canonical link. Without a
//! body, the document is the summary and a link out.

use errand::parse::feed::FeedEntry;
use inker::{
    Block, DocumentDiagnostic, DocumentProvenance, DocumentTrustState, EngineDocument, EngineInput,
    InlineSpan,
};

/// The address of `guid`'s own document within the feed at `feed_address`.
pub(super) fn entry_address(feed_address: &str, guid: &str) -> String {
    let base = feed_address.split('#').next().unwrap_or(feed_address);
    format!("{base}#{}", encode_fragment(guid))
}

/// The entry an address names by its fragment, if the address has one and an
/// entry's guid matches it.
pub(super) fn entry_for<'a>(address: &str, entries: &'a [FeedEntry]) -> Option<&'a FeedEntry> {
    let (_, fragment) = address.split_once('#')?;
    let guid = decode_fragment(fragment);
    entries
        .iter()
        .find(|entry| entry.guid.as_deref() == Some(guid.as_str()))
}

/// Render `entry` as its own document at `input.address`.
pub(super) fn render(input: &EngineInput, entry: &FeedEntry, engine_id: &str) -> EngineDocument {
    let title = entry.title.clone().unwrap_or_default();
    let mut blocks = Vec::new();
    if !title.is_empty() {
        blocks.push(Block::Heading {
            level: 1,
            spans: vec![InlineSpan::Text(title.clone())],
        });
    }
    for (label, value) in [("Published", &entry.published), ("Updated", &entry.updated)] {
        if let Some(value) = value {
            blocks.push(Block::MetadataRow {
                label: label.into(),
                value: value.clone(),
            });
        }
    }
    let mut diagnostics = Vec::new();
    let base = entry.link.as_deref().unwrap_or(&input.address);
    match entry
        .content
        .as_deref()
        .and_then(|html| body_blocks(html, base, &mut diagnostics))
    {
        Some(body) => blocks.extend(body),
        None => {
            if let Some(summary) = &entry.summary {
                blocks.push(Block::Paragraph {
                    spans: vec![InlineSpan::Text(summary.clone())],
                });
            }
        },
    }
    if let Some(url) = &entry.link {
        blocks.push(Block::Paragraph {
            spans: vec![InlineSpan::Link {
                url: url.clone(),
                title: None,
                spans: vec![InlineSpan::Text("Open the article".into())],
                predicate: None,
            }],
        });
    }
    EngineDocument {
        address: input.address.clone(),
        title: (!title.is_empty()).then_some(title),
        content_type: "text/html".into(),
        lang: None,
        provenance: DocumentProvenance {
            source_kind: Some(engine_id.into()),
            canonical_uri: Some(entry.link.clone().unwrap_or_else(|| input.address.clone())),
            fetched_at: None,
            source_label: None,
        },
        trust: DocumentTrustState::Unknown,
        diagnostics,
        navigation: Default::default(),
        blocks,
    }
}

/// The body's blocks through the html-fragment engine, resolving against
/// `base`; its diagnostics join the document's.
#[cfg(feature = "html-fragment")]
fn body_blocks(
    html: &str,
    base: &str,
    diagnostics: &mut Vec<DocumentDiagnostic>,
) -> Option<Vec<Block>> {
    use inker::Engine as _;
    let document = crate::HtmlFragmentEngine::new()
        .render(&EngineInput::new(base, html))
        .ok()?;
    diagnostics.extend(document.diagnostics);
    let mut blocks = document.blocks;
    // The body is the article's, so its relative links are too; the host would
    // otherwise resolve them against the feed's address.
    if let Ok(base) = url::Url::parse(base) {
        resolve_blocks(&mut blocks, &base);
    }
    Some(blocks)
}

/// Resolve every link and image URL in `blocks` against `base`.
#[cfg(feature = "html-fragment")]
fn resolve_blocks(blocks: &mut [Block], base: &url::Url) {
    for block in blocks {
        match block {
            Block::Presented { block, .. } => {
                resolve_blocks(std::slice::from_mut(&mut **block), base)
            },
            Block::Heading { spans, .. } | Block::Paragraph { spans } => resolve_spans(spans, base),
            Block::Quote { blocks } => resolve_blocks(blocks, base),
            Block::List { items, .. } => {
                for item in items {
                    resolve_blocks(item, base);
                }
            },
            Block::Table { header, rows, .. } => {
                for cell in header.iter_mut().chain(rows.iter_mut().flatten()) {
                    resolve_spans(cell, base);
                }
            },
            Block::Image { url, .. } => resolve_url(url, base),
            // The html-fragment engine emits none of the other kinds.
            _ => {},
        }
    }
}

#[cfg(feature = "html-fragment")]
fn resolve_spans(spans: &mut [InlineSpan], base: &url::Url) {
    for span in spans {
        match span {
            InlineSpan::Link { url, spans, .. } => {
                resolve_url(url, base);
                resolve_spans(spans, base);
            },
            InlineSpan::Presented { spans, .. }
            | InlineSpan::Emphasis(spans)
            | InlineSpan::Strong(spans)
            | InlineSpan::Submit { spans, .. }
            | InlineSpan::InPage { spans, .. } => resolve_spans(spans, base),
            InlineSpan::Text(_)
            | InlineSpan::Code(_)
            | InlineSpan::LineBreak
            | InlineSpan::SoftBreak => {},
        }
    }
}

#[cfg(feature = "html-fragment")]
fn resolve_url(href: &mut String, base: &url::Url) {
    if let Ok(resolved) = base.join(href) {
        *href = resolved.into();
    }
}

/// Without the html-fragment engine there is no passive lowering to trust,
/// so the entry falls back to its summary.
#[cfg(not(feature = "html-fragment"))]
fn body_blocks(
    _html: &str,
    _base: &str,
    diagnostics: &mut Vec<DocumentDiagnostic>,
) -> Option<Vec<Block>> {
    diagnostics.push(DocumentDiagnostic::UnsupportedConstruct(
        "feed entry body (nematic built without html-fragment)".into(),
    ));
    None
}

/// Percent-encode everything outside RFC 3986's unreserved set.
fn encode_fragment(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Decode `%XX` escapes; a malformed escape stays as written.
fn decode_fragment(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let hex = |byte: u8| (byte as char).to_digit(16).map(|digit| digit as u8);
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(high << 4 | low);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragments_round_trip_awkward_guids() {
        for guid in [
            "post-1",
            "urn:uuid:1225c695-cfb8",
            "tag:x.test,2026:/a b#c",
            "é",
        ] {
            let address = entry_address("gemini://x.test/feed.xml#old", guid);
            assert!(address.starts_with("gemini://x.test/feed.xml#"));
            assert_eq!(address.matches('#').count(), 1, "{address}");
            let (_, fragment) = address.split_once('#').unwrap();
            assert_eq!(decode_fragment(fragment), guid);
        }
        assert_eq!(decode_fragment("100%"), "100%", "a bare percent stays");
        assert_eq!(
            decode_fragment("%é"),
            "%é",
            "a percent before a wide char stays"
        );
    }
}
