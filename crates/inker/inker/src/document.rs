// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Portable document model — what engines produce.
//!
//! This module defines the universal semantic shape every engine renders
//! into. The model carries:
//!
//! - **Structure**: `Block` / `InlineSpan` (headings, paragraphs,
//!   lists, links, etc.) — what content there is.
//! - **Semantic intent**: `FeedHeader`, `FeedEntry`, `MetadataRow`, `Badge`
//!   — when a source format has block-level meaning beyond "this is a
//!   paragraph", the engine can name it. Intelligence layers downstream
//!   (search, summarise, recommend, recall) match on these intents, not
//!   just text.
//! - **Provenance**: where the content came from (canonical URI, source
//!   engine, fetch time, source label).
//! - **Trust state**: how authenticated the source is (Trusted / Tofu /
//!   Insecure / Broken / Unknown).
//! - **Diagnostics**: parser / render warnings the engine wants to surface
//!   (unsupported construct, degraded rendering, parse warnings).
//!
//! ## Spec faithfulness
//!
//! Protocol engines (gemini, gopher, RSS/Atom, finger) populate semantic
//! variants only when the source spec actually says them — RSS `<item>`
//! becomes `FeedEntry`, finger `Login: alice` becomes `MetadataRow`. The
//! engines do not invent semantics the spec doesn't say. File-format
//! engines (markdown, the knot note format) can be richer because the
//! format owns its own semantics.
//!
//! ## Round-trip
//!
//! [`EngineDocument::to_markdown`] and [`EngineDocument::to_gemini`] live
//! in the `render` submodule and let clips / notes export back into native
//! CommonMark / gemtext.

use serde::{Deserialize, Serialize};

mod block_provenance;
mod evaluate;
mod navigation;
mod render;
mod transclude;
pub use evaluate::{
    BlockEvaluator, BlockEvaluators, EvalOutcome, EvalOutput, EvaluationPolicy, evaluate_blocks,
    parse_eval,
};
pub use render::GophermapContext;
pub use transclude::{
    Fetched, TranscludeOutcome, TransclusionPolicy, parse_include, resolve_transclusions,
};

pub use block_provenance::{BlockProvenance, BlockProvenanceMap, ResolvedProvenance};
pub use navigation::{
    DocumentAnchor, DocumentFold, DocumentNavigation, FoldKey, FoldMarkers, FoldState, InPageTarget,
};

/// A rendered document.
///
/// Portable, serializable, host-neutral. No layout coordinates — those come
/// later in `platen` once a real surface and font system are bound.
///
/// **A11y note:** the document maps to an AccessKit `Role::Document` node.
/// `title` is the document's accessible name; `lang` is its declared
/// language (BCP 47). Engines that don't see a language declaration leave
/// `lang` as `None` and the host fills it from environment defaults.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineDocument {
    /// The address the engine was asked to render. May differ from
    /// `provenance.canonical_uri` when the host followed a redirect.
    pub address: String,
    pub title: Option<String>,
    pub content_type: String,
    /// BCP 47 language tag (e.g. `"en"`, `"en-GB"`, `"ja"`). `None` when the
    /// engine had no signal — host supplies a default during projection.
    #[serde(default)]
    pub lang: Option<String>,
    /// Where the document came from. Defaulted to `Default` on engines that
    /// haven't been updated yet; new engines should populate at least
    /// `source_kind` and `canonical_uri`.
    #[serde(default)]
    pub provenance: DocumentProvenance,
    /// How authenticated the source is. Default is `Unknown`; the host or
    /// the engine fills it in once enough information is available.
    #[serde(default)]
    pub trust: DocumentTrustState,
    /// Parser / render warnings worth surfacing to the user or to a debug
    /// overlay. Defaults to empty.
    #[serde(default)]
    pub diagnostics: Vec<DocumentDiagnostic>,
    /// In-page anchors and collapsible extents over `blocks`, when the source
    /// format has them. Empty for engines without in-page navigation.
    #[serde(default)]
    pub navigation: DocumentNavigation,
    pub blocks: Vec<Block>,
}

impl EngineDocument {
    /// Walk every block (and nested blocks inside quotes / list items),
    /// yielding inline spans in document order. Useful for link extraction
    /// and plain-text summarisation.
    pub fn walk_inline_spans(&self) -> Vec<&InlineSpan> {
        let mut out = Vec::new();
        for block in &self.blocks {
            collect_block_spans(block, &mut out);
        }
        out
    }

