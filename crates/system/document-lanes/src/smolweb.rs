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

// The smolweb palette and theme are tabard's. Hosts on the compatibility
// palette (current Pelt and Mere) keep these paths; new engine-native callers
// may configure [`DocumentStyleSheet`] directly through
// [`SmolwebDocument::from_document`].
pub use tabard::smolweb::{SmolwebPalette, SmolwebTheme};

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
    reading: Option<tabard::ReadingPreferences>,
    find: crate::reading_find::ReadingFind,
    pending_reading: Option<tabard::ReadingAnchor>,
    background: [f32; 4],
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

impl SmolwebDocument {
    /// Fetch `url` through the host fetcher, then lower and retain the body.
    #[cfg(feature = "smolweb")]
    pub fn load(
        fetcher: &impl ResourceFetcher,
        url: &str,
        theme: SmolwebTheme,
    ) -> Result<Self, String> {
        Self::load_input_with_inline_media(
            fetcher,
            url,
            None,
            theme,
            SmolwebInlineMediaPolicy::default(),
        )
    }

    /// Fetch a document and apply the host's inline-media presentation policy.
    #[cfg(feature = "smolweb")]
    pub fn load_with_inline_media(
        fetcher: &impl ResourceFetcher,
        url: &str,
        theme: SmolwebTheme,
        policy: SmolwebInlineMediaPolicy,
    ) -> Result<Self, String> {
        Self::load_input_with_inline_media(fetcher, url, None, theme, policy)
    }

    #[cfg(feature = "smolweb")]
    pub(crate) fn load_input_with_inline_media(
        fetcher: &impl ResourceFetcher,
        url: &str,
        fallback_content_type: Option<&str>,
        theme: SmolwebTheme,
        policy: SmolwebInlineMediaPolicy,
    ) -> Result<Self, String> {
        let response = fetcher
            .fetch_response(without_fragment(url))
            .ok_or_else(|| format!("could not load {url}"))?;
        // Keep the existing requested document/fragment identity. Redirect and
        // effective-base policy remain with the embedding host.
        let mut input = EngineInput::new(url, String::from_utf8_lossy(&response.bytes));
        input.content_type = response
            .content_type
            .or_else(|| fallback_content_type.map(str::to_owned));
        Ok(Self::parse_input_with_inline_media(&input, theme, policy))
    }

    /// Lower already-fetched content through the matching Nematic engine.
    #[cfg(feature = "smolweb")]
    pub fn parse(url: &str, body: &str, theme: SmolwebTheme) -> Self {
        Self::parse_input(&EngineInput::new(url, body), theme)
    }

    /// Lower supplied text and response MIME without performing a fetch.
    #[cfg(feature = "smolweb")]
    pub fn parse_input(input: &EngineInput, theme: SmolwebTheme) -> Self {
        Self::parse_input_with_inline_media(input, theme, SmolwebInlineMediaPolicy::default())
    }

    /// Lower an already-fetched body and expose eligible linked images for the
    /// host to resolve through [`SmolwebDocument::subresources`].
    #[cfg(feature = "smolweb")]
    pub fn parse_with_inline_media(
        url: &str,
        body: &str,
        theme: SmolwebTheme,
        policy: SmolwebInlineMediaPolicy,
    ) -> Self {
        Self::parse_input_with_inline_media(&EngineInput::new(url, body), theme, policy)
    }

    /// Preserve supplied response MIME while applying host inline-media policy.
    #[cfg(feature = "smolweb")]
    pub fn parse_input_with_inline_media(
        input: &EngineInput,
        theme: SmolwebTheme,
        policy: SmolwebInlineMediaPolicy,
    ) -> Self {
        let mut document = lower(input);
        promote_inline_image_links(&mut document, policy);
        let (style, background) = style_for_theme(&theme, &input.address, &document.content_type);
        let mut retained =
            Self::from_document_with_media_policy(document, style, background, policy);
        retained.source_content_type = input.content_type.clone();
        retained
    }

    /// Retain an already-lowered document with an explicit host style.
    pub fn from_document(
        document: EngineDocument,
        style: DocumentStyleSheet,
        background: [f32; 4],
    ) -> Self {
        Self::from_document_with_media_policy(
            document,
            style,
            background,
            SmolwebInlineMediaPolicy::default(),
        )
    }

    /// Retain a portable document using the same host-theme mapping as the
    /// existing document lane.
    pub fn from_document_with_theme(document: EngineDocument, theme: SmolwebTheme) -> Self {
        let (style, background) =
            style_for_theme(&theme, &document.address, &document.content_type);
        Self::from_document(document, style, background)
    }

    /// Retain a shared portable packet while keeping this appearance's layout,
    /// viewport, scroll and inline-media cache independent.
    pub fn from_shared_document_with_theme(
        document: Arc<EngineDocument>,
        theme: SmolwebTheme,
    ) -> Self {
        let (style, background) =
            style_for_theme(&theme, &document.address, &document.content_type);
        Self::from_shared_document(
            document,
            style,
            background,
            SmolwebInlineMediaPolicy::default(),
        )
    }

    fn from_document_with_media_policy(
        document: EngineDocument,
        style: DocumentStyleSheet,
        background: [f32; 4],
        inline_media: SmolwebInlineMediaPolicy,
    ) -> Self {
        Self::from_shared_document(Arc::new(document), style, background, inline_media)
    }

    fn from_shared_document(
        document: Arc<EngineDocument>,
        style: DocumentStyleSheet,
        background: [f32; 4],
        inline_media: SmolwebInlineMediaPolicy,
    ) -> Self {
        Self {
            document,
            source_content_type: None,
            style,
            reading: None,
            find: Default::default(),
            pending_reading: None,
            background,
            images: HashMap::new(),
            inline_media,
            layout: None,
            fonts: Vec::new(),
            generic_fonts: Vec::new(),
            dictionaries: Vec::new(),
            size: (0, 0),
            scroll_x: 0.0,
            scroll_y: 0.0,
            presented: false,
            folds: FoldState::default(),
            focus: None,
            in_page: Vec::new(),
            pending_reveal: None,
            layout_generation: 0,
            last_render_error: None,
        }
    }

    /// The portable document retained by this session.
    /// The retained appearance's style. Source, fold/focus state and viewport
    /// are separate from this presentation policy.
    pub fn style_sheet(&self) -> &DocumentStyleSheet {
        &self.style
    }

    /// Replace only style and invalidate measured geometry. Document identity,
    /// images, folds, focus and scroll remain owned by this retained session.
    pub fn set_style_sheet(&mut self, style: DocumentStyleSheet) -> bool {
        if self.style == style {
            return false;
        }
        self.style = style;
        self.invalidate_layout();
        true
    }

