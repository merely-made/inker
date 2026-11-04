// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Top-level document layout — dispatches per [`Block`] variant
//! and stacks the resulting blocks vertically inside the viewport.
//!
//! v1 is a simple top-down stack with no float / no inline-image flow /
//! no scrolling. Width fills the available content width; height grows
//! to fit content (may exceed `viewport.height`).

use parley::segmenter_dictionaries::{DictionaryBlob, SegmenterDictionaries};
use std::collections::HashMap;

use inker::{
    Block, BlockAlignment, EngineDocument, FoldState, InlineSpan, MenuRow, TableAlignment,
};

use crate::font_table::{FontInterner, FontTable};
use crate::style_sheet::{
    BlockRole, ColorToken, DocumentStyleSheet, ResolvedBlockStyle, SourcePresentation,
};
use crate::text::{
    Flattened, LaidOutText, LayoutEnvironment, TextBaseStyle, flatten_inline,
    layout_text_block_with_link_identity_base,
};
use crate::types::{
    DocumentRenderPacket, InteractionKind, InteractionRegion, LinkSemantics, Point, Rect,
    RenderedBlock, RenderedBlockKind, SemanticInteractionId, Size, Viewport,
};

/// A laid-out document: the serializable [`DocumentRenderPacket`] plus the
/// out-of-band [`FontTable`] sidecar carrying the `parley::FontData` each
/// `GlyphRun` was shaped against. The packet alone is the portable,
/// serializable shape; `fonts` holds `Arc`-cheap face handles whose owned
/// bytes materialize only at the `paint_list_api` boundary (the
/// `PaintList`'s font side-table). See [`crate::font_table`].
#[derive(Clone, Debug)]
pub struct LaidOutDocument {
    pub searchable: Vec<crate::text::SearchableText>,
    pub packet: DocumentRenderPacket,
    pub fonts: FontTable,
    /// Segmenter dictionaries the text met and the layout did not have; a
    /// host that loads them and lays out again breaks those scripts properly.
    pub wanted_dictionaries: SegmenterDictionaries,
}

/// Layout entry point. Consumes an `EngineDocument` and a viewport,
/// produces a [`LaidOutDocument`]: a portable [`DocumentRenderPacket`]
/// downstream renderers paint, plus the font sidecar that resolves each
/// run's [`FontFaceId`](crate::FontFaceId) to the real face bytes parley
/// shaped against.
///
/// Collapsible sections take their authored state; see
/// [`layout_document_with_folds`].
pub fn layout_document(
    document: &EngineDocument,
    viewport: Viewport,
    style: &DocumentStyleSheet,
) -> LaidOutDocument {
    layout_document_with_folds(document, viewport, style, &FoldState::default())
}

/// Lay out under a reader's fold state. Blocks inside a closed extent are
/// skipped but still reserve their link identities, so identity does not
/// depend on fold state. Each collapsible heading gains its style-sheet
/// marker and an [`InteractionKind::Fold`] region. A navigation table that
/// no longer describes the blocks is ignored.
pub fn layout_document_with_folds(
    document: &EngineDocument,
    viewport: Viewport,
    style: &DocumentStyleSheet,
    folds: &FoldState,
) -> LaidOutDocument {
    layout_document_with_folds_and_fonts(document, viewport, style, folds, &[])
}

/// Lay out with explicit host-held fonts, including browser hosts without
/// system font access. The selected face bytes travel with the glyph packet.
pub fn layout_document_with_folds_and_fonts(
    document: &EngineDocument,
    viewport: Viewport,
    style: &DocumentStyleSheet,
    folds: &FoldState,
    fonts: &[Vec<u8>],
) -> LaidOutDocument {
    layout_document_with_folds_and_font_resources(document, viewport, style, folds, fonts, &[], &[])
        .expect("empty generic font ledger")
}

