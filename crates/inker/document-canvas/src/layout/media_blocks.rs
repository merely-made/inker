// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

impl<'a> DocumentLayouter<'a> {
    pub(super) fn render_image(
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
            kind: InteractionKind::Link { url: url.clone() },
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

    pub(super) fn render_rule(&mut self, source_index: usize, indent_level: u32) -> RenderedBlock {
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

    pub(super) fn render_feed_header(
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

    pub(super) fn render_feed_entry(
        &mut self,
        source_index: usize,
        indent_level: u32,
        title: &str,
        date: Option<&str>,
        summary: Option<&str>,
        content_address: Option<&str>,
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
        // The entry's own document, read from the feed's copy of its body.
        if let Some(url) = content_address {
            composed.push(Block::Paragraph {
                spans: vec![InlineSpan::Link {
                    url: url.to_string(),
                    title: None,
                    spans: vec![InlineSpan::Text("Read here".to_string())],
                    predicate: None,
                }],
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

    pub(super) fn render_metadata_row(
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
        let base = text_base_from(&resolved, self.alignment, self.style.source_presentation);
        self.render_text_block_with_spacing(
            source_index,
            indent_level,
            &spans,
            base,
            resolved.spacing_above,
            resolved.spacing_below,
        )
    }

    pub(super) fn render_badge(
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
        let base = text_base_from(&resolved, self.alignment, self.style.source_presentation);
        self.render_text_block_with_spacing(
            source_index,
            indent_level,
            &spans,
            base,
            resolved.spacing_above,
            resolved.spacing_below,
        )
    }
}
