// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

impl<'a> DocumentLayouter<'a> {
    /// Lay out a table as a group of cell-sized text blocks. Cells are kept
    /// as ordinary text blocks so links and submissions retain the same
    /// interaction and painting path as paragraphs. The table itself and all
    /// of its cells retain the source block's index: a table cell is a view of
    /// the source block, not a synthetic document block.
    pub(super) fn render_table(
        &mut self,
        source_index: usize,
        indent_level: u32,
        alignments: &[TableAlignment],
        header: &[Vec<InlineSpan>],
        rows: &[Vec<Vec<InlineSpan>>],
    ) -> RenderedBlock {
        let body_resolved = self.style.resolve(BlockRole::Body);
        // A Micron table's block alignment positions the table itself. Its
        // cells retain their per-column alignment, so do not feed the block
        // alignment into parley's line alignment here.
        let block_alignment = self.alignment;
        let body_base = text_base_from(
            &body_resolved,
            BlockAlignment::Start,
            self.style.source_presentation,
        );
        let mut header_base = body_base.clone();
        header_base.bold = true;

        let column_count = alignments
            .len()
            .max(header.len())
            .max(rows.iter().map(Vec::len).max().unwrap_or(0));
        let table_top = self.cursor_y;
        if column_count == 0 {
            let table_left = self.content_left(indent_level);
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
            flattened_rows.push((true, header.iter().map(|cell| self.flatten(cell)).collect()));
        }
        flattened_rows.extend(
            rows.iter()
                .map(|row| (false, row.iter().map(|cell| self.flatten(cell)).collect())),
        );

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
        // The widest run each column cannot break inside: its longest word,
        // including a link's adornment prefix where it binds to the label.
        let mut min_content_widths = vec![0.0_f32; column_count];
        let mut unbreakable_columns = vec![false; column_count];
        for (is_header, cells) in &flattened_rows {
            for (column, flattened) in cells.iter().enumerate() {
                let base = if *is_header { &header_base } else { &body_base };
                let natural_width = self.measure_table_cell(flattened, base);
                natural_widths[column] = natural_widths[column]
                    .max(minimum_cell_width)
                    .max(natural_width);
                min_content_widths[column] = min_content_widths[column]
                    .max(self.measure_table_cell_min_content(flattened, base));
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
        let inner_available = (available - gap * (column_count - 1) as f32).max(0.0);
        let minimum_inner_width = minimum_cell_width * column_count as f32;
        let fitted: Vec<f32> = {
            if natural_total > inner_available && inner_available >= minimum_inner_width {
                let excess_total = natural_total - minimum_inner_width;
                let excess_budget = inner_available - minimum_inner_width;
                if excess_total > 0.0 {
                    natural_widths
                        .iter()
                        .map(|width| {
                            minimum_cell_width
                                + (*width - minimum_cell_width) * (excess_budget / excess_total)
                        })
                        .collect()
                } else {
                    vec![minimum_cell_width; column_count]
                }
            } else if natural_total > inner_available {
                vec![minimum_cell_width; column_count]
            } else if natural_total > 0.0 {
                let extra = (inner_available - natural_total) / column_count as f32;
                natural_widths.iter().map(|width| width + extra).collect()
            } else {
                vec![inner_available / column_count as f32; column_count]
            }
        };
        // When even the metric minimum cannot fit, preserve each cell's measured
        // width and let the table overflow horizontally. This keeps unbreakable
        // words from painting through their neighbouring cells: a cell without
        // whitespace, or one whose min-content run (a long word after a link's
        // adornment prefix, say) is wider than the share it would receive.
        let unbreakable_overflow = (natural_total > inner_available
            && unbreakable_columns.iter().any(|unbreakable| *unbreakable))
            || fitted
                .iter()
                .zip(&min_content_widths)
                .any(|(share, min_content)| *min_content > *share + 0.5);
        let horizontal_overflow = available < minimum_table_width || unbreakable_overflow;
        let table_width = if horizontal_overflow {
            natural_total + gap * (column_count - 1) as f32
        } else {
            available
        };
        let content_left = self.content_left(indent_level);
        let table_left = content_left
            + match block_alignment {
                BlockAlignment::Start => 0.0,
                BlockAlignment::Center => ((available - table_width) * 0.5).max(0.0),
                BlockAlignment::End => (available - table_width).max(0.0),
            };
        let column_widths = if horizontal_overflow {
            natural_widths.clone()
        } else {
            fitted
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
                    self.searchable.push(laid_out.searchable.clone());
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

    pub(super) fn measure_table_cell(
        &mut self,
        flattened: &Flattened,
        base: &TextBaseStyle,
    ) -> f32 {
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

    /// The cell's min-content width: laid out wrapping at every opportunity,
    /// its widest line is the widest run it cannot break.
    pub(super) fn measure_table_cell_min_content(
        &mut self,
        flattened: &Flattened,
        base: &TextBaseStyle,
    ) -> f32 {
        let mut fonts = FontInterner::new();
        let mut wrapping_base = base.clone();
        wrapping_base.wrap = crate::style_sheet::WrapPolicy::Wrap;
        layout_text_block_with_link_identity_base(
            self.env,
            flattened,
            &wrapping_base,
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
}