/// Lay out with explicit host generic bindings. The selected bytes ride in
/// the ordinary font sidecar; no authored family/style is rewritten.
/// `dictionaries` are the segmenter dictionaries the host has loaded (see
/// [`LaidOutDocument::wanted_dictionaries`]).
pub fn layout_document_with_folds_and_font_resources(
    document: &EngineDocument,
    viewport: Viewport,
    style: &DocumentStyleSheet,
    folds: &FoldState,
    fonts: &[Vec<u8>],
    generic_fonts: &[(String, Vec<u8>)],
    dictionaries: &[DictionaryBlob],
) -> Result<LaidOutDocument, crate::text::GenericFontError> {
    let mut env = LayoutEnvironment::new();
    crate::text::register_generic_fonts(&mut env, generic_fonts)?;
    for blob in dictionaries {
        env.layout_cx.load_segmenter_dictionaries(blob);
    }
    for bytes in fonts {
        env.font_cx.collection.register_fonts(
            parley::fontique::Blob::new(std::sync::Arc::new(bytes.clone())),
            None,
        );
    }
    // The document's own scheme classifies its links as in-protocol vs
    // external for the `⇒` / `→` adornment.
    let base_scheme = crate::style_sheet::url_scheme(&document.address).map(str::to_string);
    let navigation = &document.navigation;
    let current = navigation.is_current(&document.blocks);
    let mut layouter = DocumentLayouter::new(viewport, style, &mut env, base_scheme, current);
    let (hidden, headings) = if current {
        let headings: HashMap<usize, (usize, bool)> = (0..navigation.folds.len())
            .map(|fold| {
                let open = folds.is_open(navigation, fold);
                (navigation.folds[fold].heading, (fold, open))
            })
            .collect();
        (folds.hidden(navigation), headings)
    } else {
        Default::default()
    };

    for (idx, block) in document.blocks.iter().enumerate() {
        if hidden.iter().any(|range| range.contains(&idx)) {
            let count = layouter.link_identities(block);
            layouter.reserve_link_identities(count);
        } else {
            layouter.fold_heading = headings.get(&idx).copied();
            layouter.lay_out_block(block, idx, 0);
            layouter.fold_heading = None;
        }
    }

    Ok(layouter.finish())
}

/// Build the parley block base from a resolved role style: the role's
/// typography, base text `color`, and `wrap` policy.
/// Spans with every soft break made a line break (`SoftBreak::Preserve`).
fn preserve_soft_breaks(spans: &[InlineSpan]) -> Vec<InlineSpan> {
    spans
        .iter()
        .map(|span| match span {
            InlineSpan::SoftBreak => InlineSpan::LineBreak,
            InlineSpan::Presented {
                presentation,
                spans,
            } => InlineSpan::Presented {
                presentation: *presentation,
                spans: preserve_soft_breaks(spans),
            },
            InlineSpan::Emphasis(spans) => InlineSpan::Emphasis(preserve_soft_breaks(spans)),
            InlineSpan::Strong(spans) => InlineSpan::Strong(preserve_soft_breaks(spans)),
            InlineSpan::Link {
                url,
                title,
                spans,
                predicate,
            } => InlineSpan::Link {
                url: url.clone(),
                title: title.clone(),
                spans: preserve_soft_breaks(spans),
                predicate: predicate.clone(),
            },
            InlineSpan::Submit { target, spans } => InlineSpan::Submit {
                target: target.clone(),
                spans: preserve_soft_breaks(spans),
            },
            InlineSpan::InPage { target, spans } => InlineSpan::InPage {
                target: target.clone(),
                spans: preserve_soft_breaks(spans),
            },
            InlineSpan::Text(_) | InlineSpan::Code(_) | InlineSpan::LineBreak => span.clone(),
        })
        .collect()
}

/// A menu's rows as spans for one monospace block: each row is its type
/// label padded to the widest label, a space, then its label, a link when the
/// row has a target. Info rows pad an empty label, so their text lines up with
/// the other rows' labels.
fn menu_spans(rows: &[MenuRow]) -> Vec<InlineSpan> {
    let width = rows
        .iter()
        .map(|row| row.kind.label().chars().count())
        .max()
        .unwrap_or(0);
    let mut spans = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            spans.push(InlineSpan::LineBreak);
        }
        spans.push(InlineSpan::Text(format!("{:<width$} ", row.kind.label())));
        match &row.target {
            Some(url) => spans.push(InlineSpan::Link {
                url: url.clone(),
                title: None,
                spans: row.label.clone(),
                predicate: None,
            }),
            None => spans.extend(row.label.iter().cloned()),
        }
    }
    spans
}