    pub fn source_document(&self) -> Arc<EngineDocument> { self.document.clone() }
    /// Fresh viewport/search/reading state over the same source and held fonts.
    pub fn new_appearance(&self) -> Self {
        let mut appearance = Self::from_shared_document(
            self.document.clone(),
            self.style.clone(),
            self.background,
            self.inline_media,
        );
        appearance.source_content_type = self.source_content_type.clone();
        appearance.fonts = self.fonts.clone();
        appearance.generic_fonts = self.generic_fonts.clone();
        appearance.dictionaries = self.dictionaries.clone();
        appearance.images = self.images.clone();
        appearance
    }
    pub fn reading_anchor(&self) -> Option<tabard::ReadingAnchor> {
        let layout = self.layout.as_ref()?;
        crate::reading_anchor::capture(&self.document.address, &layout.searchable, self.scroll_y, 1.0)
    }
    /// Applied once after layout. A stale quote never jumps to unrelated text.
    pub fn restore_reading_anchor(&mut self, anchor: tabard::ReadingAnchor) {
        self.pending_reading = Some(anchor);
        self.invalidate_layout();
    }
    fn retain_reading_anchor(&mut self) {
        if self.pending_reading.is_none() { self.pending_reading = self.reading_anchor(); }
    }
    pub fn reading_preferences(&self) -> tabard::ReadingPreferences { self.reading.unwrap_or_default() }
    pub fn set_reading_preferences(&mut self, value: tabard::ReadingPreferences) -> Result<bool, String> {
        value.validate()?;
        if Some(value) == self.reading { return Ok(false); }
        self.reading = Some(value);
        self.invalidate_layout();
        Ok(true)
    }
    pub fn document_find(&mut self, query: &inker::DocumentFindQuery) -> inker::DocumentFindState {
        self.ensure_layout(self.size.0.max(1), self.size.1.max(1));
        self.find.replace(query.clone(), &self.layout.as_ref().unwrap().searchable);
        self.reveal_find();
        self.find.state()
    }
    pub fn document_find_step(&mut self, direction: inker::DocumentFindDirection) -> inker::DocumentFindState {
        self.ensure_layout(self.size.0.max(1), self.size.1.max(1));
        self.find.step(direction);
        self.reveal_find();
        self.find.state()
    }
    pub fn clear_document_find(&mut self) { self.find = Default::default(); }
    fn reveal_find(&mut self) {
        if let Some(y) = self.find.current_y() {
            self.scroll_y = (y - 24.0).clamp(0.0, self.max_scroll());
        }
    }

    pub fn document(&self) -> &EngineDocument {
        &self.document
    }

    /// Select whether this retained appearance honors source-specified inline
    /// colors and underlines. This changes only presentation, so the shared
    /// source document and its semantics remain untouched.
    pub fn set_source_presentation(&mut self, presentation: SourcePresentation) {
        if self.style.source_presentation == presentation {
            return;
        }
        self.style.source_presentation = presentation;
        self.layout = None;
        self.presented = false;
    }

    pub fn source_presentation(&self) -> SourcePresentation {
        self.style.source_presentation
    }

    /// Replace an in-flight document body without replacing its live session.
    ///
    /// Host-selected styling, viewport, and scroll position remain stable.
    /// Lowered structure and inline-media requests are rebuilt from the exact
    /// prefix now available; decoded images survive only while still named by
    /// the replacement document.
    #[cfg(feature = "smolweb")]
    pub fn replace_body(&mut self, url: &str, body: &str) {
        let mut input = EngineInput::new(url, body);
        if url == self.document.address {
            input.content_type = self.source_content_type.clone();
        }
        self.replace_input(&input);
    }

    /// Replace a streamed body and its response MIME explicitly. `None` means
    /// unknown MIME; it never borrows interpretation metadata from an old page.
    #[cfg(feature = "smolweb")]
    pub fn replace_input(&mut self, input: &EngineInput) {
        let mut document = lower(input);
        promote_inline_image_links(&mut document, self.inline_media);
        self.replace_document(document);
        self.source_content_type = input.content_type.clone();
    }

    /// Replace the retained document in place with an already-lowered one,
    /// keeping style, viewport, scroll and still-named images. Fold state is
    /// reconciled by heading key, so it survives streamed prefixes and drops
    /// only for an edited heading; a new address starts fresh.
    pub fn replace_document(&mut self, document: EngineDocument) {
        self.pending_reading = if document.address == self.document.address { self.reading_anchor() } else { None };
        self.source_content_type = None;
        if document.address == self.document.address {
            self.folds.reconcile(&document.navigation);
        } else {
            self.folds = FoldState::default();
            self.focus = None;
        }
        let retained_images: HashSet<_> = image_urls(&document.blocks).into_iter().collect();
        self.images
            .retain(|url, _| retained_images.contains(url.as_str()));
        self.find = Default::default();
        self.document = Arc::new(document);
        self.pending_reveal = None;
        self.layout=None;
        self.invalidate_layout();
    }

    fn invalidate_layout(&mut self) {
        self.retain_reading_anchor();
        self.layout = None;
        self.presented = false;
    }

    /// The reader's fold state for this document.
    pub fn folds(&self) -> &FoldState {
        &self.folds
    }

    fn navigation_is_current(&self) -> bool {
        self.document.navigation.is_current(&self.document.blocks)
    }

    /// Open or close `fold` of the navigation table; false when the table
    /// has no such current fold.
    pub fn toggle_fold(&mut self, fold: usize) -> bool {
        if !self.navigation_is_current() || fold >= self.document.navigation.folds.len() {
            return false;
        }
        self.folds.toggle(&self.document.navigation, fold);
        self.invalidate_layout();
        true
    }

    /// Open `block`'s closed ancestors and scroll it to the viewport top.
    /// Later resizes keep pixel scroll (plan decision 14). Before any layout
    /// the scroll waits for the first one. False unless `block` is a
    /// top-level block of a current navigation table.
    pub fn reveal_block(&mut self, block: usize) -> bool {
        if !self.navigation_is_current() || block >= self.document.blocks.len() {
            return false;
        }
        if self.folds.open_ancestors(&self.document.navigation, block) {
            self.invalidate_layout();
        }
        match self.size {
            (0, 0) => self.pending_reveal = Some(block),
            (width, height) => {
                self.ensure_layout(width, height);
                self.scroll_to_block(block);
            },
        }
        true
    }

    /// Reveal the block `name` resolves to; false for a missing anchor.
    pub fn reveal_anchor(&mut self, name: &str) -> bool {
        let target = self
            .navigation_is_current()
            .then(|| self.document.navigation.resolve(name))
            .flatten();
        target.is_some_and(|block| self.reveal_block(block))
    }

    /// In-page activations since the last drain, oldest first.
    pub fn take_in_page_navigations(&mut self) -> Vec<InPageNavigation> {
        std::mem::take(&mut self.in_page)
    }

    fn scroll_to_block(&mut self, block: usize) {
        let top = self
            .layout
            .as_ref()
            .and_then(|layout| layout.packet.top_level_block(block))
            .map(|rendered| rendered.bounds.origin.y);
        if let Some(top) = top {
            self.scroll_y = top.clamp(0.0, self.max_scroll());
        }
    }

    /// Perform an interaction. Links and submissions are host actions;
    /// folds and in-page links are handled here and issue no request.
    pub(crate) fn activate(&mut self, kind: InteractionKind) -> SessionClick {
        match kind {
            InteractionKind::Link { url } => SessionClick::Navigate(url),
            InteractionKind::Submit { target } => SessionClick::Submit(target),
            InteractionKind::Fold { fold } => {
                self.toggle_fold(fold);
                SessionClick::Handled
            },
            InteractionKind::InPage { block, fragment } => {
                // A missing target is an inert no-op, as stock (probes 03, 04).
                if let Some(block) = block
                    && self.reveal_block(block)
                {
                    self.in_page.push(InPageNavigation { fragment, block });
                }
                SessionClick::Handled
            },
        }
    }

