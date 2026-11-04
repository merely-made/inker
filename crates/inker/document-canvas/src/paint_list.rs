// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! PaintList producer.
//!
//! Walks a [`crate::DocumentRenderPacket`] and emits an [`InkerPaintList`]
//! — an implementation of [`paint_list_api::PaintList`], the engine-facing
//! display-list vocabulary shared across the Mere/Genet renderer
//! ecosystem. The list lowers to a `netrender::Scene` via
//! `paint_list_render` (see [`crate::netrender_backend`]); it can equally
//! cross IPC or sit in a capture/replay fixture, since the vocabulary is
//! fully serializable and self-contained.
//!
//! This is the portable half of the rendering path: it depends only on
//! `paint_list_api` (euclid + serde, no wgpu) plus parley's font handles,
//! so it builds everywhere the rest of document-canvas does. The Scene
//! lowering lives behind the `netrender` feature.
//!
//! ## Font handling
//!
//! Each glyph run carries a [`FontFaceId`](crate::FontFaceId) — parley's
//! *actually-shaped* face. The producer resolves it through the
//! [`FontTable`](crate::FontTable) sidecar (built during layout, carried
//! beside the packet), extracts the face bytes once per unique face into
//! the list's [`PaintList::fonts`] side-table, and references them from
//! each `DrawText` by `FontInstanceKey`.
//!
//! There is no `(family, weight, style)` re-resolution: the glyph ids and
//! the shipped face come from the *same* parley face, so a fallback can't
//! desync them (the bug this producer used to have). A run whose face is
//! somehow absent from the sidecar falls back to a placeholder rect —
//! defensive only; a table from the same layout pass always contains it.
//!
//! Interaction regions (`InteractionKind::Link`) are not represented in
//! the paint list — it carries pixels, not hit-test trees. The host
//! consumes the packet's `interactions` separately.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use paint_list_api::{
    AlphaType, ColorF, CommonPlacement, DeviceIntSize, EngineId, FontInstanceKey, FontResource,
    GlyphInstance, IdNamespace, ImageItem, ImageKey, ImageRendering, ImageResource, LayoutPoint,
    LayoutRect, LineItem, LineOrientation, LineStyle, PaintCmd, PaintList, RectItem, TextOptions,
    TextRunItem,
};

use crate::font_table::FontTable;
use crate::style::ColorVocabulary;
use crate::types::{
    DecodedImage, DocumentRenderPacket, FontFaceId, GlyphRun, Rect, RenderedBlock,
    RenderedBlockKind,
};

/// Half the hairline thickness used for [`RenderedBlockKind::Rule`]
/// (the rule fills a 1px strip centered on its mid-line).
const RULE_HALF_THICKNESS: f32 = 0.5;

/// A document-view paint list: the unit of paint output for one rendered
/// frame of an [`inker`](https://crates.io/crates/inker) document.
/// Implements [`PaintList`] so it lowers through the shared
/// `paint_list_render` translator like any other engine's output.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InkerPaintList {
    viewport: DeviceIntSize,
    commands: Vec<PaintCmd>,
    fonts: Vec<FontResource>,
    images: Vec<ImageResource>,
    generation: u64,
}

impl PaintList for InkerPaintList {
    fn engine_id(&self) -> EngineId {
        EngineId::INKER
    }
    fn viewport(&self) -> DeviceIntSize {
        self.viewport
    }
    fn generation_id(&self) -> u64 {
        self.generation
    }
    fn commands(&self) -> &[PaintCmd] {
        &self.commands
    }
    fn fonts(&self) -> &[FontResource] {
        &self.fonts
    }
    fn images(&self) -> &[ImageResource] {
        &self.images
    }
}

/// Build an [`InkerPaintList`] from a [`DocumentRenderPacket`] and its
/// [`FontTable`] sidecar (both produced by
/// [`layout_document`](crate::layout_document)).
///
/// `fonts` supplies the real face bytes parley shaped each run against;
/// `colors` supplies theme primitives. Every text run renders as real
/// glyphs — no host font wiring required, since the faces come from
/// parley's own shaping.
pub fn paint_list_from_packet(
    packet: &DocumentRenderPacket,
    fonts: &FontTable,
    colors: &ColorVocabulary,
) -> InkerPaintList {
    paint_list_from_packet_with_images(packet, fonts, colors, &HashMap::new())
}