    /// Extract every link target referenced by the document, in document
    /// order. Walks both inline `Link` spans inside structural blocks and
    /// the URL fields of semantic blocks (`FeedHeader.source_url`,
    /// `FeedEntry.article_url` / `source_url`).
    ///
    /// Links only: a link is somewhere the reader can navigate. Image
    /// sources are subresources of *this* document, not destinations, and
    /// are collected separately — see `collect_block_link_urls`.
    pub fn outgoing_links(&self) -> Vec<&str> {
        let mut links = Vec::new();
        for block in &self.blocks {
            collect_block_link_urls(block, &mut links);
        }
        links
    }
}

/// Where the document came from.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentProvenance {
    /// Stable engine ID that produced this document (e.g.
    /// `"nematic.markdown"`). Lets downstream consumers know which parser
    /// shape produced the output.
    #[serde(default)]
    pub source_kind: Option<String>,
    /// Canonical address of the document. Often equal to
    /// [`EngineDocument::address`] but can differ when a fetcher followed
    /// redirects or canonicalised a URL.
    #[serde(default)]
    pub canonical_uri: Option<String>,
    /// RFC 3339 timestamp; populated by the host's fetch layer.
    #[serde(default)]
    pub fetched_at: Option<String>,
    /// Human-readable label for the source ("Wikipedia", "alice's blog").
    #[serde(default)]
    pub source_label: Option<String>,
}

impl DocumentProvenance {
    /// Convenience constructor for engines: records the engine's own ID and
    /// the address it was asked to render. Hosts add `fetched_at` and
    /// `source_label` later.
    pub fn for_engine(engine_id: &str, address: &str) -> Self {
        Self {
            source_kind: Some(engine_id.to_string()),
            canonical_uri: Some(address.to_string()),
            fetched_at: None,
            source_label: None,
        }
    }
}

/// How authenticated the document's source is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentTrustState {
    /// Source verified through a chain of trust (TLS root, signature,
    /// signed envelope, etc.).
    Trusted,
    /// First contact with this peer was accepted on trust ("trust on first
    /// use"). Re-fetches must verify the same key.
    Tofu,
    /// Loaded over an unauthenticated transport (plain HTTP, file://).
    Insecure,
    /// Verification was attempted and failed (cert mismatch, signature
    /// invalid, key changed). Content rendered with a warning.
    Broken,
    /// Trust state has not been evaluated.
    #[default]
    Unknown,
}

/// A note the engine attaches to the document about parse / render quality.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentDiagnostic {
    /// A spec construct exists in the source but isn't supported by this
    /// engine; some content was lost.
    UnsupportedConstruct(String),
    /// Content rendered with reduced fidelity (HTML tags stripped, links
    /// flattened, etc.).
    DegradedRendering(String),
    /// Recoverable parser warning.
    ParseWarning(String),
    /// Engine fell back to raw-source presentation; consumers may want to
    /// expose the unparsed bytes.
    RawSourceFallback,
}