    /// Resolve and perform a viewport-local click.
    pub(crate) fn activate_at(&mut self, x: f32, y: f32, width: u32, height: u32) -> SessionClick {
        match self.click_at(x, y, width, height) {
            Some(kind) => self.activate(kind),
            None => SessionClick::Miss,
        }
    }

    /// Move keyboard focus through every interaction in document order,
    /// wrapping as Genet's other lanes do, and scroll the stop into view.
    pub fn focus_move(
        &mut self,
        direction: SessionFocusDirection,
        width: u32,
        height: u32,
    ) -> bool {
        self.ensure_layout(width, height);
        let stops = self.focus_stops();
        let Some(last) = stops.len().checked_sub(1) else {
            return false;
        };
        let current = self
            .focus
            .as_ref()
            .and_then(|focus| stops.iter().position(|(stop, _)| stop == focus));
        let next = match (direction, current) {
            (SessionFocusDirection::Forward, Some(index)) if index < last => index + 1,
            (SessionFocusDirection::Forward, _) => 0,
            (SessionFocusDirection::Backward, Some(index)) if index > 0 => index - 1,
            (SessionFocusDirection::Backward, _) => last,
        };
        let (focus, regions) = &stops[next];
        self.scroll_into_view(regions);
        self.focus = Some(focus.clone());
        true
    }

    /// Drop keyboard focus, as when the document loses it.
    pub fn clear_focus(&mut self) {
        self.focus = None;
    }

    /// The focused stop's interaction, laying out first if needed, for a
    /// host that shows where focus is.
    pub fn focused_interaction(&mut self, width: u32, height: u32) -> Option<InteractionKind> {
        self.ensure_layout(width, height);
        let regions = self.focused_regions()?;
        Some(
            self.layout.as_ref()?.packet.interactions[regions[0]]
                .kind
                .clone(),
        )
    }

    /// Focus stops in document order, each with its region indices.
    fn focus_stops(&self) -> Vec<(Focus, Vec<usize>)> {
        let Some(layout) = &self.layout else {
            return Vec::new();
        };
        let keys = self.document.navigation.fold_keys();
        let mut stops: Vec<(Focus, Vec<usize>)> = Vec::new();
        let mut positions: HashMap<Focus, usize> = HashMap::new();
        let mut submits: HashMap<&str, usize> = HashMap::new();
        for (index, region) in layout.packet.interactions.iter().enumerate() {
            let focus = match (&region.kind, &region.link_semantics) {
                (InteractionKind::Fold { fold }, _) => Focus::Fold(keys[*fold].clone()),
                (_, Some(semantics)) => Focus::Link(semantics.identity),
                (InteractionKind::Submit { target }, None) => {
                    // A wrapped submission's rectangles are consecutive.
                    if let Some((Focus::Submit(previous, _), regions)) = stops.last_mut()
                        && previous == target
                        && regions.last() == Some(&(index - 1))
                    {
                        regions.push(index);
                        continue;
                    }
                    let seen = submits.entry(target).or_default();
                    *seen += 1;
                    Focus::Submit(target.clone(), *seen - 1)
                },
                _ => continue,
            };
            match positions.get(&focus) {
                Some(&position) => stops[position].1.push(index),
                None => {
                    positions.insert(focus.clone(), stops.len());
                    stops.push((focus, vec![index]));
                },
            }
        }
        stops
    }

    fn focused_regions(&self) -> Option<Vec<usize>> {
        let focus = self.focus.as_ref()?;
        self.focus_stops()
            .into_iter()
            .find(|(stop, _)| stop == focus)
            .map(|(_, regions)| regions)
    }

    fn scroll_into_view(&mut self, regions: &[usize]) {
        let Some(layout) = &self.layout else {
            return;
        };
        let bounds: Vec<Rect> = regions
            .iter()
            .map(|&region| layout.packet.interactions[region].bounds)
            .collect();
        let span = |start: fn(&Rect) -> f32, end: fn(&Rect) -> f32| {
            let first = bounds.iter().map(start).fold(f32::INFINITY, f32::min);
            let last = bounds.iter().map(end).fold(f32::NEG_INFINITY, f32::max);
            (first, last)
        };
        let (top, bottom) = span(|rect| rect.origin.y, Rect::max_y);
        let (left, right) = span(|rect| rect.origin.x, Rect::max_x);
        let (width, height) = (self.size.0 as f32, self.size.1 as f32);
        self.scroll_y = into_view(self.scroll_y, top, bottom, height).clamp(0.0, self.max_scroll());
        self.scroll_x =
            into_view(self.scroll_x, left, right, width).clamp(0.0, self.max_scroll_x());
    }

    /// Outline the focused stop with the style sheet's indicator.
    fn paint_focus(&self, scene: &mut Scene) {
        let (Some(layout), Some(regions)) = (&self.layout, self.focused_regions()) else {
            return;
        };
        let indicator = self.style.focus_indicator;
        let color = self.style.token_color(indicator.color);
        for region in regions {
            let Some([x, y, width, height]) =
                self.viewport_rect(layout.packet.interactions[region].bounds)
            else {
                continue;
            };
            for edge in Rect::from_xywh(x, y, width, height).outline(indicator.width) {
                if edge.size.width > 0.0 && edge.size.height > 0.0 {
                    scene.push_rect(
                        edge.origin.x,
                        edge.origin.y,
                        edge.max_x(),
                        edge.max_y(),
                        color,
                    );
                }
            }
        }
    }

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

    /// The host's reading face. Code and column-aligned menu formats retain
    /// their monospace role. A supplied sans-serif generic can fill glyphs
    /// missing from the prose face; all font bytes remain host-owned.
    pub fn set_font_family(&mut self, family: &str) {
        if !FIXED_WIDTH_MENU_TYPES.contains(&self.document.content_type.as_str()) {
            self.style.body_font_family = format!("{family}, sans-serif, monospace");
        }
        self.layout = None;
        self.presented = false;
    }
    pub fn set_fonts(&mut self, fonts: Vec<Vec<u8>>) {
        if self.fonts != fonts {
            self.fonts = fonts;
            self.layout = None;
            self.presented = false;
        }
    }

    /// Replace host-chosen CSS generic resources atomically; retained reflow
    /// and viewport changes use this same ledger.
    pub fn set_generic_fonts(
        &mut self,
        fonts: Vec<(String, Vec<u8>)>,
    ) -> Result<bool, document_canvas::text::GenericFontError> {
        if self.generic_fonts == fonts {
            return Ok(false);
        }
        let mut validation = document_canvas::text::LayoutEnvironment::new();
        document_canvas::text::register_generic_fonts(&mut validation, &fonts)?;
        self.generic_fonts = fonts;
        self.layout = None;
        self.presented = false;
        Ok(true)
    }

