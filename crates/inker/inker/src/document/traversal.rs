//! Inline and link traversal over blocks and spans.

use super::*;

// =============================================================================
// Inline / link traversal helpers
// =============================================================================

/// Flatten a span list into a plain-text string suitable for accessible
/// names and snippet extraction. `Code` contributes its inner text; styling
/// containers (`Emphasis`/`Strong`) and `Link` contribute their inner span
/// text recursively. `SoftBreak` becomes a space, `LineBreak` becomes a
/// newline.
pub fn inline_text(spans: &[InlineSpan]) -> String {
    let mut out = String::new();
    for span in spans {
        append_inline_text(span, &mut out);
    }
    out
}

pub(super) fn append_inline_text(span: &InlineSpan, out: &mut String) {
    match span {
        InlineSpan::Text(text) | InlineSpan::Code(text) => out.push_str(text),
        InlineSpan::Presented { spans, .. }
        | InlineSpan::Emphasis(spans)
        | InlineSpan::Strong(spans) => {
            for inner in spans {
                append_inline_text(inner, out);
            }
        },
        InlineSpan::Link { spans, .. }
        | InlineSpan::Submit { spans, .. }
        | InlineSpan::InPage { spans, .. } => {
            for inner in spans {
                append_inline_text(inner, out);
            }
        },
        InlineSpan::SoftBreak => out.push(' '),
        InlineSpan::LineBreak => out.push('\n'),
    }
}

pub(super) fn collect_block_spans<'a>(block: &'a Block, out: &mut Vec<&'a InlineSpan>) {
    match block {
        Block::Presented { block, .. } => collect_block_spans(block, out),
        Block::Heading { spans, .. } | Block::Paragraph { spans } => {
            for span in spans {
                out.push(span);
            }
        },
        Block::Quote { blocks } => {
            for inner in blocks {
                collect_block_spans(inner, out);
            }
        },
        Block::List { items, .. } => {
            for item in items {
                for inner in item {
                    collect_block_spans(inner, out);
                }
            }
        },
        Block::Table { header, rows, .. } => {
            for cell in header.iter().chain(rows.iter().flatten()) {
                for span in cell {
                    out.push(span);
                }
            }
        },
        Block::Menu { rows } => {
            for row in rows {
                for span in &row.label {
                    out.push(span);
                }
            }
        },
        Block::CodeBlock { .. }
        | Block::Image { .. }
        | Block::Preformatted { .. }
        | Block::Rule
        | Block::FeedHeader { .. }
        | Block::FeedEntry { .. }
        | Block::MetadataRow { .. }
        | Block::Badge { .. } => {},
    }
}

pub(super) fn collect_block_link_urls<'a>(block: &'a Block, out: &mut Vec<&'a str>) {
    match block {
        Block::Presented { block, .. } => collect_block_link_urls(block, out),
        Block::Heading { spans, .. } | Block::Paragraph { spans } => {
            for span in spans {
                collect_link_urls(span, out);
            }
        },
        Block::Quote { blocks } => {
            for inner in blocks {
                collect_block_link_urls(inner, out);
            }
        },
        Block::List { items, .. } => {
            for item in items {
                for inner in item {
                    collect_block_link_urls(inner, out);
                }
            }
        },
        Block::FeedHeader { source_url, .. } => {
            if let Some(url) = source_url {
                out.push(url.as_str());
            }
        },
        Block::FeedEntry {
            article_url,
            source_url,
            content_address,
            ..
        } => {
            if let Some(url) = article_url {
                out.push(url.as_str());
            }
            if let Some(url) = source_url {
                out.push(url.as_str());
            }
            if let Some(url) = content_address {
                out.push(url.as_str());
            }
        },
        Block::Table { header, rows, .. } => {
            for cell in header.iter().chain(rows.iter().flatten()) {
                for span in cell {
                    collect_link_urls(span, out);
                }
            }
        },
        Block::Menu { rows } => {
            for row in rows {
                if let Some(url) = &row.target {
                    out.push(url.as_str());
                }
                for span in &row.label {
                    collect_link_urls(span, out);
                }
            }
        },
        // `Block::Image` is deliberately NOT a link. An image `url` is a
        // subresource this document wants fetched and painted in place, not
        // a destination the reader can navigate to, and the sole production
        // consumer of `outgoing_links` is `DocumentSession::inspect` ->
        // `ContentReport.links`, whose contract reads "outgoing `<a href>`
        // targets". Image prefetching has its own channel:
        // `DocumentSession::subresources` / `provide_subresource`, whose
        // smolweb implementation walks `Block::Image` itself and never calls
        // `outgoing_links`. Image sources were briefly collected here (genet
        // 1f3fc462, "Add host-driven smolweb inline images") as a side effect
        // of adding that channel; nothing consumed the mixed set.
        Block::CodeBlock { .. }
        | Block::Image { .. }
        | Block::Preformatted { .. }
        | Block::Rule
        | Block::MetadataRow { .. }
        | Block::Badge { .. } => {},
    }
}

pub(super) fn collect_link_urls<'a>(span: &'a InlineSpan, out: &mut Vec<&'a str>) {
    match span {
        InlineSpan::Link { url, spans, .. } => {
            out.push(url.as_str());
            for inner in spans {
                collect_link_urls(inner, out);
            }
        },
        InlineSpan::Presented { spans, .. }
        | InlineSpan::Emphasis(spans)
        | InlineSpan::Strong(spans)
        | InlineSpan::Submit { spans, .. }
        | InlineSpan::InPage { spans, .. } => {
            for inner in spans {
                collect_link_urls(inner, out);
            }
        },
        InlineSpan::Text(_)
        | InlineSpan::Code(_)
        | InlineSpan::LineBreak
        | InlineSpan::SoftBreak => {},
    }
}