/// Build a paint list while resolving image blocks against decoded host
/// resources keyed by the block URL.
pub fn paint_list_from_packet_with_images(
    packet: &DocumentRenderPacket,
    fonts: &FontTable,
    colors: &ColorVocabulary,
    images: &HashMap<String, DecodedImage>,
) -> InkerPaintList {
    let viewport = DeviceIntSize::new(
        packet.viewport.width.max(0.0).round() as i32,
        packet.viewport.height.max(0.0).round() as i32,
    );
    let mut builder = Builder::new(fonts, colors, images);
    for block in &packet.blocks {
        builder.emit_block(block);
    }
    InkerPaintList {
        viewport,
        commands: builder.commands,
        fonts: builder.fonts,
        images: builder.images,
        generation: 0,
    }
}

/// Walk state: accumulates `PaintCmd`s + the font side-table, interning
/// each referenced [`FontFaceId`] to a single `FontInstanceKey` (and
/// shipping its bytes from the sidecar once).
struct Builder<'a> {
    face_table: &'a FontTable,
    colors: &'a ColorVocabulary,
    decoded_images: &'a HashMap<String, DecodedImage>,
    commands: Vec<PaintCmd>,
    fonts: Vec<FontResource>,
    images: Vec<ImageResource>,
    face_keys: HashMap<FontFaceId, FontInstanceKey>,
    image_keys: HashMap<String, ImageKey>,
    next_font_key: u32,
    next_image_key: u32,
}

impl<'a> Builder<'a> {
    fn new(
        face_table: &'a FontTable,
        colors: &'a ColorVocabulary,
        decoded_images: &'a HashMap<String, DecodedImage>,
    ) -> Self {
        Self {
            face_table,
            colors,
            decoded_images,
            commands: Vec::new(),
            fonts: Vec::new(),
            images: Vec::new(),
            face_keys: HashMap::new(),
            image_keys: HashMap::new(),
            next_font_key: 0,
            next_image_key: 0,
        }
    }

    fn emit_block(&mut self, block: &RenderedBlock) {
        match &block.kind {
            RenderedBlockKind::Text { glyph_runs } => {
                for run in glyph_runs {
                    self.emit_glyph_run(run);
                }
            },
            RenderedBlockKind::Image { url, .. } => {
                if let Some((key, width, height)) = self.intern_image(url) {
                    self.commands.push(PaintCmd::DrawImage(ImageItem {
                        placement: CommonPlacement::new(layout_rect(contained_image_bounds(
                            block.bounds,
                            width,
                            height,
                        ))),
                        image_key: key,
                        image_rendering: ImageRendering::Auto,
                        alpha_type: AlphaType::Alpha,
                        color: ColorF::WHITE,
                    }));
                } else {
                    self.push_rect(block.bounds, self.colors.placeholder_image);
                }
            },
            RenderedBlockKind::Rule => {
                // Hairline: a 1px-tall strip centered on the rect's
                // vertical midpoint. Lowered as a (filled) line primitive.
                let mid_y = block.bounds.origin.y + block.bounds.size.height * 0.5;
                let strip = Rect::from_xywh(
                    block.bounds.origin.x,
                    mid_y - RULE_HALF_THICKNESS,
                    block.bounds.size.width,
                    RULE_HALF_THICKNESS * 2.0,
                );
                self.commands.push(PaintCmd::DrawLine(LineItem {
                    placement: CommonPlacement::new(layout_rect(strip)),
                    color: colorf(self.colors.rule),
                    style: LineStyle::Solid,
                    orientation: LineOrientation::Horizontal,
                    wavy_thickness: 0.0,
                }));
            },
            RenderedBlockKind::Group { children } => {
                for child in children {
                    self.emit_block(child);
                }
            },
        }
    }