    /// Replace the host's segmenter dictionaries. The current layout is redone
    /// only when its text wanted one of them.
    pub fn set_segmenter_dictionaries(&mut self, dictionaries: Vec<DictionaryBlob>) {
        let brought = dictionaries
            .iter()
            .fold(SegmenterDictionaries::default(), |held, blob| {
                let carried = blob.dictionaries();
                SegmenterDictionaries {
                    southeast_asian: held.southeast_asian || carried.southeast_asian,
                    chinese_japanese: held.chinese_japanese || carried.chinese_japanese,
                }
            });
        let wanted = self.wanted_segmenter_dictionaries();
        self.dictionaries = dictionaries;
        if (wanted.southeast_asian && brought.southeast_asian)
            || (wanted.chinese_japanese && brought.chinese_japanese)
        {
            self.layout = None;
            self.presented = false;
        }
    }

    /// Dictionaries the current layout's text met without: what the host
    /// should fetch and hand to [`Self::set_segmenter_dictionaries`].
    pub fn wanted_segmenter_dictionaries(&self) -> SegmenterDictionaries {
        self.layout
            .as_ref()
            .map(|layout| layout.wanted_dictionaries)
            .unwrap_or_default()
    }

    fn ensure_layout(&mut self, width: u32, height: u32) {
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
                    |preferences| crate::typography::document_style_for_reading(&self.style, preferences),
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
            self.pending_reading=None;
            self.scroll_to_block(block);
        } else if let Some(anchor) = self.pending_reading.take() {
            if let Some(y) = crate::reading_anchor::resolve(&self.document.address, &self.layout.as_ref().unwrap().searchable, &anchor, 1.0) {
                self.scroll_y = y.clamp(0.0, self.max_scroll());
            }
        }
    }

    fn max_scroll_x(&self) -> f32 {
        let Some(layout) = &self.layout else {
            return 0.0;
        };
        (layout.packet.content_bounds.size.width - self.size.0 as f32).max(0.0)
    }

    fn max_scroll(&self) -> f32 {
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
        self.find.paint(&mut scene, self.scroll_x, self.scroll_y, self.size);
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

    /// Move the single host-owned document viewport.
    pub fn scroll_by(&mut self, dx: f32, dy: f32) -> bool {
        if self.layout.is_none() {
            return false;
        }
        let before = (self.scroll_x, self.scroll_y);
        self.scroll_x = (self.scroll_x + dx).clamp(0.0, self.max_scroll_x());
        self.scroll_y = (self.scroll_y + dy).clamp(0.0, self.max_scroll());
        (self.scroll_x, self.scroll_y) != before
    }

    /// document-canvas has one viewport scroller, so point routing delegates
    /// to [`scroll_by`](Self::scroll_by).
    pub fn scroll_at(&mut self, _x: f32, _y: f32, dx: f32, dy: f32) -> bool {
        self.scroll_by(dx, dy)
    }

    /// Apply the established Genet keyboard-scroll vocabulary.
    pub fn scroll_for_key(&mut self, key: SessionScrollKey) -> bool {
        if self.layout.is_none() {
            return false;
        }
        let before = self.scroll_y;
        let page = self.size.1 as f32 * 0.9;
        self.scroll_y = match key {
            SessionScrollKey::LineUp => self.scroll_y - 40.0,
            SessionScrollKey::LineDown => self.scroll_y + 40.0,
            SessionScrollKey::PageUp => self.scroll_y - page,
            SessionScrollKey::PageDown => self.scroll_y + page,
            SessionScrollKey::Home => 0.0,
            SessionScrollKey::End => self.max_scroll(),
        }
        .clamp(0.0, self.max_scroll());
        self.scroll_y != before
    }

    /// Jump to an absolute full-document offset.
    pub fn scroll_to(&mut self, y: f32) {
        if self.layout.is_some() {
            self.scroll_y = y.clamp(0.0, self.max_scroll());
        }
    }

    /// Current vertical viewport offset, in laid-out document pixels.
    ///
    /// Hosts use this read-only value to observe independent document
    /// appearances. It does not force layout or change scroll state.
    pub fn scroll_y(&self) -> f32 {
        self.scroll_y
    }

    /// Current horizontal viewport offset for wide retained table packets.
    pub fn scroll_x(&self) -> f32 {
        self.scroll_x
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

    /// Viewport-space link rectangles as `[x, y, width, height]`, matching the
    /// shared retained-session contract.
    pub fn links(&self) -> Vec<(String, [f32; 4])> {
        let Some(layout) = &self.layout else {
            return Vec::new();
        };
        layout
            .packet
            .interactions
            .iter()
            .filter_map(|region| {
                let InteractionKind::Link { url } = &region.kind else {
                    return None;
                };
                let [x, y, width, height] = self.viewport_rect(region.bounds)?;
                Some((url.clone(), [x, y, width, height]))
            })
            .collect()
    }

    /// Return the current visible logical links without changing retained
    /// layout. `None` means no completed frame has made this document's
    /// geometry publishable yet.
    pub(crate) fn retained_accessible_links(&self) -> Option<Vec<RetainedAccessibleLink>> {
        if !self.presented {
            return None;
        }
        let layout = self.layout.as_ref()?;
        let mut links: Vec<RetainedAccessibleLink> = Vec::new();
        for region in &layout.packet.interactions {
            let InteractionKind::Link { url } = &region.kind else {
                continue;
            };
            let Some(semantics) = &region.link_semantics else {
                continue;
            };
            let Some(rect) = self.viewport_rect(region.bounds) else {
                continue;
            };
            if let Some(link) = links
                .iter_mut()
                .find(|link| link.identity == semantics.identity)
            {
                link.rects.push(rect);
            } else {
                links.push(RetainedAccessibleLink {
                    identity: semantics.identity,
                    label: semantics.accessible_label.clone(),
                    url: url.clone(),
                    rects: vec![rect],
                });
            }
        }
        Some(links)
    }

    /// Resolve a current, visible pointer point for one semantic link token.
    /// This recomputes from the retained packet and current scroll rather than
    /// accepting a snapshot rectangle, and never asks layout to rebuild.
    pub(crate) fn retained_accessible_pointer_target(
        &self,
        identity: SemanticInteractionId,
    ) -> Option<(f32, f32)> {
        if !self.presented {
            return None;
        }
        let layout = self.layout.as_ref()?;
        for region in layout.packet.interactions.iter().rev() {
            let Some(semantics) = &region.link_semantics else {
                continue;
            };
            if semantics.identity != identity {
                continue;
            }
            let Some([x, y, width, height]) = self.viewport_rect(region.bounds) else {
                continue;
            };
            let point = (x + width * 0.5, y + height * 0.5);
            let document_x = point.0 + self.scroll_x;
            let document_y = point.1 + self.scroll_y;
            let current_topmost = layout
                .packet
                .interactions
                .iter()
                .rev()
                .find(|candidate| rect_contains(candidate.bounds, document_x, document_y));
            if current_topmost
                .and_then(|candidate| candidate.link_semantics.as_ref())
                .is_some_and(|current| current.identity == identity)
            {
                return Some(point);
            }
        }
        None
    }

    /// Clip a retained full-document rect into this document's current
    /// viewport. The returned rectangle and all pointer targets derived from
    /// it remain inside the content hole.
    fn viewport_rect(&self, bounds: Rect) -> Option<[f32; 4]> {
        let left = (bounds.origin.x - self.scroll_x).max(0.0);
        let right = (bounds.max_x() - self.scroll_x).min(self.size.0 as f32);
        let top = (bounds.origin.y - self.scroll_y).max(0.0);
        let bottom = (bounds.max_y() - self.scroll_y).min(self.size.1 as f32);
        (left < right && top < bottom).then_some([left, top, right - left, bottom - top])
    }

    /// Resolve a viewport-local click through the retained full-document packet.
    pub fn click_at(&mut self, x: f32, y: f32, width: u32, height: u32) -> Option<InteractionKind> {
        self.ensure_layout(width, height);
        self.interaction_at(x, y).cloned()
    }

    /// The interaction under a viewport point in the retained layout, without
    /// laying out.
    pub(crate) fn interaction_at(&self, x: f32, y: f32) -> Option<&InteractionKind> {
        self.layout
            .as_ref()?
            .packet
            .interaction_at(x + self.scroll_x, y + self.scroll_y)
    }
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
    let palette = SmolwebPalette::for_theme(theme, url);
    let defaults = ColorVocabulary::default();
    let background = parse_color(&palette.bg).unwrap_or([1.0, 1.0, 1.0, 1.0]);
    let foreground = parse_color(&palette.fg).unwrap_or(defaults.body_text);
    let link = parse_color(&palette.link).unwrap_or(defaults.link_text);
    let quote = parse_color(&palette.quote).unwrap_or(defaults.badge_text);
    let pre = parse_color(&palette.pre_bg).unwrap_or(defaults.placeholder_image);
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
    style.colors = ColorVocabulary {
        body_text: foreground,
        heading_text: foreground,
        link_text: link,
        code_text: foreground,
        badge_text: quote,
        rule: quote,
        placeholder_text: foreground,
        placeholder_image: pre,
    };
    (style, background)
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
mod tests {
    use super::*;

    #[test]
    fn response_mime_precedes_carrier_and_plain_text_is_not_sniffed() {
        let body = "# Literal heading\n<rss><channel><title>Still literal</title></channel></rss>";
        let plain = SmolwebDocument::parse_input(
            &EngineInput::new("scroll://source.test/page", body)
                .with_content_type("Text/Plain; charset=utf-8"),
            SmolwebTheme::Plain,
        );
        assert!(plain.document().title.is_none());
        assert_eq!(
            plain.document().provenance.source_kind.as_deref(),
            Some("nematic.text")
        );
        assert!(
            plain
                .document()
                .blocks
                .iter()
                .all(|block| matches!(block, Block::Paragraph { .. }))
        );
        let markdown = SmolwebDocument::parse_input(
            &EngineInput::new("gopher://source.test/page", "# Markdown heading\n")
                .with_content_type("text/markdown;charset=utf-8"),
            SmolwebTheme::Plain,
        );
        assert_eq!(
            markdown.document().provenance.source_kind.as_deref(),
            Some("nematic.markdown")
        );
        assert_eq!(
            markdown.document().title.as_deref(),
            Some("Markdown heading")
        );
        let feed = SmolwebDocument::parse_input(
            &EngineInput::new("gemini://source.test/feed", "<?xml version=\"1.0\"?><rss version=\"2.0\"><channel><title>Feed</title><link>https://source.test/</link><description>Saved</description></channel></rss>")
                .with_content_type("application/xml"),
            SmolwebTheme::Plain,
        );
        assert_eq!(
            feed.document().provenance.source_kind.as_deref(),
            Some("nematic.feed")
        );
        assert_eq!(feed.document().title.as_deref(), Some("Feed"));
    }

    #[test]
    fn streamed_replacement_retains_response_mime_and_clears_stale_interpretation() {
        let url = "scroll://source.test/page";
        let input =
            EngineInput::new(url, "##### Prefix\n").with_content_type("text/scroll;lang=fr");
        let mut document = SmolwebDocument::parse_input(&input, SmolwebTheme::Plain);
        let mut appearance = document.new_appearance();
        appearance.replace_body(url, "##### Complete\n=> /record Cited [Citation]\n");
        assert_eq!(
            appearance.source_content_type.as_deref(),
            Some("text/scroll;lang=fr")
        );
        assert!(
            appearance
                .document()
                .blocks
                .iter()
                .any(|block| matches!(block, Block::Heading { level: 5, .. }))
        );
        assert_eq!(
            appearance.document().provenance.source_label.as_deref(),
            Some("scrolltext")
        );
        document.replace_input(
            &EngineInput::new(url, "# Literal now\n").with_content_type("text/plain"),
        );
        assert!(document.document().title.is_none());
        document.replace_body("gemini://source.test/new", "# New address\n");
        assert_eq!(document.document().title.as_deref(), Some("New address"));
        assert!(document.source_content_type.is_none());
        document.replace_input(&EngineInput::new(url, "# Explicit unknown MIME\n"));
        assert_eq!(
            document.document().title.as_deref(),
            Some("Explicit unknown MIME")
        );
        assert!(document.source_content_type.is_none());
    }

    fn test_png() -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(2, 1, image::Rgba([20, 40, 60, 255]));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .expect("encode PNG fixture");
        bytes
    }

    #[test]
    fn gemtext_uses_engine_document_and_paints_text() {
        let mut doc = SmolwebDocument::parse(
            "gemini://x.test/",
            "# Hello\n\nWorld.\n",
            SmolwebTheme::Site,
        );
        assert_eq!(
            doc.document().provenance.source_kind.as_deref(),
            Some(nematic::ENGINE_GEMTEXT)
        );
        let scene = doc.frame(800, 600);
        assert!(
            scene
                .ops
                .iter()
                .any(|op| matches!(op, netrender::SceneOp::GlyphRun(_)))
        );
        assert!(matches!(
            scene.ops.first(),
            Some(netrender::SceneOp::Rect(_))
        ));
    }

    #[test]
    fn opted_in_gemtext_image_link_fetches_decodes_paints_and_stays_clickable() {
        let mut doc = SmolwebDocument::parse_with_inline_media(
            "gemini://x.test/posts/index.gmi",
            "=> media/picture.png A picture\n",
            SmolwebTheme::Plain,
            SmolwebInlineMediaPolicy::images(),
        );
        assert!(matches!(
            &doc.document().blocks[0],
            Block::Image { url, alt }
                if url == "gemini://x.test/posts/media/picture.png" && alt == "A picture"
        ));
        assert_eq!(
            doc.subresources(),
            ["gemini://x.test/posts/media/picture.png"]
        );
        assert!(doc.provide_subresource("gemini://x.test/posts/media/picture.png", &test_png()));
        assert!(doc.subresources().is_empty());

        let scene = doc.frame(400, 300);
        assert!(
            scene
                .ops
                .iter()
                .any(|operation| matches!(operation, netrender::SceneOp::Image(_)))
        );
        let (url, [x, y, width, height]) =
            doc.links().into_iter().next().expect("image link region");
        assert_eq!(url, "gemini://x.test/posts/media/picture.png");
        assert!(matches!(
            doc.click_at(x + width / 2.0, y + height / 2.0, 400, 300),
            Some(InteractionKind::Link { url })
                if url == "gemini://x.test/posts/media/picture.png"
        ));
    }

    #[test]
    fn nested_image_resources_are_deduplicated_admitted_and_retained_by_replacement() {
        let mut doc = SmolwebDocument::parse_with_inline_media(
            "gemini://x.test/nested",
            "",
            SmolwebTheme::Plain,
            SmolwebInlineMediaPolicy::images(),
        );
        let image = |url: &str| Block::Image {
            url: url.into(),
            alt: String::new(),
        };
        let shared = "gemini://x.test/shared.png";
        let removed = "gemini://x.test/removed.png";
        let added = "gemini://x.test/added.png";
        let mut initial = (*doc.document).clone();
        initial.blocks = vec![
            Block::Quote {
                blocks: vec![image(shared)],
            },
            Block::List {
                ordered: false,
                items: vec![vec![image(shared), image(removed)]],
            },
            Block::Presented {
                presentation: Default::default(),
                block: Box::new(image(added)),
            },
        ];
        doc.replace_document(initial);

        assert_eq!(doc.subresources(), [shared, removed, added]);
        assert!(doc.provide_subresource(shared, &test_png()));
        assert!(doc.provide_subresource(removed, &test_png()));
        assert!(!doc.provide_subresource("gemini://x.test/unknown.png", &test_png()));

        let mut replacement = (*doc.document).clone();
        replacement.blocks = vec![Block::List {
            ordered: true,
            items: vec![vec![
                Block::Presented {
                    presentation: Default::default(),
                    block: Box::new(image(shared)),
                },
                image(shared),
                image(added),
            ]],
        }];
        doc.replace_document(replacement);

        assert!(doc.images.contains_key(shared));
        assert!(!doc.images.contains_key(removed));
        assert_eq!(doc.subresources(), [added]);
        assert!(doc.provide_subresource(added, &test_png()));
        assert!(doc.subresources().is_empty());
    }

    #[test]
    fn replacing_a_streamed_body_rebuilds_structure_and_invalidates_layout() {
        let mut doc = SmolwebDocument::parse_with_inline_media(
            "gemini://x.test/live",
            "# Prefix\n=> first.png First\n",
            SmolwebTheme::Plain,
            SmolwebInlineMediaPolicy::images(),
        );
        let _ = doc.frame(400, 300);
        assert!(doc.layout.is_some());

        doc.replace_body(
            "gemini://x.test/live",
            "# Complete\n=> second.png Second\nTail\n",
        );

        assert_eq!(doc.document().title.as_deref(), Some("Complete"));
        assert_eq!(doc.subresources(), ["gemini://x.test/second.png"]);
        assert!(doc.layout.is_none());
        assert!(
            doc.frame(400, 300)
                .ops
                .iter()
                .any(|operation| matches!(operation, netrender::SceneOp::GlyphRun(_)))
        );
    }

    #[test]
    fn streamed_replacement_scene_retains_prefix_runs_and_adds_tail_runs() {
        let url = "gemini://x.test/streaming.gmi";
        let prefix = "# Streaming prefix visible\nThis arrived before connection close.\n";
        let complete = concat!(
            "# Streaming prefix visible\n",
            "This arrived before connection close.\n",
            "## Streaming tail arrived\n",
            "The terminal body is complete.\n",
        );
        let mut doc = SmolwebDocument::parse(url, prefix, SmolwebTheme::Plain);
        let prefix_scene = doc.frame(614, 600);
        let prefix_runs: Vec<_> = prefix_scene
            .iter_glyph_runs()
            .map(|run| {
                (
                    run.font_size.to_bits(),
                    run.glyphs
                        .iter()
                        .map(|glyph| (glyph.id, glyph.x.to_bits(), glyph.y.to_bits()))
                        .collect::<Vec<_>>(),
                )
            })
            .collect();

        doc.replace_body(url, complete);
        let complete_scene = doc.frame(614, 600);
        let complete_runs: Vec<_> = complete_scene
            .iter_glyph_runs()
            .map(|run| {
                (
                    run.font_size.to_bits(),
                    run.glyphs
                        .iter()
                        .map(|glyph| (glyph.id, glyph.x.to_bits(), glyph.y.to_bits()))
                        .collect::<Vec<_>>(),
                )
            })
            .collect();

        assert!(complete_runs.len() > prefix_runs.len());
        assert!(
            prefix_runs
                .iter()
                .all(|prefix_run| complete_runs.contains(prefix_run)),
            "the full replacement scene must still carry every prefix glyph run"
        );
    }

    #[test]
    fn long_document_scrolls_and_windows() {
        let body: String = (0..200).map(|i| format!("Line {i}\n\n")).collect();
        let mut doc = SmolwebDocument::parse("gemini://x.test/", &body, SmolwebTheme::Plain);
        assert!(doc.content_height(400, 300) > 300);
        assert!(!doc.scroll_by(0.0, -50.0));
        assert!(doc.scroll_by(0.0, 240.0));
        assert_eq!(doc.frame(400, 300).viewport_height, 300);
    }

    #[test]
    fn links_and_clicks_share_viewport_coordinates() {
        let mut body: String = (0..4).map(|i| format!("Line {i}\n\n")).collect();
        body.push_str("=> gemini://x.test/page A link\n");
        body.extend((0..30).map(|i| format!("Tail {i}\n\n")));
        let mut doc = SmolwebDocument::parse("gemini://x.test/", &body, SmolwebTheme::Plain);
        assert!(doc.links().is_empty());
        let _ = doc.frame(400, 300);
        let (url, [_, initial_y, _, _]) = doc.links().into_iter().next().expect("link region");
        assert_eq!(url, "gemini://x.test/page");
        assert!(doc.scroll_by(0.0, 40.0));
        let (url, [x, y, width, height]) = doc
            .links()
            .into_iter()
            .next()
            .expect("scrolled link region");
        assert_eq!(url, "gemini://x.test/page");
        assert!(y < initial_y, "scrolling must move the host hit table");
        assert!(width > 0.0 && height > 0.0);
        assert!(matches!(
            doc.click_at(x + width / 2.0, y + height / 2.0, 400, 300),
            Some(InteractionKind::Link { url }) if url == "gemini://x.test/page"
        ));
        doc.scroll_to(f32::MAX);
        assert!(
            doc.links().is_empty(),
            "viewport-space links omit regions scrolled fully out of view"
        );
    }

    #[test]
    fn wide_table_links_clip_scroll_hit_test_and_resize_in_viewport_coordinates() {
        let document = EngineDocument {
            address: "gemini://x.test/wide".into(),
            title: None,
            content_type: "text/gemini".into(),
            lang: None,
            provenance: inker::DocumentProvenance::default(),
            trust: inker::DocumentTrustState::Unknown,
            diagnostics: Vec::new(),
            navigation: Default::default(),
            blocks: vec![Block::Table {
                alignments: Vec::new(),
                header: Vec::new(),
                rows: vec![vec![
                    vec![InlineSpan::Text(
                        "first-column-is-intentionally-unbreakable".into(),
                    )],
                    vec![InlineSpan::Link {
                        url: "gemini://x.test/wide/target".into(),
                        title: None,
                        spans: vec![InlineSpan::Text("target".into())],
                        predicate: None,
                    }],
                ]],
            }],
        };
        let mut doc = SmolwebDocument::from_document(
            document,
            DocumentStyleSheet::default(),
            [1.0, 1.0, 1.0, 1.0],
        );
        let _ = doc.frame(120, 100);
        assert!(
            doc.max_scroll_x() > 0.0,
            "the wide table must expose a horizontal range"
        );
        assert!(
            doc.links().is_empty(),
            "offscreen link rectangles are not exposed as viewport targets"
        );

        assert!(doc.scroll_by(f32::MAX, 0.0));
        let (url, [x, y, width, height]) = doc.links().into_iter().next().expect("scrolled target");
        assert_eq!(url, "gemini://x.test/wide/target");
        assert!(x >= 0.0 && x + width <= 120.0);
        assert!(y >= 0.0 && y + height <= 100.0);
        assert!(matches!(
            doc.click_at(x + width * 0.5, y + height * 0.5, 120, 100),
            Some(InteractionKind::Link { url }) if url == "gemini://x.test/wide/target"
        ));

        let _ = doc.frame(240, 100);
        assert!(doc.scroll_x() <= doc.max_scroll_x());
        for (_, [x, y, width, height]) in doc.links() {
            assert!(x >= 0.0 && x + width <= 240.0);
            assert!(y >= 0.0 && y + height <= 100.0);
        }
    }

    #[test]
    fn spartan_prompt_click_remains_a_submission() {
        let mut doc = SmolwebDocument::parse(
            "spartan://x.test/guestbook",
            "=: /guestbook/sign Sign it\n",
            SmolwebTheme::Plain,
        );
        let _ = doc.frame(400, 300);
        let region = doc
            .layout
            .as_ref()
            .expect("layout")
            .packet
            .interactions
            .iter()
            .find(|region| matches!(region.kind, InteractionKind::Submit { .. }))
            .expect("prompt region");
        let x0 = region.bounds.origin.x;
        let y0 = region.bounds.origin.y;
        let x1 = x0 + region.bounds.size.width;
        let y1 = y0 + region.bounds.size.height;
        assert!(matches!(
            doc.click_at((x0 + x1) / 2.0, (y0 + y1) / 2.0, 400, 300),
            Some(InteractionKind::Submit { target }) if target == "/guestbook/sign"
        ));
    }

    #[test]
    fn schemes_select_their_nematic_engines() {
        let gopher = SmolwebDocument::parse(
            "gopher://x.test/",
            "1Files\t/files\tx.test\t70\r\n",
            SmolwebTheme::Plain,
        );
        assert_eq!(
            gopher.document().provenance.source_kind.as_deref(),
            Some(nematic::ENGINE_GOPHER)
        );

        let feed = SmolwebDocument::parse(
            "gemini://x.test/feed",
            "<?xml version=\"1.0\"?><rss version=\"2.0\"><channel><title>Log</title></channel></rss>",
            SmolwebTheme::Dark,
        );
        assert_eq!(
            feed.document().provenance.source_kind.as_deref(),
            Some(nematic::ENGINE_FEED)
        );
    }

    /// A gopher menu reaches the host as one typed menu (smolweb fidelity plan
    /// WS4, R2): its rows keep their kinds, a directory row is a viewport link
    /// target, and the search row submits rather than navigates.
    #[test]
    fn a_gopher_menu_reaches_the_host_as_one_typed_menu() {
        let body = concat!(
            "iWelcome\tfake\t(NULL)\t0\r\n",
            "1Phlog\t/phlog\tx.test\t70\r\n",
            "7Search\t/find\tx.test\t70\r\n",
        );
        let mut doc = SmolwebDocument::parse("gopher://x.test/", body, SmolwebTheme::Plain);
        let [Block::Menu { rows }] = doc.document().blocks.as_slice() else {
            panic!("one menu: {:?}", doc.document().blocks);
        };
        let kinds: Vec<_> = rows.iter().map(|row| row.kind).collect();
        assert_eq!(
            kinds,
            [
                inker::MenuItemKind::Info,
                inker::MenuItemKind::Directory,
                inker::MenuItemKind::Search,
            ]
        );
        let _ = doc.frame(640, 480);
        let links: Vec<_> = doc.links().into_iter().map(|(url, _)| url).collect();
        assert_eq!(
            links,
            ["gopher://x.test/1/phlog"],
            "the search row is not a link"
        );
    }

    /// An entry's own address fetches the feed without its fragment and renders
    /// the entry alone (smolweb fidelity plan WS4, R4).
    #[test]
    fn an_entry_address_loads_the_feed_and_renders_the_entry() {
        const FEED: &str = r#"<?xml version="1.0"?>
<rss version="2.0" xmlns:content="http://purl.org/rss/1.0/modules/content/">
  <channel><title>Log</title>
    <item>
      <title>Post</title>
      <link>https://x.test/post</link>
      <guid>post-1</guid>
      <description>Teaser.</description>
      <content:encoded><![CDATA[<p>The whole post.</p>]]></content:encoded>
    </item>
  </channel>
</rss>"#;
        struct Recording(std::sync::Mutex<Vec<String>>);
        impl ResourceFetcher for Recording {
            fn fetch_request(
                &self,
                _request: &genet_host_api::ResourceRequest,
            ) -> Option<genet_host_api::ResourceFetchResponse> {
                None
            }
            fn fetch(&self, url: &str) -> Option<Vec<u8>> {
                self.0.lock().unwrap().push(url.to_string());
                Some(FEED.as_bytes().to_vec())
            }
        }

        let feed = SmolwebDocument::parse("gemini://x.test/feed.xml", FEED, SmolwebTheme::Plain);
        let address = feed
            .document()
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::FeedEntry {
                    content_address: Some(address),
                    ..
                } => Some(address.clone()),
                _ => None,
            })
            .expect("the entry carries its own address");

        let fetcher = Recording(Default::default());
        let article = SmolwebDocument::load(&fetcher, &address, SmolwebTheme::Plain).unwrap();
        assert_eq!(
            *fetcher.0.lock().unwrap(),
            ["gemini://x.test/feed.xml"],
            "the fragment is not sent"
        );
        let document = article.document();
        assert_eq!(document.title.as_deref(), Some("Post"));
        assert_eq!(
            document.provenance.canonical_uri.as_deref(),
            Some("https://x.test/post")
        );
        assert!(inline_text_of(document).contains("The whole post."));
    }

    fn inline_text_of(document: &EngineDocument) -> String {
        document
            .walk_inline_spans()
            .into_iter()
            .map(|span| inline_text(std::slice::from_ref(span)))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// A fixed-width menu format keeps one typeface across its column grid
    /// (see `FIXED_WIDTH_MENU_TYPES`).
    #[test]
    fn a_fixed_width_menu_sets_its_body_font_to_the_monospace_one() {
        for content_type in ["application/gopher-menu", "application/x-nex-listing"] {
            let (style, _) = style_for_theme(&SmolwebTheme::Dark, "gopher://x.test/", content_type);
            assert_eq!(
                style.body_font_family, style.mono_font_family,
                "{content_type} must not mix typefaces inside one column grid"
            );
        }
        // A prose format keeps the body serif: gemtext is not column-aligned.
        for content_type in ["text/gemini", "text/plain"] {
            let (style, _) = style_for_theme(&SmolwebTheme::Dark, "gemini://x.test/", content_type);
            assert_eq!(style.body_font_family, "serif, sans-serif, monospace", "{content_type} keeps prose and glyph fallback roles");
        }
    }

    #[test]
    fn app_rgb_palette_maps_into_document_style() {
        let theme = SmolwebTheme::App(SmolwebPalette {
            bg: "rgb(16, 32, 48)".into(),
            fg: "rgb(250, 250, 250)".into(),
            link: "rgb(51, 204, 255)".into(),
            quote: "rgb(153, 170, 187)".into(),
            pre_bg: "rgb(10, 22, 34)".into(),
        });
        let (style, background) = style_for_theme(&theme, "gemini://x.test/", "text/gemini");
        assert_eq!(background, [16.0 / 255.0, 32.0 / 255.0, 48.0 / 255.0, 1.0]);
        assert_eq!(
            style.colors.link_text,
            [51.0 / 255.0, 204.0 / 255.0, 1.0, 1.0]
        );
    }
}

