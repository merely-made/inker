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

use inker::{Block, EngineDocument, InlineSpan, TableAlignment};

use crate::font_table::{FontInterner, FontTable};
use crate::style_sheet::{BlockRole, ColorToken, DocumentStyleSheet, ResolvedBlockStyle};
use crate::text::{
    Flattened, LaidOutText, LayoutEnvironment, TextBaseStyle, flatten_inline,
    layout_text_block_with_link_identity_base,
};
use crate::types::{
    DocumentRenderPacket, InteractionRegion, LinkSemantics, Point, Rect, RenderedBlock,
    RenderedBlockKind, SemanticInteractionId, Size, Viewport,
};

/// A laid-out document: the serializable [`DocumentRenderPacket`] plus the
/// out-of-band [`FontTable`] sidecar carrying the `parley::FontData` each
/// `GlyphRun` was shaped against. The packet alone is the portable,
/// serializable shape; `fonts` holds `Arc`-cheap face handles whose owned
/// bytes materialize only at the `paint_list_api` boundary (the
/// `PaintList`'s font side-table). See [`crate::font_table`].
#[derive(Clone, Debug)]
pub struct LaidOutDocument {
    pub packet: DocumentRenderPacket,
    pub fonts: FontTable,
}

/// Layout entry point. Consumes an `EngineDocument` and a viewport,
/// produces a [`LaidOutDocument`]: a portable [`DocumentRenderPacket`]
/// downstream renderers paint, plus the font sidecar that resolves each
/// run's [`FontFaceId`](crate::FontFaceId) to the real face bytes parley
/// shaped against.
pub fn layout_document(
    document: &EngineDocument,
    viewport: Viewport,
    style: &DocumentStyleSheet,
) -> LaidOutDocument {
    let mut env = LayoutEnvironment::new();
    // The document's own scheme classifies its links as in-protocol vs
    // external for the `⇒` / `→` adornment.
    let base_scheme = crate::style_sheet::url_scheme(&document.address).map(str::to_string);
    let mut layouter = DocumentLayouter::new(viewport, style, &mut env, base_scheme);

    for (idx, block) in document.blocks.iter().enumerate() {
        layouter.lay_out_block(block, idx, 0);
    }

    layouter.finish()
}

/// Build the parley block base from a resolved role style: the role's
/// typography, base text `color`, and `wrap` policy.
fn text_base_from(resolved: &ResolvedBlockStyle) -> TextBaseStyle {
    TextBaseStyle {
        font_size: resolved.font_size,
        font_family: resolved.font_family.clone(),
        bold: resolved.bold,
        italic: resolved.italic,
        monospace: resolved.monospace,
        line_height_ratio: resolved.line_height_ratio,
        color: resolved.color,
        wrap: resolved.wrap,
    }
}

struct DocumentLayouter<'a> {
    viewport: Viewport,
    style: &'a DocumentStyleSheet,
    env: &'a mut LayoutEnvironment,
    cursor_y: f32,
    blocks: Vec<RenderedBlock>,
    interactions: Vec<InteractionRegion>,
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
}

