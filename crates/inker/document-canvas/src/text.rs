// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Inline-span flattening + parley wrapper.
//!
//! Two responsibilities:
//!
//! 1. **Flatten** a `Vec<inker::InlineSpan>` into a single text buffer plus
//!    ranged style attributes plus link metadata. parley wants byte ranges
//!    over a single string; we walk the inline tree to produce them.
//! 2. **Lay out** the flattened text via parley, emitting our own
//!    [`GlyphRun`] records (positions in packet-local coordinates) plus
//!    interaction regions for links.

use std::{borrow::Cow, ops::Range};

use inker::{BlockAlignment, InlineSpan};
use parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, GenericFamily, LayoutContext, LineHeight,
    PositionedLayoutItem, StyleProperty,
};

use crate::font_table::FontInterner;
use crate::style::InlineStyle;
use crate::style_sheet::{LinkAdornment, SourcePresentation, WrapPolicy};
use crate::types::{
    GlyphRun, InteractionKind, InteractionRegion, LinkSemantics, Point, PositionedGlyph, Rect,
    SemanticInteractionId, Size, TextStyle,
};

/// Block-level base style for a text block (heading vs body vs code etc.).
#[derive(Clone, Debug)]
pub struct TextBaseStyle {
    pub font_size: f32,
    pub font_family: String,
    pub bold: bool,
    pub italic: bool,
    pub monospace: bool,
    pub line_height_ratio: f32,
    /// Premultiplied RGBA (0..=1) for the block's text. Inline links + code
    /// override it per-run inside [`layout_text_block`].
    pub color: [f32; 4],
    /// Whether the block wraps to the available width (`Wrap`) or lays out on
    /// its natural width and overflows for the host to scroll (`NoWrap`, e.g.
    /// code blocks).
    pub wrap: WrapPolicy,
    /// Alignment carried by the source block, resolved in the shared layout
    /// rather than by a consumer-specific renderer.
    pub alignment: BlockAlignment,
    /// Host-selected source-presentation policy.
    pub source_presentation: SourcePresentation,
}

impl Default for TextBaseStyle {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            font_family: "system-ui".to_string(),
            bold: false,
            italic: false,
            monospace: false,
            line_height_ratio: 1.4,
            color: [0.0, 0.0, 0.0, 1.0],
            wrap: WrapPolicy::Wrap,
            alignment: BlockAlignment::Start,
            source_presentation: SourcePresentation::Respect,
        }
    }
}

/// Flatten a span tree into a single text string plus ranged styles plus
/// link annotations. Returned ranges are byte offsets into the text string.
///
/// `adornment` + `base_scheme` drive the per-link prefix glyph (the `⇒` / `→`
/// scheme arrows), and `in_page_adornment` does the same for in-page links:
/// when adornment applies, the prefix is prepended to the link's display
/// text, styled as part of the link and covered by its byte range (so it
/// colors + hit-tests as the link).
pub fn flatten_inline(
    spans: &[InlineSpan],
    adornment: &LinkAdornment,
    in_page_adornment: &LinkAdornment,
    base_scheme: Option<&str>,
) -> Flattened {
    let mut out = Flattened::default();
    let adornments = Adornments {
        link: adornment,
        in_page: in_page_adornment,
    };
    flatten_into(
        spans,
        InlineStyle::NORMAL,
        adornments,
        base_scheme,
        &mut out,
    );
    out
}

/// The network and in-page link adornments, carried together down the span tree.
#[derive(Clone, Copy)]
struct Adornments<'a> {
    link: &'a LinkAdornment,
    in_page: &'a LinkAdornment,
}

#[derive(Clone, Debug, Default)]
pub struct Flattened {
    pub text: String,
    /// Per-span style ranges (byte offsets). May overlap if you nest
    /// emphasis-in-strong; later ranges win in the parley range stack
    /// (parley merges them additively per-property).
    pub styles: Vec<(Range<usize>, InlineStyle)>,
    /// Link annotations. The range covers the rendered hit text, including a
    /// visual scheme-arrow prefix; `accessible_label` keeps only the
    /// author-lowered link text.
    pub links: Vec<LinkAnnotation>,
    /// Submission annotations. Byte range of the label + mutation target.
    pub submissions: Vec<(Range<usize>, String)>,
}

