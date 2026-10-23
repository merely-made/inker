// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Application typography projected onto the existing portable document styles.
use document_canvas::{DocumentStyleSheet, FontChoice, SizeSpec};
use tabard::{FontRole, Typography};

/// Start from the consumer's own baseline on every application/reset. Untouched
/// roles and properties retain their normal hierarchy, weights and wrapping.
/// Font discovery, fallback and measurement remain the canvas text engine's job.
pub fn document_style_with_typography(
    baseline: &DocumentStyleSheet,
    typography: &Typography,
) -> Result<DocumentStyleSheet, String> {
    typography.validate()?;
    let mut style = baseline.clone();
    if let Some(p) = typography.0.get(&FontRole::Reading) {
        if let Some(f) = p.css_family(FontRole::Reading) {
            style.roles.body.family = FontChoice::Explicit(f);
        }
        if let Some(size) = p.size_px {
            style.roles.body.size = SizeSpec::Absolute(size);
        }
        style.roles.body.line_height_ratio = p.line_height.or(style.roles.body.line_height_ratio);
    }
    if let Some(p) = typography.0.get(&FontRole::Heading) {
        if let Some(f) = p.css_family(FontRole::Heading) {
            style.roles.heading.family = FontChoice::Explicit(f);
        }
        if let Some(size) = p.size_px {
            let scale = size / style.roles.heading.sizes[0];
            style
                .roles
                .heading
                .sizes
                .iter_mut()
                .for_each(|value| *value *= scale);
        }
        style.roles.heading.line_height_ratio =
            p.line_height.or(style.roles.heading.line_height_ratio);
    }
    if let Some(p) = typography.0.get(&FontRole::Code) {
        if let Some(f) = p.css_family(FontRole::Code) {
            style.roles.code.family = FontChoice::Explicit(f);
        }
        if let Some(size) = p.size_px {
            style.roles.code.size = SizeSpec::Absolute(size);
        }
        style.roles.code.line_height_ratio = p.line_height.or(style.roles.code.line_height_ratio);
    }
    if let Some(p) = typography.0.get(&FontRole::Label) {
        for role in [&mut style.roles.metadata, &mut style.roles.badge] {
            if let Some(f) = p.css_family(FontRole::Label) {
                role.family = FontChoice::Explicit(f);
            }
            if let Some(size) = p.size_px {
                role.size = SizeSpec::Absolute(size);
            }
            role.line_height_ratio = p.line_height.or(role.line_height_ratio);
        }
    }
    Ok(style)
}

/// Every reading length of `baseline` multiplied by `scale`, leaving authored
/// intent, source identity, leading ratios, font families and color roles
/// unchanged.
fn scaled_document_style(baseline: &DocumentStyleSheet, scale: f32) -> DocumentStyleSheet {
    let scale = if scale.is_finite() { scale.clamp(0.5, 4.0) } else { 1.0 };
    let mut style = baseline.clone();
    style.body_font_size *= scale;
    style.horizontal_padding *= scale;
    style.vertical_padding *= scale;
    style.indent_per_level *= scale;
    style.max_content_width = style.max_content_width.map(|width| width * scale);
    for role in [&mut style.roles.body, &mut style.roles.code, &mut style.roles.metadata, &mut style.roles.badge] {
        if let SizeSpec::Absolute(size) = &mut role.size { *size *= scale; }
        role.spacing_above *= scale;
        role.spacing_below *= scale;
    }
    for size in &mut style.roles.heading.sizes { *size *= scale; }
    style.roles.heading.spacing_above *= scale;
    style.roles.heading.spacing_below *= scale;
    style
}

/// Reading controls adjust a projection's logical type and measure. Font
/// custody and baseline roles survive. Lengths stay logical: the host lays the
/// document out at its layout size and rasterizes at the window's scale.
pub fn document_style_for_reading(
    baseline: &DocumentStyleSheet, preferences: tabard::ReadingPreferences,
) -> DocumentStyleSheet {
    let ratio = f32::from(preferences.text_size) / 18.0;
    let scaled = scaled_document_style(baseline, ratio);
    let mut style = document_style_with_typography(&scaled, &preferences.typography())
        .expect("validated projection reading preferences");
    // Gutters do not grow with text size; line length does.
    style.horizontal_padding = baseline.horizontal_padding;
    style.vertical_padding = baseline.vertical_padding;
    style.max_content_width = Some(preferences.width_px());
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use document_canvas::BlockRole;
    use tabard::ThemeFontStyle;
    #[test]
    fn code_can_explicitly_choose_a_generic_ui_face_without_being_forced_to_monospace() {
        let baseline = DocumentStyleSheet::default();
        let mut fonts = Typography::default();
        fonts
            .set(
                FontRole::Code,
                ThemeFontStyle {
                    family: Some("system-ui".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        let style = document_style_with_typography(&baseline, &fonts).unwrap();
        let resolved = style.resolve(BlockRole::Code);
        assert_eq!(resolved.font_family, "system-ui");
        assert!(!resolved.monospace);
        assert_eq!(resolved.color, baseline.resolve(BlockRole::Code).color);
        assert_eq!(
            resolved.spacing_below,
            baseline.resolve(BlockRole::Code).spacing_below
        );
    }

    #[test]
    fn reading_override_preserves_heading_code_and_label_defaults() {
        let baseline = DocumentStyleSheet::default();
        let mut fonts = Typography::default();
        fonts
            .set(
                FontRole::Reading,
                ThemeFontStyle {
                    family: Some("Georgia".into()),
                    size_px: Some(23.0),
                    line_height: Some(1.7),
                },
            )
            .unwrap();
        let style = document_style_with_typography(&baseline, &fonts).unwrap();
        assert_eq!(style.resolve(BlockRole::Body).font_size, 23.0);
        assert_eq!(style.resolve(BlockRole::Body).line_height_ratio, 1.7);
        for role in [
            BlockRole::Heading(1),
            BlockRole::Code,
            BlockRole::Metadata,
            BlockRole::Badge,
        ] {
            assert_eq!(style.resolve(role), baseline.resolve(role));
        }
        assert_eq!(
            document_style_with_typography(&baseline, &Typography::default()).unwrap(),
            baseline
        );
    }
    #[test]
    fn heading_override_preserves_relative_hierarchy_and_code_style() {
        let baseline = DocumentStyleSheet::default();
        let mut fonts = Typography::default();
        fonts
            .set(
                FontRole::Heading,
                ThemeFontStyle {
                    size_px: Some(42.0),
                    line_height: Some(1.2),
                    ..Default::default()
                },
            )
            .unwrap();
        fonts
            .set(
                FontRole::Code,
                ThemeFontStyle {
                    family: Some("Menlo".into()),
                    size_px: Some(18.0),
                    ..Default::default()
                },
            )
            .unwrap();
        let style = document_style_with_typography(&baseline, &fonts).unwrap();
        assert_eq!(style.resolve(BlockRole::Heading(1)).font_size, 42.0);
        assert_eq!(style.resolve(BlockRole::Heading(2)).font_size, 33.0);
        assert_eq!(style.resolve(BlockRole::Code).font_size, 18.0);
        assert_eq!(
            style.resolve(BlockRole::Code).font_family,
            "\"Menlo\", monospace"
        );
        assert_eq!(
            style.resolve(BlockRole::Code).wrap,
            baseline.resolve(BlockRole::Code).wrap
        );
        assert_eq!(
            style.resolve(BlockRole::Body),
            baseline.resolve(BlockRole::Body)
        );
    }
}