fn text_base_from(
    resolved: &ResolvedBlockStyle,
    alignment: BlockAlignment,
    source_presentation: SourcePresentation,
) -> TextBaseStyle {
    TextBaseStyle {
        font_size: resolved.font_size,
        font_family: resolved.font_family.clone(),
        bold: resolved.bold,
        italic: resolved.italic,
        monospace: resolved.monospace,
        line_height_ratio: resolved.line_height_ratio,
        color: resolved.color,
        wrap: resolved.wrap,
        alignment,
        source_presentation,
    }
}

struct DocumentLayouter<'a> {
    viewport: Viewport,
    style: &'a DocumentStyleSheet,
    env: &'a mut LayoutEnvironment,
    cursor_y: f32,
    blocks: Vec<RenderedBlock>,
    interactions: Vec<InteractionRegion>,
    searchable: Vec<crate::text::SearchableText>,
    max_x: f32,
    /// Interns parley's chosen face per run; sealed into the
    /// [`LaidOutDocument`]'s [`FontTable`] sidecar at `finish`.
    fonts: FontInterner,
    /// The document's own URL scheme, for classifying links (in-protocol vs
    /// external) when adorning them. `None` for schemeless addresses.
    base_scheme: Option<String>,
    /// Next opaque identity for a logical link in this lowered document.
    /// Wrapped rectangles reserve only one identity and share it.
    next_link_identity: SemanticInteractionId,
    alignment: BlockAlignment,
    /// Whether in-page targets index this document's blocks.
    in_page_current: bool,
    /// Fold index and state while laying out a collapsible heading.
    fold_heading: Option<(usize, bool)>,
}

impl<'a> DocumentLayouter<'a> {
    fn new(
        viewport: Viewport,
        style: &'a DocumentStyleSheet,
        env: &'a mut LayoutEnvironment,
        base_scheme: Option<String>,
        in_page_current: bool,
    ) -> Self {
        Self {
            viewport,
            style,
            env,
            cursor_y: style.vertical_padding,
            blocks: Vec::new(),
            interactions: Vec::new(),
            searchable: Vec::new(),
            max_x: 0.0,
            fonts: FontInterner::new(),
            base_scheme,
            next_link_identity: SemanticInteractionId::from_lowered_ordinal(1),
            alignment: BlockAlignment::Start,
            in_page_current,
            fold_heading: None,
        }
    }

    fn flatten(&self, spans: &[InlineSpan]) -> Flattened {
        let preserved;
        let spans = match self.style.soft_break {
            crate::style_sheet::SoftBreak::Reflow => spans,
            crate::style_sheet::SoftBreak::Preserve => {
                preserved = preserve_soft_breaks(spans);
                preserved.as_slice()
            },
        };
        let mut flattened = flatten_inline(
            spans,
            &self.style.link_adornment,
            &self.style.in_page_link_adornment,
            self.base_scheme.as_deref(),
        );
        if !self.in_page_current {
            // Indices from another document's table must not scroll this one.
            for link in &mut flattened.links {
                if let InteractionKind::InPage { block, fragment } = &mut link.kind {
                    *block = None;
                    *fragment = None;
                }
            }
        }
        flattened
    }