/// One flattened link's visual range and retained semantic text.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkAnnotation {
    /// Byte range of the rendered link hit region, including decoration.
    pub range: Range<usize>,
    /// [`InteractionKind::Link`] or [`InteractionKind::InPage`]; both take a
    /// link identity.
    pub kind: InteractionKind,
    /// The author-lowered link text, excluding decorative adornment.
    pub accessible_label: String,
}

fn flatten_into(
    spans: &[InlineSpan],
    inherited: InlineStyle,
    adornments: Adornments<'_>,
    base_scheme: Option<&str>,
    out: &mut Flattened,
) {
    for span in spans {
        match span {
            InlineSpan::Presented {
                presentation,
                spans,
            } => {
                flatten_into(
                    spans,
                    inherited.with_presentation(*presentation),
                    adornments,
                    base_scheme,
                    out,
                );
            },
            InlineSpan::Text(t) => {
                let start = out.text.len();
                out.text.push_str(t);
                let end = out.text.len();
                if start < end {
                    out.styles.push((start..end, inherited));
                }
            },
            InlineSpan::Code(t) => {
                let start = out.text.len();
                out.text.push_str(t);
                let end = out.text.len();
                if start < end {
                    out.styles.push((start..end, inherited.with_monospace()));
                }
            },
            InlineSpan::Emphasis(inner) => {
                flatten_into(inner, inherited.with_italic(), adornments, base_scheme, out);
            },
            InlineSpan::Strong(inner) => {
                flatten_into(inner, inherited.with_bold(), adornments, base_scheme, out);
            },
            InlineSpan::Link {
                url, spans: inner, ..
            } => flatten_link(
                InteractionKind::Link { url: url.clone() },
                adornments.link.prefix_for(url, base_scheme),
                inner,
                inherited,
                adornments,
                base_scheme,
                out,
            ),
            InlineSpan::Submit {
                target,
                spans: inner,
            } => {
                let start = out.text.len();
                flatten_into(inner, inherited.with_link(), adornments, base_scheme, out);
                let end = out.text.len();
                if start < end {
                    out.submissions.push((start..end, target.clone()));
                }
            },
            InlineSpan::InPage {
                target,
                spans: inner,
            } => flatten_link(
                InteractionKind::InPage {
                    block: target.block,
                    fragment: target.fragment.clone(),
                },
                // A same-document reference is in-protocol, like a relative link.
                adornments.in_page.prefix_for("#", base_scheme),
                inner,
                inherited,
                adornments,
                base_scheme,
                out,
            ),
            InlineSpan::SoftBreak => {
                out.text.push(' ');
            },
            InlineSpan::LineBreak => {
                out.text.push('\n');
            },
        }
    }
}

/// A link's optional adornment prefix and label, styled and ranged as one link.
fn flatten_link(
    kind: InteractionKind,
    prefix: Option<&str>,
    inner: &[InlineSpan],
    inherited: InlineStyle,
    adornments: Adornments<'_>,
    base_scheme: Option<&str>,
    out: &mut Flattened,
) {
    let link_start = out.text.len();
    if let Some(prefix) = prefix {
        out.text.push_str(prefix);
        out.styles
            .push((link_start..out.text.len(), inherited.with_link()));
    }
    let label_start = out.text.len();
    flatten_into(inner, inherited.with_link(), adornments, base_scheme, out);
    let link_end = out.text.len();
    if link_start < link_end {
        out.links.push(LinkAnnotation {
            range: link_start..link_end,
            kind,
            accessible_label: out.text[label_start..link_end].to_string(),
        });
    }
}

/// Per-block layout output.
#[derive(Clone, Debug)]
pub struct LaidOutText {
    /// Rendered text and measured cluster geometry, independent of glyph IDs.
    pub searchable: SearchableText,
    pub glyph_runs: Vec<GlyphRun>,
    pub total_size: Size,
    pub interactions: Vec<InteractionRegion>,
}

/// Text retained for selection/find. Byte ranges name the exact flattened
/// buffer passed to Parley, including inline style boundaries and soft wraps.
#[derive(Clone, Debug, Default)]
pub struct SearchableText {
    pub text: String,
    pub clusters: Vec<(Range<usize>, Rect)>,
}

/// Owns parley contexts so consumers don't recreate them per call.
pub struct LayoutEnvironment {
    pub font_cx: FontContext,
    pub layout_cx: LayoutContext<InlineStyle>,
}

