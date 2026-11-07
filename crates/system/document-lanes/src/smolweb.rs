/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Retained smolweb documents through Genet's engine-native document path.
//!
//! Nematic parses protocol content into an [`inker::EngineDocument`].
//! `document-canvas` owns layout, visible-band derivation, link regions, and
//! PaintList lowering. This module retains that packet plus viewport scroll
//! and exposes the existing session API to Pelt and Mere.

use document_canvas::{DictionaryBlob, SegmenterDictionaries};
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::sync::Arc;

#[cfg(test)]
use document_canvas::DocumentRenderPacket;
use document_canvas::{
    ColorVocabulary, DecodedImage, DocumentStyleSheet, InteractionKind, LaidOutDocument, Rect,
    SemanticInteractionId, SourcePresentation, Viewport,
    netrender_backend::scene_from_packet_with_images,
};
#[cfg(feature = "smolweb")]
use genet_host_api::ResourceFetcher;
use image::GenericImageView;
use inker::session_engine::{SessionClick, SessionFocusDirection};
use inker::{Block, EngineDocument, FoldKey, FoldState, SessionScrollKey};
#[cfg(feature = "smolweb")]
use inker::{Engine, EngineInput, InlineSpan, inline_text};
use netrender::Scene;

mod a11y;
pub(crate) use a11y::NodeIds as AccessibilityNodeIds;

// The smolweb palette and theme are the lanes' own (engine split D16, D19);
// engine-native callers may configure [`DocumentStyleSheet`] directly through
// [`SmolwebDocument::from_document`].
mod palette;
pub use palette::{ModeProfile, SmolwebPalette, SmolwebTheme};

/// Host policy for promoting image-shaped gemtext links into inline images.
///
/// The generic Genet lane defaults this off: gemtext specifies links, while
/// embedding them is a browser presentation choice. Product hosts opt in at
/// engine registration and retain explicit fetch and decode budgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SmolwebInlineMediaPolicy {
    pub enabled: bool,
    pub max_images: usize,
    pub max_encoded_bytes_per_image: usize,
    pub max_decoded_bytes_per_image: usize,
    pub max_dimension: u32,
}

impl SmolwebInlineMediaPolicy {
    pub fn images() -> Self {
        Self {
            enabled: true,
            max_images: 8,
            max_encoded_bytes_per_image: 8 * 1024 * 1024,
            max_decoded_bytes_per_image: 64 * 1024 * 1024,
            max_dimension: 8192,
        }
    }
}

impl Default for SmolwebInlineMediaPolicy {
    fn default() -> Self {
        let mut policy = Self::images();
        policy.enabled = false;
        policy
    }
}

/// A retained engine document, its document-canvas layout, and host viewport.
pub struct SmolwebDocument {
    document: Arc<EngineDocument>,
    /// Supplied response MIME, distinct from the lowerer's canonical output type.
    source_content_type: Option<String>,
    style: DocumentStyleSheet,
    reading: Option<crate::ReadingPreferences>,
    find: crate::reading_find::ReadingFind,
    pending_reading: Option<crate::ReadingAnchor>,
    background: [f32; 4],
    /// The theme this appearance's colors came from, when it was built from
    /// one, and the host mode they were derived under.
    theme: Option<SmolwebTheme>,
    color_mode: ModeProfile,
    images: HashMap<String, DecodedImage>,
    inline_media: SmolwebInlineMediaPolicy,
    layout: Option<LaidOutDocument>,
    fonts: Vec<Vec<u8>>,
    generic_fonts: Vec<(String, Vec<u8>)>,
    /// Segmenter dictionaries the host has loaded; shared, never fetched here.
    dictionaries: Vec<DictionaryBlob>,
    size: (u32, u32),
    scroll_x: f32,
    scroll_y: f32,
    /// A semantic snapshot is only meaningful after the layout has crossed a
    /// completed presentation boundary. Layout may exist earlier for sizing or
    /// hit-testing, but that is not an a11y publication event.
    presented: bool,
    /// The reader's fold overrides, keyed by heading so they survive
    /// re-lowering (plan decision 10). Session-only.
    folds: FoldState,
    /// Keyboard focus, never a block or region index.
    focus: Option<Focus>,
    /// In-page activations the host has not drained.
    in_page: Vec<InPageNavigation>,
    /// A reveal requested before any layout, applied by the first one.
    pending_reveal: Option<usize>,
    /// Counts layouts built, so the accessibility projection's revision
    /// changes with every new geometry.
    layout_generation: u64,
    /// Last paint admission refusal for the infallible document-session API.
    last_render_error: Option<String>,
}