#[cfg(test)]
mod typography_tests {
    use super::*;
    #[test]
    fn prose_measure_reflows_to_the_appearance_without_changing_its_source() {
        let source = Arc::new(EngineDocument {
            address: "mere://reading/test".into(),
            title: Some("Reading".into()),
            content_type: "text/plain".into(),
            lang: None,
            provenance: Default::default(),
            trust: Default::default(),
            diagnostics: Vec::new(),
            navigation: Default::default(),
            blocks: vec![Block::Paragraph {
                spans: vec![inker::InlineSpan::Text("A measured line keeps the next sentence in reach. ".repeat(80))],
            }],
        });
        let mut doc = SmolwebDocument::from_shared_document_with_theme(source.clone(), SmolwebTheme::Plain);
        let code = doc.style_sheet().resolve(document_canvas::BlockRole::Code);
        doc.set_font_family("serif");
        assert_eq!(doc.style_sheet().resolve(document_canvas::BlockRole::Code), code,
            "a prose face cannot replace the code role");
        doc.frame(1200, 600);
        let wide = doc.layout.as_ref().unwrap().packet.clone();
        let paragraph = &wide.blocks[0].bounds;
        assert!(paragraph.origin.x > 200.0, "wide reading is centered with a bounded measure");
        assert!(paragraph.size.width < 750.0);
        doc.frame(320, 600);
        let narrow = &doc.layout.as_ref().unwrap().packet;
        assert!(narrow.content_bounds.size.height > wide.content_bounds.size.height, "narrow reading must reflow");
        for block in &narrow.blocks {
            assert!(block.bounds.origin.x >= 0.0);
            assert!(block.bounds.origin.x + block.bounds.size.width <= 320.1, "prose does not require horizontal scrolling");
        }
        assert!(Arc::ptr_eq(&source, &doc.document));
    }

