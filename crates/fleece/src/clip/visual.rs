// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

pub fn attach_cropped_visual(
    fragment: &mut ClipFragment,
    snapshot_png: &[u8],
    surface_size: (u32, u32),
) -> bool {
    let Some(rect) = fragment.rect.as_ref() else {
        return false;
    };
    if rect.width <= 1.0 || rect.height <= 1.0 || snapshot_png.is_empty() {
        return false;
    }
    let Ok(image) = image::load_from_memory(snapshot_png) else {
        return false;
    };
    let rgba = image.to_rgba8();
    let (image_w, image_h) = rgba.dimensions();
    if image_w == 0 || image_h == 0 {
        return false;
    }
    let scale_x = image_w as f32 / surface_size.0.max(1) as f32;
    let scale_y = image_h as f32 / surface_size.1.max(1) as f32;
    let x = scaled_start(rect.x, scale_x, image_w);
    let y = scaled_start(rect.y, scale_y, image_h);
    let width = scaled_len(rect.width, scale_x, image_w - x);
    let height = scaled_len(rect.height, scale_y, image_h - y);
    if width == 0 || height == 0 {
        return false;
    }

    const CLIP_VISUAL_MAX: u32 = 1024;
    let cropped = image::imageops::crop_imm(&rgba, x, y, width, height).to_image();
    let cropped = if width > CLIP_VISUAL_MAX || height > CLIP_VISUAL_MAX {
        image::DynamicImage::ImageRgba8(cropped)
            .thumbnail(CLIP_VISUAL_MAX, CLIP_VISUAL_MAX)
            .to_rgba8()
    } else {
        cropped
    };
    let (width, height) = cropped.dimensions();
    let Some(png_bytes) = encode_png_rgba(cropped.as_raw(), width, height) else {
        return false;
    };
    let data_uri = format!("data:image/png;base64,{}", encode_base64(&png_bytes));
    fragment.visual = Some(ClipVisual {
        png_bytes,
        width,
        height,
        data_uri,
    });
    true
}

pub(super) fn scaled_start(value: f32, scale: f32, limit: u32) -> u32 {
    if limit == 0 {
        return 0;
    }
    (value.max(0.0) * scale)
        .floor()
        .clamp(0.0, (limit - 1) as f32) as u32
}

pub(super) fn scaled_len(value: f32, scale: f32, limit: u32) -> u32 {
    if limit == 0 {
        return 0;
    }
    (value.max(1.0) * scale).ceil().clamp(1.0, limit as f32) as u32
}

pub(super) fn encode_png_rgba(rgba: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || rgba.len() < (width as usize) * (height as usize) * 4 {
        return None;
    }
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .ok()?;
    Some(png)
}

pub(super) fn encode_base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    let mut chunks = data.chunks_exact(3);
    for chunk in &mut chunks {
        let bits = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | chunk[2] as u32;
        out.push(ALPHABET[((bits >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((bits >> 12) & 0x3f) as usize] as char);
        out.push(ALPHABET[((bits >> 6) & 0x3f) as usize] as char);
        out.push(ALPHABET[(bits & 0x3f) as usize] as char);
    }
    let rem = chunks.remainder();
    if !rem.is_empty() {
        let bits = match rem {
            [a] => (*a as u32) << 16,
            [a, b] => ((*a as u32) << 16) | ((*b as u32) << 8),
            _ => unreachable!(),
        };
        out.push(ALPHABET[((bits >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((bits >> 12) & 0x3f) as usize] as char);
        if rem.len() == 2 {
            out.push(ALPHABET[((bits >> 6) & 0x3f) as usize] as char);
            out.push('=');
        } else {
            out.push('=');
            out.push('=');
        }
    }
    out
}

pub(super) fn visual_block(fragment: &ClipFragment, visual: &ClipVisual) -> Block {
    Block::Image {
        url: visual.data_uri.clone(),
        alt: fragment
            .title
            .clone()
            .unwrap_or_else(|| "Clipped page fragment".to_string()),
    }
}
