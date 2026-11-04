// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

/// The document layouter supplies a distinct `link_identity_base` for each
/// lowered text block. The public one-block helper above starts at one for
/// callers that do not have document-level allocation.
pub(crate) fn layout_text_block_with_link_identity_base(
    env: &mut LayoutEnvironment,
    flattened: &Flattened,
    base: &TextBaseStyle,
    link_color: [f32; 4],
    code_color: [f32; 4],
    available_width: f32,
    origin: Point,
    fonts: &mut FontInterner,
    link_identity_base: SemanticInteractionId,
) -> LaidOutText {
    let display_scale = 1.0_f32;
    let quantize = true;

    let mut builder =
        env.layout_cx
            .ranged_builder(&mut env.font_cx, &flattened.text, display_scale, quantize);

    // Block-level defaults: font family, size, line height, weight, style.
    // A code role may request a concrete face/fallback stack. Preserve the
    // old bare monospace switch for callers using the default body family,
    // while allowing an explicit code family to reach Parley's matcher.
    let body_family =
        if base.monospace && matches!(base.font_family.as_str(), "" | "system-ui" | "monospace") {
            FontFamily::from(GenericFamily::Monospace)
        } else {
            FontFamily::Source(Cow::Borrowed(&base.font_family))
        };
    builder.push_default(body_family);
    builder.push_default(StyleProperty::FontSize(base.font_size));
    builder.push_default(LineHeight::FontSizeRelative(base.line_height_ratio));
    if base.bold {
        builder.push_default(StyleProperty::FontWeight(parley::FontWeight::BOLD));
    }
    if base.italic {
        builder.push_default(StyleProperty::FontStyle(parley::FontStyle::Italic));
    }
    // Brush default — parley needs a default brush even if we don't paint.
    builder.push_default(StyleProperty::Brush(InlineStyle::NORMAL));

    // Per-range styling: walk the flattened ranges and push only the
    // attributes each range adds beyond the block-level base.
    for (range, style) in &flattened.styles {
        if style.bold && !base.bold {
            builder.push(
                StyleProperty::FontWeight(parley::FontWeight::BOLD),
                range.clone(),
            );
        }
        if style.italic && !base.italic {
            builder.push(
                StyleProperty::FontStyle(parley::FontStyle::Italic),
                range.clone(),
            );
        }
        if style.monospace && !base.monospace {
            builder.push(FontFamily::from(GenericFamily::Monospace), range.clone());
        }
        // Links don't have an explicit visual style here; the
        // InteractionRegion carries the URL. A future slice could push an
        // Underline + colored brush for the link range.
        builder.push(StyleProperty::Brush(*style), range.clone());
    }

    let mut layout = builder.build(&flattened.text);
    // `NoWrap` roles (e.g. code blocks) lay out on their natural width and
    // overflow horizontally for the host to scroll; `Wrap` constrains to the
    // available content width. Explicit line breaks are kept either way.
    let wrap_width = match base.wrap {
        WrapPolicy::Wrap => Some(available_width),
        WrapPolicy::NoWrap => None,
    };
    layout.break_all_lines(wrap_width);
    let alignment = match base.alignment {
        BlockAlignment::Start => Alignment::Start,
        BlockAlignment::Center => Alignment::Center,
        BlockAlignment::End => Alignment::End,
    };
    layout.align(alignment, AlignmentOptions::default());

    // Walk lines, accumulating Y positions ourselves (parley gives us
    // baseline + line_height per line; the line's top is baseline minus
    // the metrics ascent, which we approximate via cumulative line height).
    let mut glyph_runs: Vec<GlyphRun> = Vec::new();
    let mut interactions: Vec<InteractionRegion> = Vec::new();
    let mut line_top_y = 0.0_f32;
    let mut searchable = SearchableText {
        text: flattened.text.clone(),
        clusters: Vec::new(),
    };

    for line in layout.lines() {
        let line_metrics = line.metrics();
        let line_height = line_metrics.line_height;
        let baseline_in_line = line_metrics.baseline - line_top_y;
        let mut cluster_x = origin.x + line_metrics.offset;
        for run in line.runs() {
            for cluster in run.visual_clusters() {
                let width = cluster.advance();
                searchable.clusters.push((
                    cluster.text_range(),
                    Rect::from_xywh(cluster_x, origin.y + line_top_y, width, line_height),
                ));
                cluster_x += width;
            }
        }

        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(parley_run) = item else {
                continue;
            };
            let run = parley_run.run();
            let font_size = run.font_size();
            let advance_x = parley_run.offset();
            let baseline_y = parley_run.baseline();

            let mut glyphs: Vec<PositionedGlyph> = Vec::new();
            for glyph in parley_run.positioned_glyphs() {
                glyphs.push(PositionedGlyph {
                    glyph_id: glyph.id,
                    x: glyph.x - advance_x,
                    y: glyph.y - baseline_y,
                    advance: glyph.advance,
                });
            }

            let run_origin = Point::new(origin.x + advance_x, origin.y + line_top_y);

            // Source the face from parley's actual choice (after any
            // fallback), not from the requested attrs — the glyph ids
            // above index into *this* face. `font_attrs` below is the
            // *requested* family/weight/style, kept only for a11y/debug.
            let font_face = fonts.intern(run.font());

            let attrs = run.font_attrs();
            let weight_value = attrs.weight.value() as u16;
            let font_style = if matches!(attrs.style, parley::FontStyle::Italic) {
                TextStyle::Italic
            } else {
                TextStyle::Normal
            };

            // Per-run color by brush role: links and inline code carry their
            // own token colors; everything else paints in the block's base
            // color. parley already segments runs at brush boundaries, so a
            // link / inline-code span is its own run here.
            let brush = parley_run.style().brush;
            let source_colors = matches!(base.source_presentation, SourcePresentation::Respect);
            let run_color = if source_colors && brush.foreground.is_some() {
                rgb(brush.foreground.expect("checked above"))
            } else if brush.link {
                link_color
            } else if brush.monospace {
                code_color
            } else {
                base.color
            };

            glyph_runs.push(GlyphRun {
                origin: run_origin,
                font_size,
                font_face,
                font_family: family_label_from_brush(brush, base),
                font_weight: weight_value,
                font_style,
                glyphs,
                baseline_y: baseline_in_line,
                color: run_color,
                background: source_colors.then(|| brush.background.map(rgb)).flatten(),
                underline: source_colors && brush.underline,
            });
        }

        // Typed interaction regions: walk the line's text range, intersect
        // with each annotation, emit a rect for the matched segment.
        let line_text_range = line.text_range();
        let typed = flattened
            .links
            .iter()
            .enumerate()
            .map(|(link_index, link)| {
                (
                    &link.range,
                    link.kind.clone(),
                    Some(LinkSemantics {
                        identity: link_identity_base.offset(link_index),
                        accessible_label: link.accessible_label.clone(),
                    }),
                )
            })
            .chain(flattened.submissions.iter().map(|(range, target)| {
                (
                    range,
                    InteractionKind::Submit {
                        target: target.clone(),
                    },
                    None,
                )
            }));
        for (interaction_range, kind, link_semantics) in typed {
            let intersect_start = interaction_range.start.max(line_text_range.start);
            let intersect_end = interaction_range.end.min(line_text_range.end);
            if intersect_start >= intersect_end {
                continue;
            }
            let mut min_x = f32::MAX;
            let mut max_x = f32::MIN;
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(parley_run) = item else {
                    continue;
                };
                let run_text_range = parley_run.run().text_range();
                let run_intersect_start = intersect_start.max(run_text_range.start);
                let run_intersect_end = intersect_end.min(run_text_range.end);
                if run_intersect_start >= run_intersect_end {
                    continue;
                }
                let advance_x = parley_run.offset();
                // v1 approximation: if any portion of this run is in the
                // link range, include the whole run's advance. A future
                // slice can do per-cluster geometry.
                min_x = min_x.min(advance_x);
                let run_advance: f32 = parley_run.positioned_glyphs().map(|g| g.advance).sum();
                max_x = max_x.max(advance_x + run_advance);
            }
            if min_x < max_x {
                interactions.push(InteractionRegion {
                    bounds: Rect::from_xywh(
                        origin.x + min_x,
                        origin.y + line_top_y,
                        max_x - min_x,
                        line_height,
                    ),
                    kind,
                    link_semantics: link_semantics.clone(),
                });
            }
        }

        line_top_y += line_height;
    }

    let total_height = layout.height();
    let total_width = layout.width();

    LaidOutText {
        searchable,
        glyph_runs,
        total_size: Size::new(total_width, total_height),
        interactions,
    }
}