/// One in-page activation, queued for the host to drain and reflect in its
/// address (plan decision 1). The session never touches address or history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InPageNavigation {
    /// A declared name resolving to `block`; `None` leaves the address alone.
    pub fragment: Option<String>,
    /// The revealed top-level block, in the document current when queued.
    pub block: usize,
}

/// A keyboard focus stop, keyed to survive relayout, toggles and streaming.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Focus {
    Fold(FoldKey),
    /// Network and in-page links share one identity counter.
    Link(SemanticInteractionId),
    /// Submissions carry no identity: their target and occurrence.
    Submit(String, usize),
}

/// One visible logical link recovered from the retained document-canvas
/// packet. This stays crate-local: Reader owns the public snapshot shape.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RetainedAccessibleLink {
    pub identity: SemanticInteractionId,
    pub label: String,
    pub url: String,
    pub rects: Vec<[f32; 4]>,
}

/// Collect image resources from every structural block shape in document
/// order. Keeping this traversal shared ensures discovery, admission, and
/// replacement retention agree about which images belong to the document.
fn image_urls(blocks: &[Block]) -> Vec<String> {
    fn visit(blocks: &[Block], seen: &mut HashSet<String>, urls: &mut Vec<String>) {
        for block in blocks {
            match block {
                Block::Image { url, .. } => {
                    if seen.insert(url.clone()) {
                        urls.push(url.clone());
                    }
                },
                Block::Presented { block, .. } => visit(std::slice::from_ref(block), seen, urls),
                Block::Quote { blocks } => visit(blocks, seen, urls),
                Block::List { items, .. } => {
                    for item in items {
                        visit(item, seen, urls);
                    }
                },
                _ => {},
            }
        }
    }

    let mut seen = HashSet::new();
    let mut urls = Vec::new();
    visit(blocks, &mut seen, &mut urls);
    urls
}

/// The scroll offset that brings `start..end` into a viewport of `extent`,
/// moving as little as possible.
fn into_view(scroll: f32, start: f32, end: f32, extent: f32) -> f32 {
    if start < scroll {
        start
    } else if end > scroll + extent {
        (end - extent).min(start)
    } else {
        scroll
    }
}

fn rect_contains(rect: Rect, x: f32, y: f32) -> bool {
    x >= rect.origin.x && x <= rect.max_x() && y >= rect.origin.y && y <= rect.max_y()
}

#[cfg(feature = "smolweb")]
fn promote_inline_image_links(document: &mut EngineDocument, policy: SmolwebInlineMediaPolicy) {
    if !policy.enabled || !is_gemtext_document(document) {
        return;
    }

    let base_address = document.address.clone();
    let mut promoted = 0;
    for block in &mut document.blocks {
        if promoted >= policy.max_images {
            break;
        }
        let Some((href, alt)) = image_link(block) else {
            continue;
        };
        let resolved = genet_host_api::resolve_href(&base_address, &href);
        if !looks_like_image_url(&resolved) {
            continue;
        }
        *block = Block::Image { url: resolved, alt };
        promoted += 1;
    }
}

#[cfg(feature = "smolweb")]
fn is_gemtext_document(document: &EngineDocument) -> bool {
    document
        .content_type
        .split(';')
        .next()
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("text/gemini"))
}

#[cfg(feature = "smolweb")]
fn image_link(block: &Block) -> Option<(String, String)> {
    let Block::Paragraph { spans } = block else {
        return None;
    };
    let [InlineSpan::Link { url, spans, .. }] = spans.as_slice() else {
        return None;
    };
    Some((url.clone(), inline_text(spans)))
}

/// `url` without its fragment: a fragment names a place in the document and is
/// never sent. The engine still sees the whole address, so a feed can render
/// the entry a fragment names (smolweb fidelity plan WS4, R4).
#[cfg(feature = "smolweb")]
fn without_fragment(url: &str) -> &str {
    url.split_once('#').map_or(url, |(base, _)| base)
}

#[cfg(feature = "smolweb")]
fn looks_like_image_url(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let extension = path.rsplit_once('.').map(|(_, extension)| extension);
    extension.is_some_and(|extension| {
        matches!(
            extension.to_ascii_lowercase().as_str(),
            "avif" | "bmp" | "gif" | "ico" | "jfif" | "jpeg" | "jpg" | "png" | "webp"
        )
    })
}