    #[test]
    fn restyling_reflows_without_replacing_source_or_session_state() {
        let source = Arc::new(EngineDocument {
            address: "gemini://x.test/".into(),
            title: Some("Held content".into()),
            content_type: "text/gemini".into(),
            lang: None,
            provenance: Default::default(),
            trust: Default::default(),
            diagnostics: Vec::new(),
            navigation: Default::default(),
            blocks: vec![Block::Paragraph {
                spans: vec![
                    inker::InlineSpan::Text("A retained paragraph. ".repeat(120)),
                    inker::InlineSpan::Link {
                        url: "gemini://x.test/next".into(),
                        title: None,
                        spans: vec![inker::InlineSpan::Text("Next".into())],
                        predicate: None,
                    },
                ],
            }],
        });
        let mut doc = SmolwebDocument::from_shared_document_with_theme(source, SmolwebTheme::Plain);
        doc.frame(300, 200);
        doc.focus_move(SessionFocusDirection::Forward, 300, 200);
        doc.scroll_by(0.0, 40.0);
        let focus = doc.focus.clone();
        let source = doc.document.clone();
        let scroll = doc.scroll_y;
        let folds = doc.folds.clone();
        let old_height = doc
            .layout
            .as_ref()
            .unwrap()
            .packet
            .content_bounds
            .size
            .height;
        let mut style = doc.style_sheet().clone();
        style.roles.body.size = document_canvas::SizeSpec::Absolute(24.0);
        style.roles.body.line_height_ratio = Some(1.8);
        assert!(doc.set_style_sheet(style.clone()));
        assert!(doc.layout.is_none());
        assert!(Arc::ptr_eq(&source, &doc.document));
        assert_eq!(doc.scroll_y, scroll);
        assert_eq!(doc.folds, folds);
        assert_eq!(doc.focus, focus);
        doc.frame(300, 200);
        assert!(
            doc.layout
                .as_ref()
                .unwrap()
                .packet
                .content_bounds
                .size
                .height
                > old_height
        );
        let body_size = |doc: &SmolwebDocument| doc.layout.as_ref().unwrap().packet.blocks.iter().find_map(|block| match &block.kind {
            document_canvas::RenderedBlockKind::Text{glyph_runs}=>glyph_runs.first().map(|run|run.font_size), _=>None,
        }).unwrap();
        assert_eq!(body_size(&doc),24.0,"hosts which did not opt into reading controls retain explicit styles");
        let mut other=doc.new_appearance();
        let mut preferences=tabard::ReadingPreferences::default(); preferences.step_text(true);
        doc.set_reading_preferences(preferences).unwrap();
        doc.frame(300,200); other.frame(300,200);
        assert_eq!(body_size(&doc),20.0);
        assert_eq!(body_size(&other),24.0,"a fresh projection retains the host baseline until it selects reading intent");
        assert!(!doc.set_style_sheet(style));
    }
}