impl<'a> DocumentLayouter<'a> {
    fn new(
        viewport: Viewport,
        style: &'a DocumentStyleSheet,
        env: &'a mut LayoutEnvironment,
        base_scheme: Option<String>,
    ) -> Self {
        Self {
            viewport,
            style,
            env,
            cursor_y: style.vertical_padding,
            blocks: Vec::new(),
            interactions: Vec::new(),
            max_x: 0.0,
            fonts: FontInterner::new(),
            base_scheme,
            next_link_identity: SemanticInteractionId::from_lowered_ordinal(1),
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
                    text_base_from(&resolved),
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
            } => Some(self.render_feed_entry(
                source_index,
                indent_level,
                title,
                date.as_deref(),
                summary.as_deref(),
                article_url.as_deref(),
                source_url.as_deref(),
            )),
            Block::MetadataRow { label, value } => {
                Some(self.render_metadata_row(source_index, indent_level, label, value))
            },
            Block::Badge { text } => Some(self.render_badge(source_index, indent_level, text)),
        }
    }

    // -------------------------------------------------------------------
    // Block renderers
    // -------------------------------------------------------------------

    fn render_heading(
        &mut self,
        source_index: usize,
        indent_level: u32,
        level: u8,
        spans: &[InlineSpan],
    ) -> RenderedBlock {
        let resolved = self.style.resolve(BlockRole::Heading(level));
        let base = text_base_from(&resolved);
        self.render_text_block_with_spacing(
            source_index,
            indent_level,
            spans,
            base,
            resolved.spacing_above,
            resolved.spacing_below,
        )
    }

    fn render_paragraph(
        &mut self,
        source_index: usize,
        indent_level: u32,
        spans: &[InlineSpan],
        base: TextBaseStyle,
        spacing_below: f32,
    ) -> RenderedBlock {
        self.render_text_block_with_spacing(
            source_index,
            indent_level,
            spans,
            base,
            0.0,
            spacing_below,
        )
    }

    fn render_text_block_with_spacing(
        &mut self,
        source_index: usize,
        indent_level: u32,
        spans: &[InlineSpan],
        base: TextBaseStyle,
        spacing_above: f32,
        spacing_below: f32,
    ) -> RenderedBlock {
        let flattened = flatten_inline(
            spans,
            self.style.link_adornment,
            self.base_scheme.as_deref(),
        );
        self.render_flattened_with_spacing(
            source_index,
            indent_level,
            &flattened,
            base,
            spacing_above,
            spacing_below,
        )
    }

    /// Lay out a table as a group of cell-sized text blocks. Cells are kept
    /// as ordinary text blocks so links and submissions retain the same
    /// interaction and painting path as paragraphs. The table itself and all
    /// of its cells retain the source block's index: a table cell is a view of
    /// the source block, not a synthetic document block.
    fn render_table(
        &mut self,
        source_index: usize,
        indent_level: u32,
        alignments: &[TableAlignment],
        header: &[Vec<InlineSpan>],
        rows: &[Vec<Vec<InlineSpan>>],
    ) -> RenderedBlock {
        let body_resolved = self.style.resolve(BlockRole::Body);
        let body_base = text_base_from(&body_resolved);
        let mut header_base = body_base.clone();
        header_base.bold = true;

        let column_count = alignments
            .len()
            .max(header.len())
            .max(rows.iter().map(Vec::len).max().unwrap_or(0));
        let table_left = self.content_left(indent_level);
        let table_top = self.cursor_y;
        if column_count == 0 {
            return RenderedBlock {
                source_block_index: source_index,
                bounds: Rect::from_xywh(table_left, table_top, 0.0, body_resolved.spacing_below),
                kind: RenderedBlockKind::Group {
                    children: Vec::new(),
                },
            };
        }

        // Flatten once for measurement and the final pass. Measurement uses a
        // throwaway font table and never allocates document link identities.
        let mut flattened_rows: Vec<(bool, Vec<Flattened>)> = Vec::new();
        if !header.is_empty() {
            flattened_rows.push((
                true,
                header
                    .iter()
                    .map(|cell| {
                        flatten_inline(cell, self.style.link_adornment, self.base_scheme.as_deref())
                    })
                    .collect(),
            ));
        }
        flattened_rows.extend(rows.iter().map(|row| {
            (
                false,
                row.iter()
                    .map(|cell| {
                        flatten_inline(cell, self.style.link_adornment, self.base_scheme.as_deref())
                    })
                    .collect(),
            )
        }));

        let available = self.available_width(indent_level);
        // Keep a metric-sized minimum cell. If the viewport cannot contain
        // all columns at that size, the table gets an explicit horizontal
        // overflow width instead of collapsing adjacent cells onto one
        // another. Ordinary narrow tables still fit and wrap to the viewport.
        let gap = if column_count > 1 {
            self.style.block_spacing().max(0.0)
        } else {
            0.0
        };
        let minimum_cell_width = body_resolved.font_size.max(1.0);
        let minimum_table_width = minimum_cell_width * column_count as f32
            + gap * (column_count.saturating_sub(1)) as f32;
        let mut natural_widths = vec![minimum_cell_width; column_count];
        let mut unbreakable_columns = vec![false; column_count];
        for (is_header, cells) in &flattened_rows {
            for (column, flattened) in cells.iter().enumerate() {
                let base = if *is_header { &header_base } else { &body_base };
                let natural_width = self.measure_table_cell(flattened, base);
                natural_widths[column] = natural_widths[column]
                    .max(minimum_cell_width)
                    .max(natural_width);
                if natural_width > minimum_cell_width
                    && !flattened.text.chars().any(char::is_whitespace)
                {
                    unbreakable_columns[column] = true;
                }
            }
        }
        // The table fills the content column. Natural widths are retained when
        // they fit; when they do not, each column receives a proportional share
        // and parley wraps the cell into that share.
        let natural_total: f32 = natural_widths.iter().sum();
        // When even the metric minimum cannot fit, preserve each cell's
        // measured width and let the table overflow horizontally. This keeps
        // unbreakable words from painting through their neighbouring cells.
        let inner_available = (available - gap * (column_count - 1) as f32).max(0.0);
        let unbreakable_overflow = natural_total > inner_available
            && unbreakable_columns.iter().any(|unbreakable| *unbreakable);
        let horizontal_overflow = available < minimum_table_width || unbreakable_overflow;
        let table_width = if horizontal_overflow {
            natural_total + gap * (column_count - 1) as f32
        } else {
            available
        };
        let inner_width = (table_width - gap * (column_count - 1) as f32).max(0.0);
        let minimum_inner_width = minimum_cell_width * column_count as f32;
        let column_widths = if horizontal_overflow {
            natural_widths.clone()
        } else if natural_total > inner_width && inner_width >= minimum_inner_width {
            let excess_total = natural_total - minimum_inner_width;
            let excess_budget = inner_width - minimum_inner_width;
            if excess_total > 0.0 {
                natural_widths
                    .iter()
                    .map(|width| {
                        minimum_cell_width
                            + (*width - minimum_cell_width) * (excess_budget / excess_total)
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![minimum_cell_width; column_count]
            }
        } else if natural_total > inner_width {
            vec![minimum_cell_width; column_count]
        } else if natural_total > 0.0 {
            let extra = (inner_width - natural_total) / column_count as f32;
            natural_widths.iter().map(|width| width + extra).collect()
        } else {
            vec![inner_width / column_count as f32; column_count]
        };

        let link_color = self.style.token_color(ColorToken::LinkText);
        let code_color = self.style.token_color(ColorToken::CodeText);
        let mut children = Vec::new();
        let mut row_top = table_top + body_resolved.spacing_above;
        for (is_header, cells) in &flattened_rows {
            let base = if *is_header { &header_base } else { &body_base };
            let mut row_cells = Vec::new();
            let mut row_height = self.style.line_height(base.font_size);
            let mut x = table_left;
            for column in 0..column_count {
                let width = column_widths[column];
                let flattened = cells.get(column);
                let mut laid_out = flattened.map(|flattened| {
                    let identity = self.reserve_link_identities(flattened.links.len());
                    layout_text_block_with_link_identity_base(
                        self.env,
                        flattened,
                        base,
                        link_color,
                        code_color,
                        width,
                        Point::new(x, row_top),
                        &mut self.fonts,
                        identity,
                    )
                });
                if let Some(laid_out) = &mut laid_out {
                    align_table_cell(laid_out, x, width, alignments.get(column).copied());
                    row_height = row_height.max(laid_out.total_size.height);
                }
                row_cells.push((x, width, laid_out));
                x += width + gap;
            }

            for (x, width, laid_out) in row_cells {
                let (glyph_runs, total_height, mut interactions) = laid_out
                    .map(|laid_out| {
                        (
                            laid_out.glyph_runs,
                            laid_out.total_size.height,
                            laid_out.interactions,
                        )
                    })
                    .unwrap_or((Vec::new(), 0.0, Vec::new()));
                self.interactions.append(&mut interactions);
                children.push(RenderedBlock {
                    source_block_index: source_index,
                    bounds: Rect::from_xywh(x, row_top, width, row_height.max(total_height)),
                    kind: RenderedBlockKind::Text { glyph_runs },
                });
            }
            row_top += row_height + gap;
        }

        let table_height = (row_top - table_top - gap + body_resolved.spacing_below).max(0.0);
        RenderedBlock {
            source_block_index: source_index,
            bounds: Rect::from_xywh(table_left, table_top, table_width, table_height),
            kind: RenderedBlockKind::Group { children },
        }
    }

    fn measure_table_cell(&mut self, flattened: &Flattened, base: &TextBaseStyle) -> f32 {
        let mut fonts = FontInterner::new();
        let mut natural_base = base.clone();
        natural_base.wrap = crate::style_sheet::WrapPolicy::NoWrap;
        layout_text_block_with_link_identity_base(
            self.env,
            flattened,
            &natural_base,
            self.style.token_color(ColorToken::LinkText),
            self.style.token_color(ColorToken::CodeText),
            0.0,
            Point::ZERO,
            &mut fonts,
            SemanticInteractionId::from_lowered_ordinal(1),
        )
        .total_size
        .width
    }

    fn render_flattened_with_spacing(
        &mut self,
        source_index: usize,
        indent_level: u32,
        flattened: &Flattened,
        base: TextBaseStyle,
        spacing_above: f32,
        spacing_below: f32,
    ) -> RenderedBlock {
        let origin = Point::new(
            self.content_left(indent_level),
            self.cursor_y + spacing_above,
        );
        let available = self.available_width(indent_level);
        // Inline link + code colors are sheet-global (any block can contain
        // them); the block's base color rides on `base`.
        let link_color = self.style.token_color(ColorToken::LinkText);
        let code_color = self.style.token_color(ColorToken::CodeText);
        let link_identity_base = self.reserve_link_identities(flattened.links.len());
        let LaidOutText {
            glyph_runs,
            total_size,
            mut interactions,
        } = layout_text_block_with_link_identity_base(
            self.env,
            flattened,
            &base,
            link_color,
            code_color,
            available,
            origin,
            &mut self.fonts,
            link_identity_base,
        );

        self.interactions.append(&mut interactions);

        let bounds = Rect::new(
            origin,
            Size::new(total_size.width, total_size.height + spacing_below),
        );

        RenderedBlock {
            source_block_index: source_index,
            bounds,
            kind: RenderedBlockKind::Text { glyph_runs },
        }
    }

    fn reserve_link_identities(&mut self, count: usize) -> SemanticInteractionId {
        let first = self.next_link_identity;
        self.next_link_identity = self.next_link_identity.offset(count);
        first
    }

    fn render_code_block(
        &mut self,
        source_index: usize,
        indent_level: u32,
        text: &str,
    ) -> RenderedBlock {
        let resolved = self.style.resolve(BlockRole::Code);
        let base = text_base_from(&resolved);
        let spans = vec![InlineSpan::Text(text.to_string())];
        self.render_text_block_with_spacing(
            source_index,
            indent_level,
            &spans,
            base,
            resolved.spacing_above,
            resolved.spacing_below,
        )
    }

    fn render_group(
        &mut self,
        source_index: usize,
        indent_level: u32,
        children: &[Block],
    ) -> RenderedBlock {
        let group_top = self.cursor_y;
        let mut child_blocks: Vec<RenderedBlock> = Vec::new();
        for (i, child) in children.iter().enumerate() {
            // Children carry their own source indices in the parent's
            // coordinate; we project the parent's index into a synthetic
            // sub-index space (parent_index * 1000 + child_index). Crude
            // but stable enough for v1 hit-back-to-source mapping.
            let synthetic = source_index.saturating_mul(1000) + i;
            if let Some(rendered) = self.render_block(child, synthetic, indent_level) {
                self.cursor_y = rendered.bounds.max_y();
                self.max_x = self.max_x.max(rendered.bounds.max_x());
                child_blocks.push(rendered);
            }
        }
        let group_bottom = self.cursor_y;
        let group_left = self.content_left(indent_level);
        let group_right = self.max_x;
        RenderedBlock {
            source_block_index: source_index,
            bounds: Rect::from_xywh(
                group_left,
                group_top,
                (group_right - group_left).max(0.0),
                group_bottom - group_top,
            ),
            kind: RenderedBlockKind::Group {
                children: child_blocks,
            },
        }
    }

    fn render_list(
        &mut self,
        source_index: usize,
        indent_level: u32,
        items: &[Vec<Block>],
    ) -> RenderedBlock {
        let group_top = self.cursor_y;
        let mut child_blocks: Vec<RenderedBlock> = Vec::new();
        for (i, item) in items.iter().enumerate() {
            for (j, child) in item.iter().enumerate() {
                let synthetic = source_index.saturating_mul(1000) + i.saturating_mul(100) + j;
                if let Some(rendered) = self.render_block(child, synthetic, indent_level) {
                    self.cursor_y = rendered.bounds.max_y();
                    self.max_x = self.max_x.max(rendered.bounds.max_x());
                    child_blocks.push(rendered);
                }
            }
        }
        let group_bottom = self.cursor_y;
        let group_left = self.content_left(indent_level);
        let group_right = self.max_x;
        RenderedBlock {
            source_block_index: source_index,
            bounds: Rect::from_xywh(
                group_left,
                group_top,
                (group_right - group_left).max(0.0),
                group_bottom - group_top,
            ),
            kind: RenderedBlockKind::Group {
                children: child_blocks,
            },
        }
    }

    fn render_image(
        &mut self,
        source_index: usize,
        indent_level: u32,
        url: String,
        alt: String,
    ) -> RenderedBlock {
        // v1 reserves a placeholder strip the height of one line of body
        // text. Renderer fetches + paints the actual image; document-canvas
        // doesn't load bytes.
        let line_height = self.style.line_height(self.style.body_font_size);
        let height = line_height * 6.0; // ~6 lines worth of placeholder
        let origin = Point::new(self.content_left(indent_level), self.cursor_y);
        let bounds = Rect::new(
            origin,
            Size::new(
                self.available_width(indent_level),
                height + self.style.block_spacing(),
            ),
        );
        let identity = self.reserve_link_identities(1);
        self.interactions.push(InteractionRegion {
            bounds,
            kind: crate::types::InteractionKind::Link { url: url.clone() },
            link_semantics: Some(LinkSemantics {
                identity,
                accessible_label: alt.clone(),
            }),
        });
        RenderedBlock {
            source_block_index: source_index,
            bounds,
            kind: RenderedBlockKind::Image { url, alt },
        }
    }

    fn render_rule(&mut self, source_index: usize, indent_level: u32) -> RenderedBlock {
        let origin = Point::new(self.content_left(indent_level), self.cursor_y);
        let bounds = Rect::new(
            origin,
            Size::new(
                self.available_width(indent_level),
                self.style.block_spacing(),
            ),
        );
        RenderedBlock {
            source_block_index: source_index,
            bounds,
            kind: RenderedBlockKind::Rule,
        }
    }

    fn render_feed_header(
        &mut self,
        source_index: usize,
        indent_level: u32,
        title: &str,
        subtitle: Option<&str>,
        summary: Option<&str>,
        source_url: Option<&str>,
    ) -> RenderedBlock {
        let mut composed: Vec<Block> = Vec::new();
        composed.push(Block::Heading {
            level: 1,
            spans: vec![InlineSpan::Text(title.to_string())],
        });
        if let Some(s) = subtitle {
            composed.push(Block::Heading {
                level: 2,
                spans: vec![InlineSpan::Text(s.to_string())],
            });
        }
        if let Some(s) = summary {
            composed.push(Block::Paragraph {
                spans: vec![InlineSpan::Text(s.to_string())],
            });
        }
        if let Some(url) = source_url {
            composed.push(Block::Paragraph {
                spans: vec![InlineSpan::Link {
                    url: url.to_string(),
                    title: None,
                    spans: vec![InlineSpan::Text("Open source".to_string())],
                    predicate: None,
                }],
            });
        }
        self.render_group(source_index, indent_level, &composed)
    }

    fn render_feed_entry(
        &mut self,
        source_index: usize,
        indent_level: u32,
        title: &str,
        date: Option<&str>,
        summary: Option<&str>,
        article_url: Option<&str>,
        source_url: Option<&str>,
    ) -> RenderedBlock {
        let mut composed: Vec<Block> = Vec::new();
        composed.push(Block::Heading {
            level: 2,
            spans: vec![InlineSpan::Text(title.to_string())],
        });
        if let Some(d) = date {
            composed.push(Block::Paragraph {
                spans: vec![InlineSpan::Emphasis(vec![InlineSpan::Text(d.to_string())])],
            });
        }
        if let Some(s) = summary {
            composed.push(Block::Paragraph {
                spans: vec![InlineSpan::Text(s.to_string())],
            });
        }
        if let Some(url) = article_url {
            composed.push(Block::Paragraph {
                spans: vec![InlineSpan::Link {
                    url: url.to_string(),
                    title: None,
                    spans: vec![InlineSpan::Text("Open article".to_string())],
                    predicate: None,
                }],
            });
        }
        if let Some(url) = source_url {
            composed.push(Block::Paragraph {
                spans: vec![InlineSpan::Link {
                    url: url.to_string(),
                    title: None,
                    spans: vec![InlineSpan::Text("Open source".to_string())],
                    predicate: None,
                }],
            });
        }
        self.render_group(source_index, indent_level, &composed)
    }

    fn render_metadata_row(
        &mut self,
        source_index: usize,
        indent_level: u32,
        label: &str,
        value: &str,
    ) -> RenderedBlock {
        // Label in bold + value in normal. Lay out as a single paragraph.
        let spans = vec![
            InlineSpan::Strong(vec![InlineSpan::Text(format!("{label}: "))]),
            InlineSpan::Text(value.to_string()),
        ];
        let resolved = self.style.resolve(BlockRole::Metadata);
        let base = text_base_from(&resolved);
        self.render_text_block_with_spacing(
            source_index,
            indent_level,
            &spans,
            base,
            resolved.spacing_above,
            resolved.spacing_below,
        )
    }

    fn render_badge(
        &mut self,
        source_index: usize,
        indent_level: u32,
        text: &str,
    ) -> RenderedBlock {
        // Badge as a small italic paragraph; renderer paints the pill if it
        // wants. v1 doesn't carry pill-shape metadata.
        let spans = vec![InlineSpan::Emphasis(vec![InlineSpan::Text(
            text.to_string(),
        )])];
        let resolved = self.style.resolve(BlockRole::Badge);
        let base = text_base_from(&resolved);
        self.render_text_block_with_spacing(
            source_index,
            indent_level,
            &spans,
            base,
            resolved.spacing_above,
            resolved.spacing_below,
        )
    }

    fn finish(self) -> LaidOutDocument {
        let total_height = self.cursor_y + self.style.vertical_padding;
        let content_width = self
            .viewport
            .width
            .max(self.max_x + self.style.horizontal_padding);
        LaidOutDocument {
            packet: DocumentRenderPacket {
                viewport: self.viewport,
                content_bounds: Rect::from_xywh(0.0, 0.0, content_width, total_height),
                blocks: self.blocks,
                interactions: self.interactions,
            },
            fonts: self.fonts.into_table(),
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
    for interaction in &mut laid_out.interactions {
        interaction.bounds.origin.x += shift_for(interaction.bounds.origin.y);
    }
}

#[cfg(test)]
mod tests;