fn decode_image(bytes: &[u8], policy: SmolwebInlineMediaPolicy) -> Option<DecodedImage> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(policy.max_dimension);
    limits.max_image_height = Some(policy.max_dimension);
    limits.max_alloc = Some(policy.max_decoded_bytes_per_image as u64);
    reader.limits(limits);
    let decoded = reader.decode().ok()?;
    let (width, height) = decoded.dimensions();
    let rgba_bytes = u64::from(width)
        .checked_mul(u64::from(height))?
        .checked_mul(4)?;
    if rgba_bytes > policy.max_decoded_bytes_per_image as u64 {
        return None;
    }
    Some(DecodedImage {
        width,
        height,
        rgba8: decoded.to_rgba8().into_raw().into(),
    })
}

#[cfg(feature = "smolweb")]
fn lower(input: &EngineInput) -> EngineDocument {
    let scheme = input
        .address
        .split_once("://")
        .map(|(scheme, _)| scheme)
        .unwrap_or("");
    let primary = input
        .content_type
        .as_deref()
        .and_then(|value| value.split(';').next())
        .map(|value| value.trim().to_ascii_lowercase());
    // MIME refinements already named by EngineRoutePolicy take precedence over
    // the carrier. In particular a literal response is never sniffed as markup.
    let by_type: Option<Box<dyn Engine>> = match primary.as_deref() {
        Some("text/plain") => Some(Box::new(nematic::TextEngine::new())),
        Some("text/markdown" | "text/x-markdown") => Some(Box::new(nematic::MarkdownEngine::new())),
        Some("text/gemini") => Some(Box::new(nematic::GemtextEngine::new())),
        Some("text/scroll" | "text/x-scroll") => Some(Box::new(nematic::ScrollEngine::new())),
        Some("application/gopher-menu") => Some(Box::new(nematic::GopherEngine::new())),
        Some("text/x-finger") => Some(Box::new(nematic::FingerEngine::new())),
        Some("application/x-nex") => Some(Box::new(nematic::NexEngine::new())),
        Some("application/x-guppy") => Some(Box::new(nematic::GuppyEngine::new())),
        Some("application/x-titan") => Some(Box::new(nematic::TitanEngine::new())),
        Some("message/x-misfin") => Some(Box::new(nematic::MisfinEngine::new())),
        Some(
            "application/rss+xml"
            | "application/atom+xml"
            | "application/feed+xml"
            | "application/feed+json",
        ) => Some(Box::new(nematic::FeedEngine::new())),
        Some("application/xml" | "text/xml") if looks_like_feed(&input.body) => {
            Some(Box::new(nematic::FeedEngine::new()))
        },
        _ => None,
    };
    let engine: Box<dyn Engine> = by_type.unwrap_or_else(|| match scheme {
        "gopher" => Box::new(nematic::GopherEngine::new()),
        "nex" => Box::new(nematic::NexEngine::new()),
        "finger" => Box::new(nematic::FingerEngine::new()),
        "spartan" => Box::new(nematic::SpartanEngine::new()),
        "titan" => Box::new(nematic::TitanEngine::new()),
        "misfin" => Box::new(nematic::MisfinEngine::new()),
        "guppy" => Box::new(nematic::GuppyEngine::new()),
        "scroll" => Box::new(nematic::ScrollEngine::new()),
        _ if looks_like_feed(&input.body) => Box::new(nematic::FeedEngine::new()),
        _ => Box::new(nematic::GemtextEngine::new()),
    });
    engine.render(input).unwrap_or_else(|_| {
        nematic::GemtextEngine::new()
            .render(input)
            .expect("gemtext lowering is infallible")
    })
}

#[cfg(feature = "smolweb")]
fn looks_like_feed(body: &str) -> bool {
    let body = body.trim_start();
    body.starts_with("<?xml") || body.starts_with("<rss") || body.starts_with("<feed")
}

/// The content types whose whole document is a fixed-width menu, so its body
/// font has to be the monospace one.
///
/// A nex listing's text lines lower to `Preformatted` and carry ASCII art and
/// column alignment, but its link lines lower to a `Paragraph` with a link
/// span, because `Preformatted` holds text and cannot hold a link. Those lines
/// would otherwise take the body serif and break the very column grid the
/// lines above and below them establish. A gopher menu now lowers to one
/// `Block::Menu`, which document-canvas sets in the monospace face itself; it
/// stays listed so anything else in the document keeps the same face.
const FIXED_WIDTH_MENU_TYPES: &[&str] = &["application/gopher-menu", "application/x-nex-listing"];

