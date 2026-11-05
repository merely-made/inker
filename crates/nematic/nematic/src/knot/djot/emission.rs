// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

/// Serialize document blocks back into a djot knot body — the dual of
/// [`parse_djot_knot_body`] (design doc §10.5 Phase 2). Semantic blocks emit as
/// the native djot constructs they parsed from: `MetadataRow` → a definition
/// list, `FeedEntry` / `FeedHeader` / `Badge` → an attributed div. Round-trips
/// on the recognized subset (see `round_trip_preserves_semantic_blocks`);
/// byte-faithful protocol-fence preservation is a later phase.
pub fn blocks_to_djot(blocks: &[Block]) -> String {
    let mut out = String::new();
    for block in blocks {
        if !out.is_empty() {
            out.push('\n');
        }
        emit_block(block, &mut out);
    }
    out
}

/// Write a djot pipe table: header row, a `---` separator, then body rows. Cells
/// flatten to inline text (the round-trip writer; the parser side that produces
/// `Table` lands with the live tile).
pub(super) fn emit_djot_table(
    header: &[Vec<InlineSpan>],
    rows: &[Vec<Vec<InlineSpan>>],
    out: &mut String,
) {
    let cols = header
        .len()
        .max(rows.iter().map(Vec::len).max().unwrap_or(0));
    if cols == 0 {
        return;
    }
    let emit_row = |cells: &[Vec<InlineSpan>], out: &mut String| {
        out.push('|');
        for i in 0..cols {
            out.push(' ');
            out.push_str(&cells.get(i).map(|c| inline_text(c)).unwrap_or_default());
            out.push_str(" |");
        }
        out.push('\n');
    };
    emit_row(header, out);
    out.push('|');
    for _ in 0..cols {
        out.push_str(" --- |");
    }
    out.push('\n');
    for r in rows {
        emit_row(r, out);
    }
}

pub(super) fn emit_block(block: &Block, out: &mut String) {
    match block {
        Block::Presented { block, .. } => emit_block(block, out),
        Block::Heading { level, spans } => {
            for _ in 0..(*level).max(1) {
                out.push('#');
            }
            out.push(' ');
            out.push_str(&inline_text(spans));
            out.push('\n');
        },
        Block::Paragraph { spans } => {
            out.push_str(&inline_text(spans));
            out.push('\n');
        },
        Block::Table { header, rows, .. } => {
            emit_djot_table(header, rows, out);
        },
        // Djot has no menu construct: write the rows as the paragraphs and
        // preformatted runs every menu-less format reads.
        Block::Menu { rows } => {
            for block in inker::menu_fallback_blocks(rows) {
                emit_block(&block, out);
            }
        },
        Block::MetadataRow { label, value } => {
            out.push_str(": ");
            out.push_str(label);
            out.push_str("\n\n  ");
            out.push_str(value);
            out.push('\n');
        },
        Block::Badge { text } => {
            out.push_str("::: badge\n");
            out.push_str(text);
            out.push_str("\n:::\n");
        },
        Block::FeedEntry {
            title,
            date,
            summary,
            article_url,
            ..
        } => {
            emit_div_attrs(out, title, article_url.as_deref(), date.as_deref());
            out.push_str("::: feed-entry\n");
            if let Some(s) = summary {
                out.push_str(s);
                out.push('\n');
            }
            out.push_str(":::\n");
        },
        Block::FeedHeader {
            title,
            summary,
            source_url,
            ..
        } => {
            emit_div_attrs(out, title, source_url.as_deref(), None);
            out.push_str("::: feed-header\n");
            if let Some(s) = summary {
                out.push_str(s);
                out.push('\n');
            }
            out.push_str(":::\n");
        },
        Block::CodeBlock { language, text } => {
            out.push_str("```");
            if let Some(lang) = language {
                out.push_str(lang);
            }
            out.push('\n');
            out.push_str(text);
            if !text.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("```\n");
        },
        Block::Preformatted { text } => {
            out.push_str("```\n");
            out.push_str(text);
            if !text.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("```\n");
        },
        Block::Quote { blocks } => {
            let mut inner = String::new();
            for (i, b) in blocks.iter().enumerate() {
                if i > 0 {
                    inner.push('\n');
                }
                emit_block(b, &mut inner);
            }
            for line in inner.lines() {
                out.push_str("> ");
                out.push_str(line);
                out.push('\n');
            }
        },
        Block::List { ordered, items } => {
            for item in items {
                let mut inner = String::new();
                for (i, b) in item.iter().enumerate() {
                    if i > 0 {
                        inner.push(' ');
                    }
                    emit_block(b, &mut inner);
                }
                out.push_str(if *ordered { "1. " } else { "- " });
                out.push_str(inner.trim());
                out.push('\n');
            }
        },
        Block::Image { url, alt } => {
            out.push_str("![");
            out.push_str(alt);
            out.push_str("](");
            out.push_str(url);
            out.push_str(")\n");
        },
        Block::Rule => out.push_str("----\n"),
        // A kind this serializer does not know yet stays visible, named.
        other => {
            out.push_str("::: unsupported\n");
            out.push_str(other.kind_name());
            out.push_str("\n:::\n");
        },
    }
}

pub(super) fn emit_div_attrs(out: &mut String, title: &str, url: Option<&str>, date: Option<&str>) {
    out.push_str("{title=\"");
    out.push_str(title);
    out.push('"');
    if let Some(url) = url {
        out.push_str(" url=\"");
        out.push_str(url);
        out.push('"');
    }
    if let Some(date) = date {
        out.push_str(" date=\"");
        out.push_str(date);
        out.push('"');
    }
    out.push_str("}\n");
}