/// A block-level region of an [`EngineDocument`].
///
/// Each variant maps to an AccessKit role; the projection layer lifts these
/// into an a11y / automation tree without any host-specific information.
///
/// Non-exhaustive: crates outside inker match it with a wildcard arm that
/// surfaces a kind they cannot draw (name it with [`Block::kind_name`])
/// rather than dropping it silently.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Block {
    /// A source-format presentation hint around one structural block. The
    /// wrapped block remains the semantic content; readers may choose a
    /// contrast-preserving override of these visual facts.
    Presented {
        presentation: BlockPresentation,
        block: Box<Block>,
    },
    /// AccessKit `Role::Heading` with the heading level set on the node.
    Heading { level: u8, spans: Vec<InlineSpan> },
    /// AccessKit `Role::Paragraph`.
    Paragraph { spans: Vec<InlineSpan> },
    /// AccessKit `Role::CodeSample`.
    CodeBlock {
        language: Option<String>,
        text: String,
    },
    /// AccessKit `Role::Blockquote`.
    Quote { blocks: Vec<Block> },
    /// AccessKit `Role::List`.
    List {
        ordered: bool,
        items: Vec<Vec<Block>>,
    },
    /// AccessKit `Role::Image`.
    Image { url: String, alt: String },
    /// AccessKit `Role::Pre`.
    Preformatted { text: String },
    /// AccessKit `Role::Separator`.
    Rule,
    /// Feed-level header (RSS `<channel>`, Atom `<feed>` top-level
    /// metadata). Distinguishes "this is the feed itself" from "this is an
    /// entry in the feed" so projection layers can render them
    /// differently.
    FeedHeader {
        title: String,
        subtitle: Option<String>,
        summary: Option<String>,
        source_url: Option<String>,
    },
    /// One entry in a syndication feed (RSS `<item>` / Atom `<entry>`).
    FeedEntry {
        title: String,
        /// One date to show: the first the entry carries.
        date: Option<String>,
        summary: Option<String>,
        article_url: Option<String>,
        source_url: Option<String>,
        /// First published (RSS `pubDate`, Atom `published`).
        #[serde(default)]
        published: Option<String>,
        /// Last changed (Atom `updated`).
        #[serde(default)]
        updated: Option<String>,
        /// Stable entry identity (RSS `guid`, Atom `id`).
        #[serde(default)]
        guid: Option<String>,
        /// Attached media (podcast audio, video).
        #[serde(default)]
        enclosures: Vec<FeedEnclosure>,
        /// The entry's own document when the feed carries its body: the feed
        /// address with the guid as its fragment. Opening it renders the body
        /// offline, as its own document.
        #[serde(default)]
        content_address: Option<String>,
    },
    /// Label / value pair (`Login: alice`, `Language: en-US`,
    /// `Last-Modified: …`). Projection renders as a definition-list row.
    MetadataRow { label: String, value: String },
    /// Short status / annotation marker (trust state notice, "raw source"
    /// affordance). Visual hint for projection; not free-flowing text.
    Badge { text: String },
    /// A table: an optional header row plus body rows, each a list of cells, each
    /// cell inline spans, with per-column alignment. Flat — no rowspan / colspan
    /// (the carve `^` / `<` span syntax is a later rung). AccessKit `Role::Table`.
    Table {
        /// Per-column alignment; `alignments[i]` applies to column `i`. May be
        /// shorter than the widest row; missing columns default to
        /// [`TableAlignment::None`].
        alignments: Vec<TableAlignment>,
        /// Header cells, one per column, or empty when the table has no header row.
        header: Vec<Vec<InlineSpan>>,
        /// Body rows; each row is a list of cells, each cell a list of inline spans.
        rows: Vec<Vec<Vec<InlineSpan>>>,
    },
    /// A typed menu (a gopher menu, a nex directory): rows that each name
    /// their kind, keep the source's raw item-type marker, and carry a label
    /// and an optional target. Laid out as a fixed-width grid, type column
    /// then label column. AccessKit `Role::List` of items naming their kind.
    Menu { rows: Vec<MenuRow> },
}

impl Block {
    /// A short stable name for the block's kind, for diagnostics and for the
    /// placeholder a renderer shows when it cannot draw a kind.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Block::Presented { block, .. } => block.kind_name(),
            Block::Heading { .. } => "heading",
            Block::Paragraph { .. } => "paragraph",
            Block::CodeBlock { .. } => "code block",
            Block::Quote { .. } => "quote",
            Block::List { .. } => "list",
            Block::Image { .. } => "image",
            Block::Preformatted { .. } => "preformatted",
            Block::Rule => "rule",
            Block::FeedHeader { .. } => "feed header",
            Block::FeedEntry { .. } => "feed entry",
            Block::MetadataRow { .. } => "metadata row",
            Block::Badge { .. } => "badge",
            Block::Table { .. } => "table",
            Block::Menu { .. } => "menu",
        }
    }
}

/// A menu rewritten as the blocks formats without a menu read: runs of info
/// and error rows as one preformatted block, a targeted row as a link
/// paragraph, and any other row (a search) as its label.
pub fn menu_fallback_blocks(rows: &[MenuRow]) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut run: Vec<String> = Vec::new();
    let flush = |run: &mut Vec<String>, blocks: &mut Vec<Block>| {
        if !run.is_empty() {
            blocks.push(Block::Preformatted {
                text: run.drain(..).collect::<Vec<_>>().join("\n"),
            });
        }
    };
    for row in rows {
        match (row.kind, &row.target) {
            (MenuItemKind::Info, None) => run.push(inline_text(&row.label)),
            (MenuItemKind::Error, None) => run.push(format!("[error] {}", inline_text(&row.label))),
            (_, Some(url)) => {
                flush(&mut run, &mut blocks);
                blocks.push(Block::Paragraph {
                    spans: vec![InlineSpan::Link {
                        url: url.clone(),
                        title: None,
                        spans: row.label.clone(),
                        predicate: None,
                    }],
                });
            },
            (_, None) => {
                flush(&mut run, &mut blocks);
                blocks.push(Block::Paragraph {
                    spans: row.label.clone(),
                });
            },
        }
    }
    flush(&mut run, &mut blocks);
    blocks
}

/// Media attached to a [`Block::FeedEntry`] (RSS `<enclosure>`, Atom
/// `rel="enclosure"`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedEnclosure {
    pub url: String,
    #[serde(default)]
    pub media_type: Option<String>,
    #[serde(default)]
    pub byte_length: Option<u64>,
}