    fn emit_glyph_run(&mut self, run: &GlyphRun) {
        if let Some(background) = run.background {
            self.push_rect(glyph_run_bounds(run), background);
        }
        let Some(font_instance) = self.intern_font(run.font_face) else {
            // The run's face isn't in the sidecar — shouldn't happen for a
            // table produced by the same layout pass. Placeholder rect so
            // the text-shaped region is still visible.
            self.push_rect(glyph_run_bounds(run), self.colors.placeholder_text);
            return;
        };

        // Translate each PositionedGlyph (relative to run.origin) into a
        // PaintList GlyphInstance (absolute in packet space, baseline-
        // anchored).
        let baseline_y = run.origin.y + run.baseline_y;
        let glyphs: Vec<GlyphInstance> = run
            .glyphs
            .iter()
            .map(|g| GlyphInstance {
                index: g.glyph_id,
                point: LayoutPoint::new(run.origin.x + g.x, baseline_y + g.y),
            })
            .collect();
        if glyphs.is_empty() {
            return;
        }

        self.commands.push(PaintCmd::DrawText(TextRunItem {
            placement: CommonPlacement::new(layout_rect(glyph_run_bounds(run))),
            font_instance,
            font_size: run.font_size,
            color: colorf(run.color),
            glyphs,
            options: TextOptions::default(),
        }));
        if run.underline {
            let bounds = glyph_run_bounds(run);
            let baseline = run.origin.y + run.baseline_y;
            let underline = Rect::from_xywh(
                bounds.origin.x,
                baseline + (run.font_size * 0.08),
                bounds.size.width,
                1.0,
            );
            self.commands.push(PaintCmd::DrawLine(LineItem {
                placement: CommonPlacement::new(layout_rect(underline)),
                color: colorf(run.color),
                style: LineStyle::Solid,
                orientation: LineOrientation::Horizontal,
                wavy_thickness: 0.0,
            }));
        }
    }

    /// Map a run's [`FontFaceId`] to the `FontInstanceKey` the side-table
    /// holds for it, minting the key + shipping the face bytes (from the
    /// sidecar) on first sight. `None` only if the sidecar lacks the face.
    fn intern_font(&mut self, face: FontFaceId) -> Option<FontInstanceKey> {
        if let Some(&key) = self.face_keys.get(&face) {
            return Some(key);
        }
        let font = self.face_table.get(face)?;
        let key = FontInstanceKey::new(IdNamespace(0), self.next_font_key);
        self.next_font_key += 1;
        self.fonts.push(FontResource {
            key,
            data: font.data.data().to_vec().into(),
            index: font.index,
        });
        self.face_keys.insert(face, key);
        Some(key)
    }

    fn intern_image(&mut self, url: &str) -> Option<(ImageKey, u32, u32)> {
        let image = self.decoded_images.get(url)?;
        if let Some(&key) = self.image_keys.get(url) {
            return Some((key, image.width, image.height));
        }
        let key = ImageKey::new(IdNamespace(0), self.next_image_key);
        self.next_image_key += 1;
        self.images.push(ImageResource {
            key,
            width: image.width,
            height: image.height,
            data: image.rgba8.clone(),
        });
        self.image_keys.insert(url.to_string(), key);
        Some((key, image.width, image.height))
    }

    fn push_rect(&mut self, bounds: Rect, color: [f32; 4]) {
        if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
            return;
        }
        self.commands.push(PaintCmd::DrawRect(RectItem {
            placement: CommonPlacement::new(layout_rect(bounds)),
            color: colorf(color),
        }));
    }
}

fn contained_image_bounds(bounds: Rect, width: u32, height: u32) -> Rect {
    if width == 0 || height == 0 || bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
        return bounds;
    }
    let scale = (bounds.size.width / width as f32).min(bounds.size.height / height as f32);
    let fitted_width = width as f32 * scale;
    let fitted_height = height as f32 * scale;
    Rect::from_xywh(
        bounds.origin.x + (bounds.size.width - fitted_width) * 0.5,
        bounds.origin.y + (bounds.size.height - fitted_height) * 0.5,
        fitted_width,
        fitted_height,
    )
}

fn glyph_run_bounds(run: &GlyphRun) -> Rect {
    let advance: f32 = run.glyphs.iter().map(|g| g.advance).sum();
    Rect::from_xywh(
        run.origin.x,
        run.origin.y,
        advance.max(1.0),
        // Approximate run height as font size * a generous line-gap factor.
        run.font_size * 1.4,
    )
}

fn layout_rect(r: Rect) -> LayoutRect {
    LayoutRect::new(
        LayoutPoint::new(r.origin.x, r.origin.y),
        LayoutPoint::new(r.max_x(), r.max_y()),
    )
}

fn colorf(c: [f32; 4]) -> ColorF {
    ColorF::new(c[0], c[1], c[2], c[3])
}

#[cfg(test)]
#[path = "paint_list/tests.rs"]
mod tests;