fn style_for_theme(
    theme: &SmolwebTheme,
    url: &str,
    content_type: &str,
) -> (DocumentStyleSheet, [f32; 4]) {
    let mut style = DocumentStyleSheet::default();
    // These values are user-facing defaults rather than host chrome. Hosts
    // can still pass an explicit sheet through `from_document`.
    style.mono_font_family = "monospace".into();
    style.body_font_family = if FIXED_WIDTH_MENU_TYPES.contains(&content_type) {
        style.mono_font_family.clone()
    } else {
        "serif, sans-serif, monospace".into()
    };
    // Reader, small-web and normalized document projections share a prose
    // baseline. This does not restyle an authored HTML browser session.
    style.body_font_size = 18.0;
    style.line_height_ratio = 1.6;
    style.horizontal_padding = 24.0;
    // About 65-75 Latin characters with the ordinary proportional faces.
    // The layout always clamps this to the current appearance's viewport.
    style.max_content_width = Some(36.0 * style.body_font_size);
    style.roles.heading.sizes = [32.0, 26.0, 22.0, 20.0, 18.0, 18.0];
    style.roles.heading.line_height_ratio = Some(1.25);
    style.roles.code.size = document_canvas::SizeSpec::ScaleOfBody(0.85);
    style.roles.metadata.size = document_canvas::SizeSpec::ScaleOfBody(0.75);
    style.vertical_padding = 24.0;
    let (colors, background) = theme_colors(theme, url, ModeProfile::LIGHT);
    style.colors = colors;
    (style, background)
}

fn theme_colors(theme: &SmolwebTheme, url: &str, mode: ModeProfile) -> (ColorVocabulary, [f32; 4]) {
    let palette = SmolwebPalette::for_mode(theme, url, mode);
    let defaults = ColorVocabulary::default();
    let background = parse_color(&palette.bg).unwrap_or([1.0, 1.0, 1.0, 1.0]);
    let foreground = parse_color(&palette.fg).unwrap_or(defaults.body_text);
    let link = parse_color(&palette.link).unwrap_or(defaults.link_text);
    let quote = parse_color(&palette.quote).unwrap_or(defaults.badge_text);
    let pre = parse_color(&palette.pre_bg).unwrap_or(defaults.placeholder_image);
    let colors = ColorVocabulary {
        body_text: foreground,
        heading_text: foreground,
        link_text: link,
        code_text: foreground,
        badge_text: quote,
        rule: quote,
        placeholder_text: foreground,
        placeholder_image: pre,
    };
    (colors, background)
}

fn parse_color(value: &str) -> Option<[f32; 4]> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        let (r, g, b) = match hex.len() {
            3 => {
                let mut chars = hex.chars();
                let expand = |c: char| u8::from_str_radix(&format!("{c}{c}"), 16).ok();
                (
                    expand(chars.next()?)?,
                    expand(chars.next()?)?,
                    expand(chars.next()?)?,
                )
            },
            6 => (
                u8::from_str_radix(&hex[0..2], 16).ok()?,
                u8::from_str_radix(&hex[2..4], 16).ok()?,
                u8::from_str_radix(&hex[4..6], 16).ok()?,
            ),
            _ => return None,
        };
        return Some([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]);
    }
    let body = value.strip_prefix("rgb(")?.strip_suffix(')')?;
    let channels: Vec<u8> = body
        .split(',')
        .map(|channel| channel.trim().parse())
        .collect::<Result<_, _>>()
        .ok()?;
    (channels.len() == 3).then(|| {
        [
            channels[0] as f32 / 255.0,
            channels[1] as f32 / 255.0,
            channels[2] as f32 / 255.0,
            1.0,
        ]
    })
}

#[cfg(all(test, feature = "smolweb"))]
#[path = "smolweb/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "smolweb/typography_tests.rs"]
mod typography_tests;

#[cfg(test)]
#[path = "smolweb/reading_anchor_tests.rs"]
mod reading_anchor_tests;

#[path = "smolweb/construction.rs"]
mod construction;

#[path = "smolweb/appearance.rs"]
mod appearance;

#[path = "smolweb/navigation.rs"]
mod navigation;

#[path = "smolweb/rendering.rs"]
mod rendering;