    /// The link identities `render_block` reserves for `block`.
    fn link_identities(&self, block: &Block) -> usize {
        let links = |spans: &[InlineSpan]| self.flatten(spans).links.len();
        match block {
            Block::Presented { block, .. } => self.link_identities(block),
            Block::Heading { spans, .. } | Block::Paragraph { spans } => links(spans),
            Block::Table { header, rows, .. } => header
                .iter()
                .chain(rows.iter().flatten())
                .map(|cell| links(cell))
                .sum(),
            Block::Quote { blocks } => blocks.iter().map(|b| self.link_identities(b)).sum(),
            Block::List { items, .. } => items
                .iter()
                .flatten()
                .map(|b| self.link_identities(b))
                .sum(),
            Block::Image { .. } => 1,
            Block::FeedHeader { source_url, .. } => usize::from(source_url.is_some()),
            Block::FeedEntry {
                article_url,
                source_url,
                content_address,
                ..
            } => {
                usize::from(content_address.is_some())
                    + usize::from(article_url.is_some())
                    + usize::from(source_url.is_some())
            },
            Block::Menu { rows } => links(&menu_spans(rows)),
            Block::CodeBlock { .. }
            | Block::Preformatted { .. }
            | Block::Rule
            | Block::MetadataRow { .. }
            | Block::Badge { .. } => 0,
            // An unknown kind renders as a placeholder badge, which has no link.
            _ => 0,
        }
    }

    fn content_left(&self, indent_level: u32) -> f32 {
        self.column_left() + (indent_level as f32) * self.style.indent_per_level
    }

    fn available_width(&self, indent_level: u32) -> f32 {
        (self.column_width() - (indent_level as f32) * self.style.indent_per_level).max(0.0)
    }

    /// The body column's actual width, bounded by the viewport's padding and
    /// an optional reader-mode measure.
    fn column_width(&self) -> f32 {
        let available = (self.viewport.width - 2.0 * self.style.horizontal_padding).max(0.0);
        self.style
            .max_content_width
            .map(|maximum| available.min(maximum.max(0.0)))
            .unwrap_or(available)
    }

    /// The body column is centred only when it has been capped. Without a
    /// cap, this remains exactly the historic horizontal padding.
    fn column_left(&self) -> f32 {
        ((self.viewport.width - self.column_width()) * 0.5).max(self.style.horizontal_padding)
    }

    fn lay_out_block(&mut self, block: &Block, source_index: usize, indent_level: u32) {
        let rendered = self.render_block(block, source_index, indent_level);
        if let Some(rendered) = rendered {
            self.cursor_y = rendered.bounds.max_y();
            self.max_x = self.max_x.max(rendered.bounds.max_x());
            self.blocks.push(rendered);
        }
    }

    fn render_block(
        &mut self,
        block: &Block,
        source_index: usize,
        indent_level: u32,
    ) -> Option<RenderedBlock> {
        match block {
            Block::Presented {
                presentation,
                block,
            } => {
                let previous = self.alignment;
                self.alignment = presentation.alignment;
                let rendered = self.render_block(
                    block,
                    source_index,
                    indent_level.saturating_add(presentation.indent_level),
                );
                self.alignment = previous;
                rendered
            },
            Block::Table {
                alignments,
                header,
                rows,
            } => Some(self.render_table(source_index, indent_level, alignments, header, rows)),
            Block::Heading { level, spans } => {
                Some(self.render_heading(source_index, indent_level, *level, spans))
            },
            Block::Paragraph { spans } => {
                let resolved = self.style.resolve(BlockRole::Body);
                Some(self.render_paragraph(
                    source_index,
                    indent_level,
                    spans,
                    text_base_from(&resolved, self.alignment, self.style.source_presentation),
                    resolved.spacing_below,
                ))
            },
            Block::CodeBlock { text, .. } => {
                Some(self.render_code_block(source_index, indent_level, text))
            },
            Block::Preformatted { text } => {
                Some(self.render_code_block(source_index, indent_level, text))
            },
            Block::Quote { blocks } => {
                Some(self.render_group(source_index, indent_level + 1, blocks))
            },
            Block::List { items, .. } => {
                Some(self.render_list(source_index, indent_level + 1, items))
            },
            Block::Image { url, alt } => {
                Some(self.render_image(source_index, indent_level, url.clone(), alt.clone()))
            },
            Block::Rule => Some(self.render_rule(source_index, indent_level)),
            Block::FeedHeader {
                title,
                subtitle,
                summary,
                source_url,
            } => Some(self.render_feed_header(
                source_index,
                indent_level,
                title,
                subtitle.as_deref(),
                summary.as_deref(),
                source_url.as_deref(),
            )),
            Block::FeedEntry {
                title,
                date,
                summary,
                article_url,
                source_url,
                content_address,
                ..
            } => Some(self.render_feed_entry(
                source_index,
                indent_level,
                title,
                date.as_deref(),
                summary.as_deref(),
                content_address.as_deref(),
                article_url.as_deref(),
                source_url.as_deref(),
            )),
            Block::MetadataRow { label, value } => {
                Some(self.render_metadata_row(source_index, indent_level, label, value))
            },
            Block::Badge { text } => Some(self.render_badge(source_index, indent_level, text)),
            Block::Menu { rows } => Some(self.render_menu(source_index, indent_level, rows)),
            // A kind this layout does not know yet stays visible, named.
            other => Some(self.render_badge(
                source_index,
                indent_level,
                &format!("unsupported block: {}", other.kind_name()),
            )),
        }
    }