/// One row of a [`Block::Menu`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuRow {
    pub kind: MenuItemKind,
    /// The source format's raw item-type character (gopher's `0`, `1`, `i`),
    /// or `None` for a format that has none.
    #[serde(default)]
    pub marker: Option<char>,
    /// The row's text. A search row carries an [`InlineSpan::Submit`] here.
    pub label: Vec<InlineSpan>,
    /// Where activating the row navigates. `None` for info and error rows,
    /// and for a search row, which submits through its label.
    #[serde(default)]
    pub target: Option<String>,
}

/// What a [`MenuRow`] points at: portable across menu formats, while
/// [`MenuRow::marker`] keeps the source's exact type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MenuItemKind {
    /// Display-only text (gopher `i`).
    Info,
    /// A server error line (gopher `3`).
    Error,
    /// A text document (gopher `0`).
    Document,
    /// A submenu or directory (gopher `1`).
    Directory,
    /// A search endpoint that takes a query (gopher `7`).
    Search,
    /// A binary file (gopher `9`, `5`).
    Binary,
    /// An image (gopher `g`, `I`).
    Image,
    /// A sound (gopher `s`).
    Sound,
    /// A terminal session (gopher `8` telnet, `T` tn3270).
    Telnet,
    /// A link out of the format (gopher `h` `URL:` items).
    External,
    /// Any other type; the marker says which.
    Other,
}

impl MenuItemKind {
    /// A stable lowercase name, for markup attributes and diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            MenuItemKind::Info => "info",
            MenuItemKind::Error => "error",
            MenuItemKind::Document => "document",
            MenuItemKind::Directory => "directory",
            MenuItemKind::Search => "search",
            MenuItemKind::Binary => "binary",
            MenuItemKind::Image => "image",
            MenuItemKind::Sound => "sound",
            MenuItemKind::Telnet => "telnet",
            MenuItemKind::External => "external",
            MenuItemKind::Other => "other",
        }
    }

    /// A short label for the type column.
    pub fn label(self) -> &'static str {
        match self {
            MenuItemKind::Info => "",
            MenuItemKind::Error => "ERR",
            MenuItemKind::Document => "TXT",
            MenuItemKind::Directory => "DIR",
            MenuItemKind::Search => "ASK",
            MenuItemKind::Binary => "BIN",
            MenuItemKind::Image => "IMG",
            MenuItemKind::Sound => "SND",
            MenuItemKind::Telnet => "TEL",
            MenuItemKind::External => "URL",
            MenuItemKind::Other => "???",
        }
    }
}

/// A table column's text alignment (djot / markdown `:---`, `:---:`, `---:`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableAlignment {
    /// No explicit alignment marker; the renderer's default (typically left).
    #[default]
    None,
    Left,
    Center,
    Right,
}

/// Source-specified alignment for one rendered block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockAlignment {
    /// Use the reader's ordinary start alignment.
    #[default]
    Start,
    Center,
    End,
}

/// Visual facts carried by a source format without changing its structural
/// block kind. `indent_level` is relative to the reader's configured indent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockPresentation {
    pub alignment: BlockAlignment,
    pub indent_level: u32,
}

/// Source-specified visual facts for inline content. The reader can render
/// them directly or apply an accessibility/contrast override while retaining
/// the source representation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InlinePresentation {
    pub foreground: Option<[u8; 3]>,
    pub background: Option<[u8; 3]>,
    pub underline: bool,
}

/// An inline-level span inside a [`Block`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InlineSpan {
    /// Presentation around an inline subtree. It is intentionally separate
    /// from emphasis/strong semantics and does not make the text interactive.
    Presented {
        presentation: InlinePresentation,
        spans: Vec<InlineSpan>,
    },
    Text(String),
    Code(String),
    Emphasis(Vec<InlineSpan>),
    Strong(Vec<InlineSpan>),
    Link {
        url: String,
        title: Option<String>,
        spans: Vec<InlineSpan>,
        /// Open predicate IRI (statements-over-schema): the link's `rel`, e.g. a
        /// djot `[[Topic]]{rel=schema:cites}`. `None` for a plain link. Knot
        /// ingestion maps this onto a kernel `Semantic` edge predicate.
        #[serde(default)]
        predicate: Option<String>,
    },
    /// A user-triggered submission endpoint. Unlike [`InlineSpan::Link`],
    /// activating this span asks the host to collect a body before any network
    /// request is made.
    Submit {
        target: String,
        spans: Vec<InlineSpan>,
    },
    /// A link within this document. Activation scrolls the current document;
    /// it is never a URL, a fetch, an outgoing link or a link statement.
    InPage {
        target: InPageTarget,
        spans: Vec<InlineSpan>,
    },
    LineBreak,
    SoftBreak,
}

mod traversal;
pub use traversal::*;

#[cfg(test)]
mod tests;