#[cfg(test)]
mod reading_anchor_tests {
    use super::*;
    #[test]
    fn passage_survives_type_width_and_cold_appearance() {
        let body=(0..60).map(|i|format!("Paragraph {i}: a distinct passage about graph projections and the relationship between the map and reading.\n\n")).collect::<String>();
        let mut doc=SmolwebDocument::from_document_with_theme(EngineDocument {
            address: "gemini://example.test/anchors".into(), title: None, content_type: "text/plain".into(), lang: None,
            provenance: Default::default(), trust: Default::default(), diagnostics: Vec::new(), navigation: Default::default(),
            blocks: body.split("\n\n").filter(|s|!s.is_empty()).map(|text| Block::Paragraph {spans:vec![inker::InlineSpan::Text(text.into())]}).collect(),
        },SmolwebTheme::default());
        let _=doc.frame(700,300); doc.scroll_to(1500.);
        let anchor=doc.reading_anchor().expect("a laid-out passage");
        doc.set_reading_preferences(tabard::ReadingPreferences{text_size:24,column_em:28}).unwrap();
        let _=doc.frame(440,300);
        let resolved=crate::reading_anchor::resolve(&doc.document.address,&doc.layout.as_ref().unwrap().searchable,&anchor,1.).unwrap();
        assert!((doc.scroll_y()-resolved).abs()<1.0);
        let after=doc.reading_anchor().unwrap();
        let _=doc.frame(880,600);
        let resolved=crate::reading_anchor::resolve(&doc.document.address,&doc.layout.as_ref().unwrap().searchable,&after,1.).unwrap();
        assert!((doc.scroll_y()-resolved).abs()<1.0);
        let saved=doc.reading_anchor().unwrap();
        let mut cold=doc.new_appearance();
        cold.set_reading_preferences(doc.reading_preferences()).unwrap();
        cold.restore_reading_anchor(saved);let _=cold.frame(880,600);
        assert!((cold.scroll_y()-doc.scroll_y()).abs()<1.0);
        assert!(std::sync::Arc::ptr_eq(&doc.source_document(),&cold.source_document()));
    }
}
