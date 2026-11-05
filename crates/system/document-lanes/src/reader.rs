/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The shared `genet.reader` lane: held HTML bytes -> fleece article ->
//! portable `EngineDocument` -> the existing document canvas.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use fleece::{Article, ExtractionLineage, Inline, RootSelector};
use inker::{
    Block, ContentLineage, ContentReport, DocumentA11yAction, DocumentA11yBounds, DocumentA11yNode,
    DocumentA11yNodeId, DocumentA11yProjection, DocumentA11yRole, DocumentA11yState,
    DocumentA11ySupport, DocumentCapabilities, DocumentCapabilityStatus, DocumentProvenance,
    DocumentSession, DocumentTrustState, EngineDocument, InlineSpan, SessionClick, SessionEngine,
    SessionError, SessionLink, SessionScrollKey, SessionSpawnRequest,
};
use netrender::Scene;

use genet_host_api::resolve_href;

use crate::smolweb::{SmolwebDocument, SmolwebTheme};

/// Renderer-neutral Reader semantics recovered from the current retained
/// document-canvas packet. It is absent until Reader has presented one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct ReaderAccessibilitySnapshot {
    /// Reader's root name, if Fleece lowered a document title.
    pub root_title: Option<String>,
    /// Logical, currently-visible links. A wrapped link appears once with all
    /// of its clipped viewport-space rectangles.
    pub links: Vec<ReaderAccessibilityLink>,
}

/// One logical Reader link in a [`ReaderAccessibilitySnapshot`].
#[derive(Clone, Debug, PartialEq)]
pub struct ReaderAccessibilityLink {
    /// Opaque identity retained by document-canvas across reflow.
    pub identity: document_canvas::SemanticInteractionId,
    /// Author-lowered link text. Decorative visual arrows are excluded.
    pub label: String,
    pub url: String,
    /// Visible `[x, y, width, height]` rectangles in Reader viewport space.
    pub rects: Vec<[f32; 4]>,
}

