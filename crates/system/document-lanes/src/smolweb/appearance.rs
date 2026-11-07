/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

impl SmolwebDocument {
    /// The portable document retained by this session.
    /// The retained appearance's style. Source, fold/focus state and viewport
    /// are separate from this presentation policy.
    pub fn style_sheet(&self) -> &DocumentStyleSheet {
        &self.style
    }

    /// The host mode this appearance's theme colors were derived under.
    pub fn color_mode(&self) -> ModeProfile {
        self.color_mode
    }

    /// Re-derive a themed appearance's colors under the host's light, dark or
    /// high-contrast mode. Fonts and reading preferences stay as they are; an
    /// appearance built from an explicit style sheet keeps its colors.
    pub fn set_color_mode(&mut self, mode: ModeProfile) -> bool {
        if self.color_mode == mode {
            return false;
        }
        self.color_mode = mode;
        let Some(theme) = &self.theme else {
            return false;
        };
        let (colors, background) = theme_colors(theme, &self.document.address, mode);
        if self.style.colors == colors && self.background == background {
            return false;
        }
        self.style.colors = colors;
        self.background = background;
        self.invalidate_layout();
        true
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

    pub fn source_document(&self) -> Arc<EngineDocument> {
        self.document.clone()
    }

    /// Fresh viewport/search/reading state over the same source and held fonts.
    pub fn new_appearance(&self) -> Self {
        let mut appearance = Self::from_shared_document(
            self.document.clone(),
            self.style.clone(),
            self.background,
            self.inline_media,
        );
        appearance.source_content_type = self.source_content_type.clone();
        appearance.theme = self.theme.clone();
        appearance.color_mode = self.color_mode;
        appearance.fonts = self.fonts.clone();
        appearance.generic_fonts = self.generic_fonts.clone();
        appearance.dictionaries = self.dictionaries.clone();
        appearance.images = self.images.clone();
        appearance
    }

    pub fn reading_anchor(&self) -> Option<crate::ReadingAnchor> {
        let layout = self.layout.as_ref()?;
        crate::reading_anchor::capture(
            &self.document.address,
            &layout.searchable,
            self.scroll_y,
            1.0,
        )
    }

    /// Applied once after layout. A stale quote never jumps to unrelated text.
    pub fn restore_reading_anchor(&mut self, anchor: crate::ReadingAnchor) {
        self.pending_reading = Some(anchor);
        self.invalidate_layout();
    }

    pub(super) fn retain_reading_anchor(&mut self) {
        if self.pending_reading.is_none() {
            self.pending_reading = self.reading_anchor();
        }
    }

    pub fn reading_preferences(&self) -> crate::ReadingPreferences {
        self.reading.unwrap_or_default()
    }

    pub fn set_reading_preferences(
        &mut self,
        value: crate::ReadingPreferences,
    ) -> Result<bool, String> {
        value.validate()?;
        if Some(value) == self.reading {
            return Ok(false);
        }
        self.reading = Some(value);
        self.invalidate_layout();
        Ok(true)
    }

    pub fn document_find(&mut self, query: &inker::DocumentFindQuery) -> inker::DocumentFindState {
        self.ensure_layout(self.size.0.max(1), self.size.1.max(1));
        self.find
            .replace(query.clone(), &self.layout.as_ref().unwrap().searchable);
        self.reveal_find();
        self.find.state()
    }

    pub fn document_find_step(
        &mut self,
        direction: inker::DocumentFindDirection,
    ) -> inker::DocumentFindState {
        self.ensure_layout(self.size.0.max(1), self.size.1.max(1));
        self.find.step(direction);
        self.reveal_find();
        self.find.state()
    }

    pub fn clear_document_find(&mut self) {
        self.find = Default::default();
    }

    pub(super) fn reveal_find(&mut self) {
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
        self.pending_reading = if document.address == self.document.address {
            self.reading_anchor()
        } else {
            None
        };
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
        self.layout = None;
        self.invalidate_layout();
    }

    pub(super) fn invalidate_layout(&mut self) {
        self.retain_reading_anchor();
        self.layout = None;
        self.presented = false;
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
}
