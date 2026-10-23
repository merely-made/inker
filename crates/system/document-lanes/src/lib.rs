/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Mere's document content lanes above Genet.
//!
//! The reader lane (held HTML through fleece into the portable document
//! canvas), the smolweb lane (protocol content through Nematic into the same
//! canvas, feature `smolweb`), their session engines, and the remote half of a
//! host's resource fetcher: http(s) over netfetcher (feature `netfetch`) and
//! the smolweb schemes over errand. None of it is web-observable engine
//! behaviour; all of it is application session and routing, which is why it
//! left `genet-documents` under the platform boundary plan (mere
//! `design_docs/mere_docs/implementation_strategy/`
//! `2026-09-02_platform_boundary_and_repository_topology_plan.md`, P1). It
//! now lives in Mere. Shared capture evidence and structured-data projection
//! live in Fleece's optional modules; this crate owns document presentation.

pub mod reader;
mod remote;
mod session;
pub mod smolweb;
/// Segmenter dictionaries a host loads for Thai, Lao, Khmer, Burmese, Chinese
/// and Japanese breaking.
pub use document_canvas::{DictionaryBlob, DictionaryBlobError, SegmenterDictionaries};
pub use genet_host_api::{ResourceFetchPolicy, ResourceFetcher};
pub use reader::{
    ReaderAccessibilityLink, ReaderAccessibilitySnapshot, ReaderDocumentSession,
    ReaderSessionEngine, lower_article,
};
pub use remote::RemoteFetcher;
pub use session::SmolwebDocumentSession;
#[cfg(feature = "smolweb")]
pub use session::SmolwebSessionEngine;
pub use smolweb::{
    InPageNavigation, SmolwebDocument, SmolwebInlineMediaPolicy, SmolwebPalette, SmolwebTheme,
};
#[cfg(feature = "retained-clips")]
pub mod retained_clip;

pub mod typography;
mod reading_find;
pub use typography::document_style_with_typography;

mod reading_anchor;

/// Bounded prose from an already retained packet. No fetch or passage inference.
pub fn reading_excerpt(document:&inker::EngineDocument)->Option<String> {
    fn prose(block:&inker::Block)->Option<String> {
        use inker::Block;
        match block {
            Block::Paragraph{spans}=>Some(inker::inline_text(spans)),
            Block::Presented{block,..}=>prose(block),
            Block::Quote{blocks}=>blocks.iter().find_map(prose),
            Block::List{items,..}=>items.iter().flatten().find_map(prose),
            _=>None,
        }
    }
    document.blocks.iter().filter_map(prose).find(|text|!text.trim().is_empty()).map(|text|text.chars().take(280).collect())
}
