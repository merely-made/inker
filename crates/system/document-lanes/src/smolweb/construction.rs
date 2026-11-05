/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

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
        retained.theme = Some(theme);
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
        let mut retained = Self::from_document(document, style, background);
        retained.theme = Some(theme);
        retained
    }

    /// Retain a shared portable packet while keeping this appearance's layout,
    /// viewport, scroll and inline-media cache independent.
    pub fn from_shared_document_with_theme(
        document: Arc<EngineDocument>,
        theme: SmolwebTheme,
    ) -> Self {
        let (style, background) =
            style_for_theme(&theme, &document.address, &document.content_type);
        let mut retained = Self::from_shared_document(
            document,
            style,
            background,
            SmolwebInlineMediaPolicy::default(),
        );
        retained.theme = Some(theme);
        retained
    }

    pub(super) fn from_document_with_media_policy(
        document: EngineDocument,
        style: DocumentStyleSheet,
        background: [f32; 4],
        inline_media: SmolwebInlineMediaPolicy,
    ) -> Self {
        Self::from_shared_document(Arc::new(document), style, background, inline_media)
    }

    pub(super) fn from_shared_document(
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
            theme: None,
            color_mode: ModeProfile::LIGHT,
        }
    }
}