fn reader_link_bounds(rects: &[[f32; 4]]) -> Option<DocumentA11yBounds> {
    let mut bounds: Option<(f32, f32, f32, f32)> = None;
    for &[x, y, width, height] in rects {
        if !x.is_finite()
            || !y.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
        {
            continue;
        }
        let right = x + width;
        let bottom = y + height;
        if !right.is_finite() || !bottom.is_finite() {
            continue;
        }
        bounds = Some(match bounds {
            Some((left, top, old_right, old_bottom)) => (
                left.min(x),
                top.min(y),
                old_right.max(right),
                old_bottom.max(bottom),
            ),
            None => (x, y, right, bottom),
        });
    }
    bounds.map(|(x, y, right, bottom)| DocumentA11yBounds {
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
}

/// The only three outcomes of the cheap static reader pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticArticleOutcome {
    Article(Box<Article>),
    NeedsScriptedDom,
    NotReadable,
}

/// Select the scripted fallback only when static fleece extraction found no
/// article and the source actually carries script.
pub fn extract_static_article(source: &str) -> StaticArticleOutcome {
    let dom = genet_static_dom::StaticDocument::parse(source);
    if let Some(article) = fleece::extract_article(&dom) {
        StaticArticleOutcome::Article(Box::new(article))
    } else if fleece::carries_script(&dom) {
        StaticArticleOutcome::NeedsScriptedDom
    } else {
        StaticArticleOutcome::NotReadable
    }
}

fn resolve_reader_href(base: &str, href: &str) -> String {
    url::Url::parse(base)
        .ok()
        .and_then(|base| base.join(href).ok())
        .map(|url| url.to_string())
        .unwrap_or_else(|| resolve_href(base, href))
}

/// Lower a render-free article into the document engine's portable block model.
pub fn lower_article(address: &str, article: &Article) -> EngineDocument {
    let mut blocks = Vec::new();
    for block in &article.blocks {
        lower_block(address, block, &mut blocks);
    }
    EngineDocument {
        address: address.to_string(),
        title: article.title.clone(),
        content_type: "text/html; profile=reader".to_string(),
        lang: article.lang.clone(),
        provenance: DocumentProvenance {
            source_kind: Some(inker::routing::ENGINE_GENET_READER.to_string()),
            canonical_uri: article
                .canonical
                .as_deref()
                .map(|canonical| resolve_reader_href(address, canonical))
                .or_else(|| Some(address.to_string())),
            fetched_at: None,
            source_label: article.site.clone(),
        },
        trust: DocumentTrustState::Unknown,
        diagnostics: Vec::new(),
        navigation: Default::default(),
        blocks,
    }
}

/// Reader rendering is intentionally anchor-blind: Fleece selector evidence remains
/// available to annotation consumers, while this boundary preserves its portable
/// document content.
fn lower_block(address: &str, block: &fleece::AnchoredBlock, out: &mut Vec<Block>) {
    match &block.block {
        fleece::Block::Heading { level, runs } => out.push(Block::Heading {
            level: *level,
            spans: lower_inline(address, runs),
        }),
        fleece::Block::Paragraph { runs } => out.push(Block::Paragraph {
            spans: lower_inline(address, runs),
        }),
        fleece::Block::List { ordered, items } => out.push(Block::List {
            ordered: *ordered,
            items: items
                .iter()
                .map(|item| {
                    let mut blocks = Vec::new();
                    for block in item {
                        lower_block(address, block, &mut blocks);
                    }
                    blocks
                })
                .collect(),
        }),
        fleece::Block::Quote { blocks } => {
            let mut lowered = Vec::new();
            for block in blocks {
                lower_block(address, block, &mut lowered);
            }
            out.push(Block::Quote { blocks: lowered });
        },
        fleece::Block::Code { language, text } => out.push(Block::CodeBlock {
            language: language.clone(),
            text: text.clone(),
        }),
        fleece::Block::Table { table } => {
            let rows = &table.rows;
            let header_index = rows.iter().position(|row| row.header);
            let header = header_index
                .map(|index| {
                    rows[index]
                        .cells
                        .iter()
                        .map(|cell| lower_inline(address, &cell.runs))
                        .collect()
                })
                .unwrap_or_default();
            let rows = rows
                .iter()
                .enumerate()
                .filter(|(index, _)| Some(*index) != header_index)
                .map(|(_, row)| {
                    row.cells
                        .iter()
                        .map(|cell| lower_inline(address, &cell.runs))
                        .collect()
                })
                .collect();
            out.push(Block::Table {
                alignments: Vec::new(),
                header,
                rows,
            });
        },
        fleece::Block::Figure { src, alt, caption } => {
            out.push(Block::Image {
                url: resolve_reader_href(address, src),
                alt: alt.clone(),
            });
            if let Some(caption) = caption {
                out.push(Block::Paragraph {
                    spans: lower_inline(address, caption),
                });
            }
        },
        fleece::Block::Rule => out.push(Block::Rule),
    }
}

fn lower_inline(address: &str, runs: &[Inline]) -> Vec<InlineSpan> {
    runs.iter()
        .map(|run| match run {
            Inline::Text(text) => InlineSpan::Text(text.clone()),
            Inline::Code(code) => InlineSpan::Code(code.clone()),
            Inline::Link { href, runs } => InlineSpan::Link {
                url: resolve_reader_href(address, href),
                title: None,
                spans: lower_inline(address, runs),
                predicate: None,
            },
            Inline::Emphasis { strong, runs } => {
                let runs = lower_inline(address, runs);
                if *strong {
                    InlineSpan::Strong(runs)
                } else {
                    InlineSpan::Emphasis(runs)
                }
            },
        })
        .collect()
}

/// Engine registration for the shared reader lane. A spawn must carry held
/// source bytes; reader mode never refetches.
pub struct ReaderSessionEngine {
    theme: SmolwebTheme,
}

impl ReaderSessionEngine {
    pub fn new(theme: SmolwebTheme) -> Self {
        Self { theme }
    }
}

impl Default for ReaderSessionEngine {
    fn default() -> Self {
        Self::new(SmolwebTheme::default())
    }
}

impl SessionEngine<Scene> for ReaderSessionEngine {
    fn engine_id(&self) -> &str {
        inker::routing::ENGINE_GENET_READER
    }

    fn spawn(
        &self,
        request: &SessionSpawnRequest,
    ) -> Result<Box<dyn DocumentSession<Scene>>, SessionError> {
        let source = request.body.as_deref().ok_or_else(|| {
            SessionError::Unsupported(
                "genet.reader requires the source bytes already held by the host".to_string(),
            )
        })?;
        let article = match extract_static_article(source) {
            StaticArticleOutcome::Article(article) => article,
            StaticArticleOutcome::NeedsScriptedDom => {
                return Err(SessionError::Unsupported(
                    "static fleece extraction found an empty scripted shell; retry over the post-JS DOM"
                        .to_string(),
                ));
            },
            StaticArticleOutcome::NotReadable => {
                return Err(SessionError::SpawnFailed(
                    "fleece found no single readable article".to_string(),
                ));
            },
        };
        let source = Arc::new(ReaderDocumentSource {
            document: Arc::new(lower_article(&request.address, &article)),
            lineage: article.lineage.clone(),
            theme: self.theme.clone(),
        });
        Ok(Box::new(ReaderDocumentSession::from_source(
            source,
            request.viewport,
        )))
    }
}

/// Retained reader rendering plus the fleece derivation that made it.
pub struct ReaderDocumentSession {
    /// Durable, renderer-neutral content. Appearances retain this by `Arc`;
    /// their document-canvas state below is deliberately not shared.
    source: Arc<ReaderDocumentSource>,
    doc: SmolwebDocument,
    viewport: (u32, u32),
    lineage: ExtractionLineage,
    /// Every presentation-affecting turn revokes prior geometry and actions.
    /// The public projection carries this revision so Pelt makes queued
    /// virtual Focus inert rather than retargeting it after a reflow.
    accessibility_revision: u64,
    focused_semantic: Option<DocumentA11yNodeId>,
    /// Document-canvas keeps link identities opaque. Retain an engine-local
    /// projection ID for each one instead of deriving identity from output
    /// order, URL, or visible rectangles.
    accessibility_nodes: RefCell<ReaderAccessibilityNodeMap>,
}

/// The immutable Reader content shared by each visible appearance of one
/// document. It has no viewport, scroll, retained layout, or focus state.
struct ReaderDocumentSource {
    document: Arc<EngineDocument>,
    lineage: ExtractionLineage,
    theme: SmolwebTheme,
}

#[derive(Default)]
struct ReaderAccessibilityNodeMap {
    ids: HashMap<document_canvas::SemanticInteractionId, DocumentA11yNodeId>,
    next: u64,
}

impl DocumentSession<Scene> for ReaderDocumentSession {
    fn document_find(
        &mut self,
        query: &inker::DocumentFindQuery,
    ) -> Result<inker::DocumentFindState, inker::SessionError> {
        Ok(self.doc.document_find(query))
    }
    fn document_find_step(
        &mut self,
        direction: inker::DocumentFindDirection,
    ) -> Result<inker::DocumentFindState, inker::SessionError> {
        Ok(self.doc.document_find_step(direction))
    }
    fn clear_document_find(&mut self) -> Result<(), inker::SessionError> {
        self.doc.clear_document_find();
        Ok(())
    }

    fn document_capabilities(&self) -> DocumentCapabilities {
        DocumentCapabilities {
            find_in_page: DocumentCapabilityStatus::Supported,
            page_zoom: DocumentCapabilityStatus::unsupported(
                "reader sessions do not expose page zoom",
            ),
            page_capture: DocumentCapabilityStatus::unsupported(
                "reader sessions do not capture rendered pages",
            ),
            navigation: DocumentCapabilityStatus::Partial {
                detail: "the host owns document lineage, policy, and refetch".into(),
            },
        }
    }

    fn frame(&mut self, width: u32, height: u32) -> Scene {
        let before = self.accessibility_snapshot();
        self.viewport = (width, height);
        let scene = self.doc.frame(width, height);
        if self.accessibility_snapshot() != before {
            self.bump_accessibility_revision();
        }
        scene
    }

    fn scroll_by(&mut self, dx: f32, dy: f32) -> bool {
        let changed = self.doc.scroll_by(dx, dy);
        if changed {
            self.bump_accessibility_revision();
        }
        changed
    }

    fn scroll_at(&mut self, x: f32, y: f32, dx: f32, dy: f32) -> bool {
        let changed = self.doc.scroll_at(x, y, dx, dy);
        if changed {
            self.bump_accessibility_revision();
        }
        changed
    }

    fn scroll_for_key(&mut self, key: SessionScrollKey) -> bool {
        let changed = self.doc.scroll_for_key(key);
        if changed {
            self.bump_accessibility_revision();
        }
        changed
    }

    fn scroll_to(&mut self, y: f32) {
        let before = self.accessibility_snapshot();
        self.doc.scroll_to(y);
        if self.accessibility_snapshot() != before {
            self.bump_accessibility_revision();
        }
    }

    fn click_at(&mut self, x: f32, y: f32) -> SessionClick {
        let (width, height) = self.viewport;
        self.doc.activate_at(x, y, width, height)
    }

    fn links(&self) -> Vec<SessionLink> {
        self.doc
            .links()
            .into_iter()
            .map(|(url, rect)| SessionLink { url, rect })
            .collect()
    }

    fn content_height(&mut self, width: u32, height: u32) -> u32 {
        self.doc.content_height(width, height)
    }

    fn subresources(&self) -> Vec<String> {
        self.doc.subresources()
    }

    fn provide_subresource(&mut self, url: &str, bytes: &[u8]) -> bool {
        self.doc.provide_subresource(url, bytes)
    }

    fn inspect(&self) -> Option<ContentReport> {
        let document = self.doc.document();
        let mut headings = Vec::new();
        collect_headings(&document.blocks, &mut headings);
        Some(ContentReport {
            title: document.title.clone(),
            links: document
                .outgoing_links()
                .into_iter()
                .map(str::to_string)
                .collect(),
            headings,
            lineage: Some(content_lineage(&self.lineage)),
            ..Default::default()
        })
    }

    fn accessibility_projection(&self) -> Option<DocumentA11yProjection> {
        self.document_accessibility_projection()
    }

    fn dispatch_accessibility_action(
        &mut self,
        request: &inker::DocumentA11yActionRequest,
    ) -> bool {
        if request.action != DocumentA11yAction::Focus || request.data.is_some() {
            return false;
        }
        let Some(projection) = self.document_accessibility_projection() else {
            return false;
        };
        if projection.revision() != request.revision
            || !projection.node(request.target).is_some_and(|node| {
                !node.state.hidden
                    && !node.state.disabled
                    && node.actions.contains(&DocumentA11yAction::Focus)
            })
        {
            return false;
        }
        if self.focused_semantic != Some(request.target) {
            self.focused_semantic = Some(request.target);
            self.bump_accessibility_revision();
        }
        true
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

fn collect_headings(blocks: &[Block], out: &mut Vec<String>) {
    for block in blocks {
        match block {
            Block::Presented { block, .. } => collect_headings(std::slice::from_ref(block), out),
            Block::Heading { spans, .. } => out.push(inker::inline_text(spans)),
            Block::Quote { blocks } => collect_headings(blocks, out),
            Block::List { items, .. } => {
                for item in items {
                    collect_headings(item, out);
                }
            },
            _ => {},
        }
    }
}

fn content_lineage(lineage: &ExtractionLineage) -> ContentLineage {
    let (selector, score) = match &lineage.root_selector {
        RootSelector::Main => ("main".to_string(), None),
        RootSelector::ScoredCandidate { tag, score } => (format!("scored {tag}"), Some(*score)),
    };
    ContentLineage {
        tool: "fleece".to_string(),
        version: lineage.fleece_version.clone(),
        selector,
        score,
        block_count: lineage.block_count,
    }
}

#[cfg(test)]
#[path = "reader/tests.rs"]
mod tests;

#[path = "reader/session.rs"]
mod session;
