// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

impl<'a> DocumentLayouter<'a> {
    /// A menu as one fixed-width text block (see [`menu_spans`]). It never
    /// wraps, so every row keeps its columns; a wide row overflows for the
    /// host to scroll, as preformatted text does.
    pub(super) fn render_menu(
        &mut self,
        source_index: usize,
        indent_level: u32,
        rows: &[MenuRow],
    ) -> RenderedBlock {
        let resolved = self.style.resolve(BlockRole::Code);
        let mut base = text_base_from(&resolved, self.alignment, self.style.source_presentation);
        base.wrap = crate::style_sheet::WrapPolicy::NoWrap;
        self.render_text_block_with_spacing(
            source_index,
            indent_level,
            &menu_spans(rows),
            base,
            resolved.spacing_above,
            resolved.spacing_below,
        )
    }

    pub(super) fn render_heading(
        &mut self,
        source_index: usize,
        indent_level: u32,
        level: u8,
        spans: &[InlineSpan],
    ) -> RenderedBlock {
        let resolved = self.style.resolve(BlockRole::Heading(level));
        let base = text_base_from(&resolved, self.alignment, self.style.source_presentation);
        let Some((fold, open)) = self.fold_heading.take() else {
            return self.render_text_block_with_spacing(
                source_index,
                indent_level,
                spans,
                base,
                resolved.spacing_above,
                resolved.spacing_below,
            );
        };
        let marker = InlineSpan::Text(self.style.fold_markers.marker(open).to_owned());
        let spans = [std::slice::from_ref(&marker), spans].concat();
        let first_region = self.interactions.len();
        let rendered = self.render_text_block_with_spacing(
            source_index,
            indent_level,
            &spans,
            base,
            resolved.spacing_above,
            resolved.spacing_below,
        );
        // The whole heading row toggles; inserted first so a link inside the
        // heading still wins the hit test.
        let row = Rect::from_xywh(
            self.content_left(indent_level),
            rendered.bounds.origin.y,
            self.available_width(indent_level),
            (rendered.bounds.size.height - resolved.spacing_below).max(0.0),
        );
        self.interactions.insert(
            first_region,
            InteractionRegion {
                bounds: row,
                kind: InteractionKind::Fold { fold },
                link_semantics: None,
            },
        );
        rendered
    }

    pub(super) fn render_paragraph(
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

    pub(super) fn render_text_block_with_spacing(
        &mut self,
        source_index: usize,
        indent_level: u32,
        spans: &[InlineSpan],
        base: TextBaseStyle,
        spacing_above: f32,
        spacing_below: f32,
    ) -> RenderedBlock {
        let flattened = self.flatten(spans);
        self.render_flattened_with_spacing(
            source_index,
            indent_level,
            &flattened,
            base,
            spacing_above,
            spacing_below,
        )
    }

    pub(super) fn render_flattened_with_spacing(
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
            searchable,
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
        self.searchable.push(searchable);

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

    pub(super) fn reserve_link_identities(&mut self, count: usize) -> SemanticInteractionId {
        let first = self.next_link_identity;
        self.next_link_identity = self.next_link_identity.offset(count);
        first
    }

    pub(super) fn render_code_block(
        &mut self,
        source_index: usize,
        indent_level: u32,
        text: &str,
    ) -> RenderedBlock {
        let resolved = self.style.resolve(BlockRole::Code);
        let base = text_base_from(&resolved, self.alignment, self.style.source_presentation);
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

    pub(super) fn render_group(
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

    pub(super) fn render_list(
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
}