    // -------------------------------------------------------------------
    // Block renderers
    // -------------------------------------------------------------------

    fn finish(self) -> LaidOutDocument {
        let total_height = self.cursor_y + self.style.vertical_padding;
        let content_width = self
            .viewport
            .width
            .max(self.max_x + self.style.horizontal_padding);
        LaidOutDocument {
            searchable: self.searchable,
            packet: DocumentRenderPacket {
                viewport: self.viewport,
                content_bounds: Rect::from_xywh(0.0, 0.0, content_width, total_height),
                blocks: self.blocks,
                interactions: self.interactions,
            },
            fonts: self.fonts.into_table(),
            wanted_dictionaries: self.env.layout_cx.wanted_segmenter_dictionaries(),
        }
    }
}

/// Shift each laid-out line into its table cell according to the column's
/// alignment. Parley remains start-aligned for ordinary document blocks;
/// table cells translate the shaped output and hit regions together.
fn align_table_cell(
    laid_out: &mut LaidOutText,
    cell_left: f32,
    cell_width: f32,
    alignment: Option<TableAlignment>,
) {
    let Some(alignment) = alignment else {
        return;
    };
    if matches!(alignment, TableAlignment::None | TableAlignment::Left) {
        return;
    }

    let mut line_widths: Vec<(f32, f32)> = Vec::new();
    for run in &laid_out.glyph_runs {
        let width = run
            .glyphs
            .iter()
            .map(|glyph| glyph.x + glyph.advance)
            .fold(0.0_f32, f32::max)
            + run.origin.x
            - cell_left;
        if let Some((line_y, line_width)) = line_widths
            .iter_mut()
            .find(|(line_y, _)| (*line_y - run.origin.y).abs() < 0.01)
        {
            *line_y = run.origin.y;
            *line_width = line_width.max(width);
        } else {
            line_widths.push((run.origin.y, width));
        }
    }

    let shift_for = |line_y: f32| {
        let Some(width) = line_widths
            .iter()
            .find(|(candidate_y, _)| (*candidate_y - line_y).abs() < 0.01)
            .map(|(_, width)| *width)
        else {
            return 0.0;
        };
        let remaining = (cell_width - width).max(0.0);
        match alignment {
            TableAlignment::Center => remaining * 0.5,
            TableAlignment::Right => remaining,
            TableAlignment::None | TableAlignment::Left => 0.0,
        }
    };

    for run in &mut laid_out.glyph_runs {
        run.origin.x += shift_for(run.origin.y);
    }
    for (_, bounds) in &mut laid_out.searchable.clusters {
        bounds.origin.x += shift_for(bounds.origin.y);
    }
    for interaction in &mut laid_out.interactions {
        interaction.bounds.origin.x += shift_for(interaction.bounds.origin.y);
    }
}

#[cfg(test)]
mod fold_tests;
#[cfg(test)]
mod tests;

#[path = "layout/tables.rs"]
mod tables;

#[path = "layout/text_blocks.rs"]
mod text_blocks;

#[path = "layout/media_blocks.rs"]
mod media_blocks;
