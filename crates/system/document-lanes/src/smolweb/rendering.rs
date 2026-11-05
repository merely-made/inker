/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

impl SmolwebDocument {
    #[cfg(test)]
    pub(crate) fn packet(&self) -> Option<&DocumentRenderPacket> {
        self.layout.as_ref().map(|layout| &layout.packet)
    }

    /// Unresolved inline-image URLs for the host fetch actor.
    pub fn subresources(&self) -> Vec<String> {
        image_urls(&self.document.blocks)
            .into_iter()
            .filter(|url| !self.images.contains_key(url))
            .collect()
    }

    /// Decode and retain one host-fetched inline image within this document's
    /// configured resource limits.
    pub fn provide_subresource(&mut self, url: &str, bytes: &[u8]) -> bool {
        if self.images.contains_key(url)
            || !self.subresources().iter().any(|pending| pending == url)
            || bytes.len() > self.inline_media.max_encoded_bytes_per_image
        {
            return false;
        }
        let Some(image) = decode_image(bytes, self.inline_media) else {
            return false;
        };
        self.images.insert(url.to_string(), image);
        true
    }

    pub(super) fn ensure_layout(&mut self, width: u32, height: u32) {
        let size = (width.max(1), height.max(1));
        if self.layout.is_some() && self.size == size {
            return;
        }
        self.retain_reading_anchor();
        // A new viewport produces new geometry. Do not let a sizing, click,
        // or content-height query publish that geometry under the preceding
        // frame; `frame` marks it present only after painting completes.
        self.presented = false;
        self.layout_generation = self.layout_generation.wrapping_add(1);
        self.layout = Some(
            document_canvas::layout::layout_document_with_folds_and_font_resources(
                &self.document,
                Viewport::new(size.0 as f32, size.1 as f32),
                &self.reading.map_or_else(
                    || self.style.clone(),
                    |preferences| {
                        crate::typography::document_style_for_reading(&self.style, preferences)
                    },
                ),
                &self.folds,
                &self.fonts,
                &self.generic_fonts,
                &self.dictionaries,
            )
            .expect("admitted generic font ledger"),
        );
        self.find.reflow(&self.layout.as_ref().unwrap().searchable);
        self.size = size;
        self.scroll_x = self.scroll_x.min(self.max_scroll_x());
        self.scroll_y = self.scroll_y.min(self.max_scroll());
        if let Some(block) = self.pending_reveal.take() {
            self.pending_reading = None;
            self.scroll_to_block(block);
        } else if let Some(anchor) = self.pending_reading.take() {
            if let Some(y) = crate::reading_anchor::resolve(
                &self.document.address,
                &self.layout.as_ref().unwrap().searchable,
                &anchor,
                1.0,
            ) {
                self.scroll_y = y.clamp(0.0, self.max_scroll());
            }
        }
    }

    pub(super) fn max_scroll_x(&self) -> f32 {
        let Some(layout) = &self.layout else {
            return 0.0;
        };
        (layout.packet.content_bounds.size.width - self.size.0 as f32).max(0.0)
    }

    pub(super) fn max_scroll(&self) -> f32 {
        let Some(layout) = &self.layout else {
            return 0.0;
        };
        (layout.packet.content_bounds.size.height - self.size.1 as f32).max(0.0)
    }

    /// Last paint refusal in this retained session, if any.
    pub fn last_render_error(&self) -> Option<&str> {
        self.last_render_error.as_deref()
    }

    /// Paint the visible document band at the retained scroll offset.
    pub fn try_frame(&mut self, width: u32, height: u32) -> Result<Scene, String> {
        self.ensure_layout(width, height);
        let layout = self.layout.as_ref().expect("layout built above");
        let packet = layout.packet.window_rect(
            self.scroll_x,
            self.scroll_y,
            self.size.0 as f32,
            self.size.1 as f32,
        );
        let mut scene = match scene_from_packet_with_images(
            &packet,
            &layout.fonts,
            &self.style.colors,
            &self.images,
        ) {
            Ok(scene) => scene,
            Err(error) => {
                self.last_render_error = Some(error.to_string());
                self.presented = false;
                return Err(error.to_string());
            },
        };
        self.find
            .paint(&mut scene, self.scroll_x, self.scroll_y, self.size);
        self.paint_focus(&mut scene);
        scene.push_rect(
            0.0,
            0.0,
            self.size.0 as f32,
            self.size.1 as f32,
            self.background,
        );
        let background = scene.ops.pop().expect("push_rect appended an op");
        scene.ops.insert(0, background);
        self.presented = true;
        self.last_render_error = None;
        Ok(scene)
    }

    /// Compatibility frame for `DocumentSession<Scene>` callers. A refused
    /// paint is exposed through `last_render_error` and never presented as
    /// partially translated content.
    pub fn frame(&mut self, width: u32, height: u32) -> Scene {
        self.try_frame(width, height)
            .unwrap_or_else(|_| Scene::new(width, height))
    }

    /// Full laid-out content height, floored to the viewport height.
    pub fn content_height(&mut self, width: u32, height: u32) -> u32 {
        self.ensure_layout(width, height);
        self.layout
            .as_ref()
            .expect("layout built above")
            .packet
            .content_bounds
            .size
            .height
            .ceil()
            .max(height.max(1) as f32) as u32
    }
}