impl Default for LayoutEnvironment {
    fn default() -> Self {
        Self {
            font_cx: FontContext::new(),
            layout_cx: LayoutContext::new(),
        }
    }
}

impl LayoutEnvironment {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a `LayoutEnvironment` with a font resolver pre-registered
    /// against parley's font context. Layout-time text shaping then sees
    /// whatever fonts the resolver provides. The render side no longer
    /// consults the resolver: glyph runs carry parley's actually-shaped
    /// face (a [`FontFaceId`](crate::FontFaceId) into the font sidecar),
    /// so the bytes downstream renders come from the same face parley
    /// shaped against.
    pub fn with_resolver<R: crate::font::FontResolver>(resolver: &R) -> Self {
        let mut env = Self::default();
        resolver.register_with_parley(&mut env.font_cx);
        env
    }
}

/// Invalid explicit generic binding. No system/global fallback is installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenericFontError {
    InvalidGeneric,
    InvalidFont,
}

/// Register the complete host generic ledger in a caller-owned layout context.
/// The caller admits the ledger only after all entries validate successfully.
pub fn register_generic_fonts(
    env: &mut LayoutEnvironment,
    fonts: &[(String, Vec<u8>)],
) -> Result<usize, GenericFontError> {
    if fonts.is_empty() {
        return Ok(0);
    }
    // Validate the whole batch in an isolated context before touching any
    // caller-owned generic mapping. An invalid late entry cannot admit an
    // earlier entry from the same batch.
    let mut validation = LayoutEnvironment::new();
    register_generic_fonts_inner(&mut validation, fonts)?;
    register_generic_fonts_inner(env, fonts)
}

fn register_generic_fonts_inner(
    env: &mut LayoutEnvironment,
    fonts: &[(String, Vec<u8>)],
) -> Result<usize, GenericFontError> {
    let mut count = 0;
    for (name, bytes) in fonts {
        let generic = match name.as_str() {
            "sans-serif" => GenericFamily::SansSerif,
            "serif" => GenericFamily::Serif,
            "monospace" => GenericFamily::Monospace,
            "system-ui" => GenericFamily::SystemUi,
            _ => return Err(GenericFontError::InvalidGeneric),
        };
        let mut families = env.font_cx.collection.register_fonts(
            parley::fontique::Blob::new(std::sync::Arc::new(bytes.clone())),
            None,
        );
        if families.is_empty() {
            return Err(GenericFontError::InvalidFont);
        }
        families.sort_by_key(|(id, _)| {
            env.font_cx
                .collection
                .family_name(*id)
                .unwrap_or("")
                .to_owned()
        });
        count += families.len();
        env.font_cx
            .collection
            .set_generic_families(generic, families.into_iter().map(|(id, _)| id));
    }
    Ok(count)
}

/// Lay out a single text block via parley. `origin` is the block's
/// top-left in packet-local coordinates; emitted glyph runs and
/// interaction regions are translated into that frame.
pub fn layout_text_block(
    env: &mut LayoutEnvironment,
    flattened: &Flattened,
    base: &TextBaseStyle,
    link_color: [f32; 4],
    code_color: [f32; 4],
    available_width: f32,
    origin: Point,
    fonts: &mut FontInterner,
) -> LaidOutText {
    layout_text_block_with_link_identity_base(
        env,
        flattened,
        base,
        link_color,
        code_color,
        available_width,
        origin,
        fonts,
        SemanticInteractionId::from_lowered_ordinal(1),
    )
}

fn rgb(color: [u8; 3]) -> [f32; 4] {
    [
        color[0] as f32 / 255.0,
        color[1] as f32 / 255.0,
        color[2] as f32 / 255.0,
        1.0,
    ]
}

/// Retain a friendly requested-family label for a glyph run. Inline code
/// uses generic monospace; a configured block code family retains its request.
/// Rendering uses the actual shaped face from the separate font sidecar.
fn family_label_from_brush(brush: InlineStyle, base: &TextBaseStyle) -> String {
    if (brush.monospace && !base.monospace)
        || (base.monospace && matches!(base.font_family.as_str(), "" | "system-ui" | "monospace"))
    {
        "monospace".to_string()
    } else {
        base.font_family.clone()
    }
}

#[cfg(test)]
#[path = "text/tests.rs"]
mod tests;

#[path = "text/layout.rs"]
mod layout;

pub(crate) use layout::layout_text_block_with_link_identity_base;
