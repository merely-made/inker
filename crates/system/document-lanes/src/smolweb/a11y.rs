// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The smolweb document's accessibility projection.
//!
//! A screen reader reads a smolweb page through this: every visible block
//! with its role and its laid-out box, every link, submission and collapsible
//! heading as a node it can focus, scroll to and activate, in document order.
//! It is built from the engine document (what each block is) and the retained
//! document-canvas packet (where each block and interaction is), and it is
//! published only after a frame has presented that packet.
//!
//! Hosts lower it through `DocumentSession::accessibility_projection`; Click
//! travels through `accessibility_click_target` and the host's ordinary
//! pointer path, and Focus and ScrollIntoView through
//! `dispatch_accessibility_action`.
//!
//! Turnstone's unusual-protocols plan, stage S8 (Mere half, its U16).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use document_canvas::{InteractionKind, Rect, RenderedBlock, RenderedBlockKind};
use inker::{
    A11yCapability, Block, DocumentA11yAction, DocumentA11yActionRequest, DocumentA11yBounds,
    DocumentA11yClickTarget, DocumentA11yNode, DocumentA11yNodeId, DocumentA11yPoint,
    DocumentA11yProjection, DocumentA11yRole, DocumentA11yState, DocumentA11ySupport, InlineSpan,
    MenuItemKind, MenuRow, inline_text,
};

use super::{Focus, SmolwebDocument, rect_contains};

/// What a projected node stands for. Kept per id so an action addressed to a
/// node is checked against the current document, not against the geometry
/// the screen reader last saw.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum NodeKey {
    Root,
    /// A block, by its path through the rendered packet's source indices.
    Block(Vec<usize>),
    /// One run of text inside a block, by its position among the block's
    /// children.
    Text(Vec<usize>, usize),
    /// One keyboard focus stop: a link, a submission or a collapsible heading.
    Stop(Focus),
}

/// Engine-local node ids, allocated once per key and never reused, so a
/// queued action can only become stale, never land on a different node.
#[derive(Debug, Default)]
pub(crate) struct NodeIds {
    ids: HashMap<NodeKey, DocumentA11yNodeId>,
    keys: HashMap<DocumentA11yNodeId, NodeKey>,
    next: u64,
}

impl NodeIds {
    fn id(&mut self, key: NodeKey) -> DocumentA11yNodeId {
        if let Some(id) = self.ids.get(&key) {
            return *id;
        }
        self.next = self
            .next
            .checked_add(1)
            .expect("smolweb accessibility node ids exhausted");
        let id = DocumentA11yNodeId::new(self.next);
        self.ids.insert(key.clone(), id);
        self.keys.insert(id, key);
        id
    }

    fn key(&self, id: DocumentA11yNodeId) -> Option<&NodeKey> {
        self.keys.get(&id)
    }
}

const LIMITATIONS: [&str; 2] = [
    "Text runs carry their block's box; per-line text geometry is not projected.",
    "Preformatted and code blocks are one text node each.",
];

/// A keyboard stop, with the indices of its interaction regions.
type Stop = (Focus, Vec<usize>);

impl SmolwebDocument {
    /// The projection's revision: it changes whenever geometry, scroll, size
    /// or focus does, so a host can tell a stale node or action from a
    /// current one.
    pub(crate) fn accessibility_revision(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.layout_generation.hash(&mut hasher);
        self.scroll_x.to_bits().hash(&mut hasher);
        self.scroll_y.to_bits().hash(&mut hasher);
        self.size.hash(&mut hasher);
        self.focus.hash(&mut hasher);
        hasher.finish()
    }

    /// The current projection, or `None` before a frame has presented the
    /// retained layout.
    pub(crate) fn accessibility_projection(
        &self,
        ids: &mut NodeIds,
    ) -> Option<DocumentA11yProjection> {
        if !self.presented {
            return None;
        }
        let layout = self.layout.as_ref()?;
        let mut builder = Builder {
            doc: self,
            ids,
            nodes: Vec::new(),
            consumed: Vec::new(),
            stops: self.focus_stops(),
        };
        builder.consumed = vec![false; builder.stops.len()];
        let root = builder.ids.id(NodeKey::Root);
        let mut children = Vec::new();
        for (index, block) in self.document.blocks.iter().enumerate() {
            let Some(rendered) = layout
                .packet
                .blocks
                .iter()
                .find(|rendered| rendered.source_block_index == index)
            else {
                // Collapsed under a heading, or empty: not presented.
                continue;
            };
            if let Some(child) = builder.block(block, rendered, vec![index], root) {
                children.push(child);
            }
        }
        let title = self
            .document
            .title
            .clone()
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| "Untitled page".to_owned());
        let mut nodes = vec![node(
            root,
            None,
            children,
            DocumentA11yRole::Document,
            Some(title),
            Some(DocumentA11yBounds {
                x: 0.0,
                y: 0.0,
                width: self.size.0 as f32,
                height: self.size.1 as f32,
            }),
        )];
        nodes.append(&mut builder.nodes);
        let support = DocumentA11ySupport::new(A11yCapability::Partial, LIMITATIONS)
            .expect("the smolweb projection names its limitations");
        Some(DocumentA11yProjection::new(
            self.accessibility_revision(),
            support,
            root,
            nodes,
        ))
    }

    /// A current, visible point on a link, submission or collapsible heading,
    /// for the host's ordinary click path. `None` when the node is stale, not
    /// clickable, or scrolled out of view (the host asks for ScrollIntoView
    /// first).
    pub(crate) fn accessibility_click_target(
        &self,
        ids: &NodeIds,
        target: DocumentA11yNodeId,
    ) -> Option<DocumentA11yClickTarget> {
        if !self.presented {
            return None;
        }
        let Some(NodeKey::Stop(focus)) = ids.key(target) else {
            return None;
        };
        let layout = self.layout.as_ref()?;
        let regions = self.stop_regions(focus)?;
        for &index in &regions {
            let Some([x, y, width, height]) =
                self.viewport_rect(layout.packet.interactions[index].bounds)
            else {
                continue;
            };
            let point = (x + width * 0.5, y + height * 0.5);
            let topmost = layout.packet.interactions.iter().rposition(|candidate| {
                rect_contains(
                    candidate.bounds,
                    point.0 + self.scroll_x,
                    point.1 + self.scroll_y,
                )
            });
            if topmost.is_some_and(|topmost| regions.contains(&topmost)) {
                return Some(DocumentA11yClickTarget {
                    revision: self.accessibility_revision(),
                    point: DocumentA11yPoint {
                        x: point.0,
                        y: point.1,
                    },
                });
            }
        }
        None
    }

    /// Focus or scroll to a node. Returns `true` only for a current revision,
    /// a known node and an action that node advertises. Click returns
    /// `false`: it travels through the click target and the pointer path.
    pub(crate) fn dispatch_accessibility_action(
        &mut self,
        ids: &NodeIds,
        request: &DocumentA11yActionRequest,
    ) -> bool {
        if !self.presented || request.revision != self.accessibility_revision() {
            return false;
        }
        let Some(key) = ids.key(request.target).cloned() else {
            return false;
        };
        match (request.action, key) {
            (DocumentA11yAction::Focus, NodeKey::Stop(focus)) => {
                let Some(regions) = self.stop_regions(&focus) else {
                    return false;
                };
                self.scroll_into_view(&regions);
                self.focus = Some(focus);
                true
            },
            (DocumentA11yAction::ScrollIntoView, NodeKey::Stop(focus)) => {
                let Some(regions) = self.stop_regions(&focus) else {
                    return false;
                };
                self.scroll_into_view(&regions);
                true
            },
            (DocumentA11yAction::ScrollIntoView, NodeKey::Block(path) | NodeKey::Text(path, _)) => {
                // A list item, table row or cell scrolls with the nearest
                // block its path reaches.
                let Some(bounds) = (1..=path.len())
                    .rev()
                    .find_map(|len| self.block_at_path(&path[..len]))
                    .map(|block| block.bounds)
                else {
                    return false;
                };
                self.scroll_rect_into_view(bounds);
                true
            },
            _ => false,
        }
    }

    fn stop_regions(&self, focus: &Focus) -> Option<Vec<usize>> {
        self.focus_stops()
            .into_iter()
            .find(|(stop, _)| stop == focus)
            .map(|(_, regions)| regions)
    }

    fn block_at_path(&self, path: &[usize]) -> Option<&RenderedBlock> {
        let layout = self.layout.as_ref()?;
        let (&first, rest) = path.split_first()?;
        let mut block = layout
            .packet
            .blocks
            .iter()
            .find(|block| block.source_block_index == first)?;
        for &index in rest {
            let RenderedBlockKind::Group { children } = &block.kind else {
                return None;
            };
            block = children
                .iter()
                .find(|child| child.source_block_index == index)?;
        }
        Some(block)
    }

    fn scroll_rect_into_view(&mut self, bounds: Rect) {
        let (width, height) = (self.size.0 as f32, self.size.1 as f32);
        self.scroll_y = super::into_view(self.scroll_y, bounds.origin.y, bounds.max_y(), height)
            .clamp(0.0, self.max_scroll());
        self.scroll_x = super::into_view(self.scroll_x, bounds.origin.x, bounds.max_x(), width)
            .clamp(0.0, self.max_scroll_x());
    }

    /// A packet rectangle in the session's viewport coordinates, unclipped:
    /// a node below the fold keeps its true place, so a screen reader can
    /// ask to scroll to it.
    fn viewport_bounds(&self, rect: Rect) -> Option<DocumentA11yBounds> {
        let bounds = DocumentA11yBounds {
            x: rect.origin.x - self.scroll_x,
            y: rect.origin.y - self.scroll_y,
            width: rect.size.width,
            height: rect.size.height,
        };
        let finite = [bounds.x, bounds.y, bounds.width, bounds.height]
            .iter()
            .all(|value| value.is_finite());
        (finite && bounds.width > 0.0 && bounds.height > 0.0).then_some(bounds)
    }
}

/// One child inside a block's text, in reading order.
enum Piece {
    Text(String),
    /// A link, in-page link or submission, with its readable label.
    Stop {
        submit: bool,
        label: String,
    },
}

fn pieces(spans: &[InlineSpan]) -> Vec<Piece> {
    fn walk(spans: &[InlineSpan], text: &mut String, out: &mut Vec<Piece>) {
        for span in spans {
            match span {
                InlineSpan::Text(value) | InlineSpan::Code(value) => text.push_str(value),
                InlineSpan::Presented { spans, .. }
                | InlineSpan::Emphasis(spans)
                | InlineSpan::Strong(spans) => walk(spans, text, out),
                InlineSpan::LineBreak | InlineSpan::SoftBreak => text.push(' '),
                InlineSpan::Link { spans, .. } | InlineSpan::InPage { spans, .. } => {
                    flush(text, out);
                    out.push(Piece::Stop {
                        submit: false,
                        label: inline_text(spans),
                    });
                },
                InlineSpan::Submit { spans, .. } => {
                    flush(text, out);
                    out.push(Piece::Stop {
                        submit: true,
                        label: inline_text(spans),
                    });
                },
            }
        }
    }
    fn flush(text: &mut String, out: &mut Vec<Piece>) {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            out.push(Piece::Text(trimmed.to_owned()));
        }
        text.clear();
    }
    let mut text = String::new();
    let mut out = Vec::new();
    walk(spans, &mut text, &mut out);
    flush(&mut text, &mut out);
    out
}

fn node(
    id: DocumentA11yNodeId,
    parent: Option<DocumentA11yNodeId>,
    children: Vec<DocumentA11yNodeId>,
    role: DocumentA11yRole,
    name: Option<String>,
    bounds: Option<DocumentA11yBounds>,
) -> DocumentA11yNode {
    DocumentA11yNode {
        id,
        parent,
        children,
        role,
        description: None,
        name,
        value: None,
        numeric_value: None,
        numeric_minimum: None,
        numeric_maximum: None,
        bounds,
        state: DocumentA11yState::default(),
        actions: Vec::new(),
    }
}

fn union(rects: impl IntoIterator<Item = Rect>) -> Option<Rect> {
    rects.into_iter().reduce(|a, b| {
        let left = a.origin.x.min(b.origin.x);
        let top = a.origin.y.min(b.origin.y);
        let right = a.max_x().max(b.max_x());
        let bottom = a.max_y().max(b.max_y());
        Rect::from_xywh(left, top, right - left, bottom - top)
    })
}

fn intersects(a: Rect, b: Rect) -> bool {
    a.origin.x < b.max_x()
        && b.origin.x < a.max_x()
        && a.origin.y < b.max_y()
        && b.origin.y < a.max_y()
}

struct Builder<'a> {
    doc: &'a SmolwebDocument,
    ids: &'a mut NodeIds,
    nodes: Vec<DocumentA11yNode>,
    stops: Vec<Stop>,
    /// Stops already given to a node.
    consumed: Vec<bool>,
}

impl Builder<'_> {
    fn regions_bounds(&self, regions: &[usize]) -> Option<Rect> {
        let layout = self.doc.layout.as_ref()?;
        union(
            regions
                .iter()
                .map(|&index| layout.packet.interactions[index].bounds),
        )
    }

    /// Take the first stop not yet given to a node that matches `wanted` and
    /// lies inside `within`. Blocks do not overlap, so a block cannot take
    /// another block's stop, and stops arrive in document order, so a
    /// block's links come out in reading order.
    fn take(&mut self, within: Rect, wanted: impl Fn(&Focus) -> bool) -> Option<Stop> {
        let index = (0..self.stops.len()).find(|&index| {
            !self.consumed[index]
                && wanted(&self.stops[index].0)
                && self
                    .regions_bounds(&self.stops[index].1)
                    .is_some_and(|bounds| intersects(bounds, within))
        })?;
        self.consumed[index] = true;
        Some(self.stops[index].clone())
    }

    /// The next link (`submit == false`) or submission inside `within`.
    fn next_stop(&mut self, within: Rect, submit: bool) -> Option<Stop> {
        self.take(within, |focus| match focus {
            Focus::Submit(..) => submit,
            Focus::Link(_) => !submit,
            Focus::Fold(_) => false,
        })
    }

    /// The collapsible-heading stop inside `within`, and whether it is open.
    fn fold_stop(&mut self, within: Rect) -> Option<(Stop, bool)> {
        let (focus, regions) = self.take(within, |focus| matches!(focus, Focus::Fold(_)))?;
        let layout = self.doc.layout.as_ref()?;
        let InteractionKind::Fold { fold } = layout.packet.interactions[regions[0]].kind else {
            return None;
        };
        let open = self.doc.folds.is_open(&self.doc.document.navigation, fold);
        Some(((focus, regions), open))
    }

    /// Every remaining link or submission inside `within`, for blocks whose
    /// links are not spans (feed entries, metadata).
    fn stops_within(&mut self, within: Rect) -> Vec<Stop> {
        let mut out = Vec::new();
        while let Some(stop) = self.take(within, |focus| !matches!(focus, Focus::Fold(_))) {
            out.push(stop);
        }
        out
    }

    fn stop_node(
        &mut self,
        parent: DocumentA11yNodeId,
        (focus, regions): Stop,
        label: Option<String>,
        description: Option<String>,
    ) -> DocumentA11yNodeId {
        let id = self.ids.id(NodeKey::Stop(focus.clone()));
        let layout = self
            .doc
            .layout
            .as_ref()
            .expect("a stop comes from the layout");
        let role = match focus {
            Focus::Submit(..) => DocumentA11yRole::Button,
            _ => DocumentA11yRole::Link,
        };
        let name = label.filter(|label| !label.trim().is_empty()).or_else(|| {
            regions.iter().find_map(|&index| {
                layout.packet.interactions[index]
                    .link_semantics
                    .as_ref()
                    .map(|semantics| semantics.accessible_label.clone())
            })
        });
        let bounds = self
            .regions_bounds(&regions)
            .and_then(|rect| self.doc.viewport_bounds(rect));
        let mut entry = node(id, Some(parent), Vec::new(), role, name, bounds);
        entry.description = description;
        entry.state.focused = self.doc.focus.as_ref() == Some(&focus);
        entry.actions = vec![
            DocumentA11yAction::Click,
            DocumentA11yAction::Focus,
            DocumentA11yAction::ScrollIntoView,
        ];
        self.nodes.push(entry);
        id
    }

    fn text_node(
        &mut self,
        parent: DocumentA11yNodeId,
        path: &[usize],
        position: usize,
        text: String,
        bounds: Option<DocumentA11yBounds>,
    ) -> DocumentA11yNodeId {
        let id = self.ids.id(NodeKey::Text(path.to_vec(), position));
        let mut entry = node(
            id,
            Some(parent),
            Vec::new(),
            DocumentA11yRole::StaticText,
            Some(text),
            bounds,
        );
        entry.actions = vec![DocumentA11yAction::ScrollIntoView];
        self.nodes.push(entry);
        id
    }

    /// A block whose content is inline spans: one named node when it is all
    /// text, otherwise its text runs and stops as children in reading order,
    /// so no link is read twice.
    fn inline_block(
        &mut self,
        role: DocumentA11yRole,
        spans: &[InlineSpan],
        rendered: &RenderedBlock,
        path: Vec<usize>,
        parent: DocumentA11yNodeId,
        fold: Option<(Stop, bool)>,
    ) -> DocumentA11yNodeId {
        let id = match &fold {
            Some(((focus, _), _)) => self.ids.id(NodeKey::Stop(focus.clone())),
            None => self.ids.id(NodeKey::Block(path.clone())),
        };
        let bounds = self.doc.viewport_bounds(rendered.bounds);
        let pieces = pieces(spans);
        let only_text = pieces.iter().all(|piece| matches!(piece, Piece::Text(_)));
        let mut children = Vec::new();
        let mut name = None;
        if only_text {
            name = Some(inline_text(spans).trim().to_owned()).filter(|text| !text.is_empty());
        } else {
            for (position, piece) in pieces.into_iter().enumerate() {
                let child = match piece {
                    Piece::Text(text) => self.text_node(id, &path, position, text, bounds),
                    Piece::Stop { submit, label } => {
                        match self.next_stop(rendered.bounds, submit) {
                            Some(stop) => self.stop_node(id, stop, Some(label), None),
                            // No laid-out region (an empty label): read it
                            // as text rather than steal the next block's.
                            None => self.text_node(id, &path, position, label, bounds),
                        }
                    },
                };
                children.push(child);
            }
        }
        let mut entry = node(id, Some(parent), children, role, name, bounds);
        entry.actions = vec![DocumentA11yAction::ScrollIntoView];
        if let Some(((focus, _), open)) = fold {
            entry.state.expanded = Some(open);
            entry.state.focused = self.doc.focus.as_ref() == Some(&focus);
            entry.actions = vec![
                DocumentA11yAction::Click,
                DocumentA11yAction::Focus,
                DocumentA11yAction::ScrollIntoView,
            ];
        }
        self.nodes.push(entry);
        id
    }

    /// A container node over child blocks found under `rendered`'s group.
    fn container(
        &mut self,
        role: DocumentA11yRole,
        name: Option<String>,
        rendered: &RenderedBlock,
        path: Vec<usize>,
        parent: DocumentA11yNodeId,
        children: Vec<(&Block, usize)>,
    ) -> DocumentA11yNodeId {
        let id = self.ids.id(NodeKey::Block(path.clone()));
        let rendered_children: &[RenderedBlock] = match &rendered.kind {
            RenderedBlockKind::Group { children } => children,
            _ => &[],
        };
        let mut ids = Vec::new();
        for (block, index) in children {
            let Some(child) = rendered_children
                .iter()
                .find(|child| child.source_block_index == index)
            else {
                continue;
            };
            let mut child_path = path.clone();
            child_path.push(index);
            if let Some(child) = self.block(block, child, child_path, id) {
                ids.push(child);
            }
        }
        let mut entry = node(
            id,
            Some(parent),
            ids,
            role,
            name,
            self.doc.viewport_bounds(rendered.bounds),
        );
        entry.actions = vec![DocumentA11yAction::ScrollIntoView];
        self.nodes.push(entry);
        id
    }

    /// A leaf whose text is not spans (feed rows, metadata, code): one text
    /// node, with any links laid out inside it as children.
    fn text_block(
        &mut self,
        role: DocumentA11yRole,
        text: String,
        rendered: &RenderedBlock,
        path: Vec<usize>,
        parent: DocumentA11yNodeId,
    ) -> DocumentA11yNodeId {
        let id = self.ids.id(NodeKey::Block(path));
        let stops = self.stops_within(rendered.bounds);
        let children = stops
            .into_iter()
            .map(|stop| self.stop_node(id, stop, None, None))
            .collect();
        let name = Some(text.trim().to_owned()).filter(|text| !text.is_empty());
        let mut entry = node(
            id,
            Some(parent),
            children,
            role,
            name,
            self.doc.viewport_bounds(rendered.bounds),
        );
        entry.actions = vec![DocumentA11yAction::ScrollIntoView];
        self.nodes.push(entry);
        id
    }

    fn block(
        &mut self,
        block: &Block,
        rendered: &RenderedBlock,
        path: Vec<usize>,
        parent: DocumentA11yNodeId,
    ) -> Option<DocumentA11yNodeId> {
        let base = path.last().copied().unwrap_or(0).saturating_mul(1000);
        Some(match block {
            Block::Presented { block, .. } => return self.block(block, rendered, path, parent),
            Block::Heading { level, spans } => {
                let fold = self.fold_stop(rendered.bounds);
                self.inline_block(
                    DocumentA11yRole::Heading { level: *level },
                    spans,
                    rendered,
                    path,
                    parent,
                    fold,
                )
            },
            Block::Paragraph { spans } => self.inline_block(
                DocumentA11yRole::Paragraph,
                spans,
                rendered,
                path,
                parent,
                None,
            ),
            Block::Quote { blocks } => {
                let children = blocks
                    .iter()
                    .enumerate()
                    .map(|(i, child)| (child, base + i))
                    .collect();
                self.container(
                    DocumentA11yRole::Group,
                    None,
                    rendered,
                    path,
                    parent,
                    children,
                )
            },
            Block::List { items, .. } => self.list(items, rendered, path, parent),
            Block::Menu { rows } => self.menu(rows, rendered, path, parent),
            Block::Table { header, rows, .. } => self.table(header, rows, rendered, path, parent),
            Block::Image { alt, .. } => {
                let id = self.ids.id(NodeKey::Block(path));
                let name = Some(alt.clone()).filter(|alt| !alt.trim().is_empty());
                let mut entry = node(
                    id,
                    Some(parent),
                    Vec::new(),
                    DocumentA11yRole::Image,
                    name,
                    self.doc.viewport_bounds(rendered.bounds),
                );
                entry.actions = vec![DocumentA11yAction::ScrollIntoView];
                self.nodes.push(entry);
                id
            },
            Block::Rule => return None,
            // A blank line (Micron lowers each one to an empty block) is
            // space, not something to stop on.
            Block::CodeBlock { text, .. } | Block::Preformatted { text }
                if text.trim().is_empty() =>
            {
                return None;
            },
            Block::CodeBlock { text, .. } | Block::Preformatted { text } => self.text_block(
                DocumentA11yRole::StaticText,
                text.clone(),
                rendered,
                path,
                parent,
            ),
            Block::FeedHeader {
                title, subtitle, ..
            } => {
                let text = match subtitle {
                    Some(subtitle) => format!("{title}. {subtitle}"),
                    None => title.clone(),
                };
                self.text_block(DocumentA11yRole::Group, text, rendered, path, parent)
            },
            Block::FeedEntry {
                title,
                date,
                summary,
                ..
            } => {
                let text = [Some(title.clone()), date.clone(), summary.clone()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(". ");
                self.text_block(DocumentA11yRole::Article, text, rendered, path, parent)
            },
            Block::MetadataRow { label, value } => self.text_block(
                DocumentA11yRole::StaticText,
                format!("{label}: {value}"),
                rendered,
                path,
                parent,
            ),
            Block::Badge { text } => {
                self.text_block(DocumentA11yRole::Note, text.clone(), rendered, path, parent)
            },
            other => self.text_block(
                DocumentA11yRole::Unknown,
                other.kind_name().to_owned(),
                rendered,
                path,
                parent,
            ),
        })
    }

    fn list(
        &mut self,
        items: &[Vec<Block>],
        rendered: &RenderedBlock,
        path: Vec<usize>,
        parent: DocumentA11yNodeId,
    ) -> DocumentA11yNodeId {
        let id = self.ids.id(NodeKey::Block(path.clone()));
        let base = path.last().copied().unwrap_or(0).saturating_mul(1000);
        let rendered_children: &[RenderedBlock] = match &rendered.kind {
            RenderedBlockKind::Group { children } => children,
            _ => &[],
        };
        let mut item_ids = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let item_id = self.ids.id(NodeKey::Text(path.clone(), i));
            let mut children = Vec::new();
            let mut item_rects = Vec::new();
            for (j, block) in item.iter().enumerate() {
                let index = base + i.saturating_mul(100) + j;
                let Some(child) = rendered_children
                    .iter()
                    .find(|child| child.source_block_index == index)
                else {
                    continue;
                };
                item_rects.push(child.bounds);
                let mut child_path = path.clone();
                child_path.push(index);
                if let Some(child) = self.block(block, child, child_path, item_id) {
                    children.push(child);
                }
            }
            if children.is_empty() {
                continue;
            }
            let bounds = union(item_rects).and_then(|rect| self.doc.viewport_bounds(rect));
            let mut entry = node(
                item_id,
                Some(id),
                children,
                DocumentA11yRole::ListItem,
                None,
                bounds,
            );
            entry.actions = vec![DocumentA11yAction::ScrollIntoView];
            self.nodes.push(entry);
            item_ids.push(item_id);
        }
        let mut entry = node(
            id,
            Some(parent),
            item_ids,
            DocumentA11yRole::List,
            None,
            self.doc.viewport_bounds(rendered.bounds),
        );
        entry.actions = vec![DocumentA11yAction::ScrollIntoView];
        self.nodes.push(entry);
        id
    }

    /// A typed menu, one row per line. Each row names its kind ("Directory",
    /// "Search"), and a row with a target is its link, so a reader hears the
    /// row once. Rows take their own line's box when the laid-out lines
    /// match the rows one to one.
    fn menu(
        &mut self,
        rows: &[MenuRow],
        rendered: &RenderedBlock,
        path: Vec<usize>,
        parent: DocumentA11yNodeId,
    ) -> DocumentA11yNodeId {
        let id = self.ids.id(NodeKey::Block(path.clone()));
        let lines = line_boxes(rendered);
        let row_box = |row: usize| {
            if lines.len() == rows.len() {
                lines[row]
            } else {
                rendered.bounds
            }
        };
        let mut row_ids = Vec::new();
        for (row, entry) in rows.iter().enumerate() {
            let row_id = self.ids.id(NodeKey::Text(path.clone(), row));
            let bounds = self.doc.viewport_bounds(row_box(row));
            let kind = kind_name(entry.kind);
            let label = inline_text(&entry.label).trim().to_owned();
            let interactive = entry.target.is_some()
                || entry
                    .label
                    .iter()
                    .any(|span| matches!(span, InlineSpan::Submit { .. }));
            let submit = entry.target.is_none();
            let stop = interactive
                .then(|| self.next_stop(row_box(row), submit))
                .flatten();
            let children = match stop {
                Some(stop) => vec![self.stop_node(
                    row_id,
                    stop,
                    Some(label.clone()),
                    Some(kind.clone()).filter(|kind| !kind.is_empty()),
                )],
                None => Vec::new(),
            };
            let name = children.is_empty().then(|| match kind.as_str() {
                "" => label.clone(),
                kind => format!("{kind}: {label}"),
            });
            let mut item = node(
                row_id,
                Some(id),
                children,
                DocumentA11yRole::ListItem,
                name,
                bounds,
            );
            item.actions = vec![DocumentA11yAction::ScrollIntoView];
            self.nodes.push(item);
            row_ids.push(row_id);
        }
        let mut entry = node(
            id,
            Some(parent),
            row_ids,
            DocumentA11yRole::List,
            None,
            self.doc.viewport_bounds(rendered.bounds),
        );
        entry.actions = vec![DocumentA11yAction::ScrollIntoView];
        self.nodes.push(entry);
        id
    }

    /// A table, header row first, its cells taking the laid-out cell boxes
    /// in row-major order.
    fn table(
        &mut self,
        header: &[Vec<InlineSpan>],
        rows: &[Vec<Vec<InlineSpan>>],
        rendered: &RenderedBlock,
        path: Vec<usize>,
        parent: DocumentA11yNodeId,
    ) -> DocumentA11yNodeId {
        let id = self.ids.id(NodeKey::Block(path.clone()));
        let cells: &[RenderedBlock] = match &rendered.kind {
            RenderedBlockKind::Group { children } => children,
            _ => &[],
        };
        let mut next_cell = cells.iter();
        let all_rows = (!header.is_empty())
            .then_some(header)
            .into_iter()
            .chain(rows.iter().map(Vec::as_slice));
        let mut row_ids = Vec::new();
        for (r, row) in all_rows.enumerate() {
            let row_id = self.ids.id(NodeKey::Text(path.clone(), r));
            let mut cell_ids = Vec::new();
            let mut row_rects = Vec::new();
            for (c, spans) in row.iter().enumerate() {
                let Some(cell) = next_cell.next() else {
                    break;
                };
                row_rects.push(cell.bounds);
                let mut cell_path = path.clone();
                cell_path.extend([r, c]);
                cell_ids.push(self.inline_block(
                    DocumentA11yRole::Cell,
                    spans,
                    cell,
                    cell_path,
                    row_id,
                    None,
                ));
            }
            let bounds = union(row_rects).and_then(|rect| self.doc.viewport_bounds(rect));
            self.nodes.push(node(
                row_id,
                Some(id),
                cell_ids,
                DocumentA11yRole::Row,
                None,
                bounds,
            ));
            row_ids.push(row_id);
        }
        let mut entry = node(
            id,
            Some(parent),
            row_ids,
            DocumentA11yRole::Table,
            None,
            self.doc.viewport_bounds(rendered.bounds),
        );
        entry.actions = vec![DocumentA11yAction::ScrollIntoView];
        self.nodes.push(entry);
        id
    }
}

/// How a row's kind reads aloud: "Directory", "Search". Info rows are
/// plain text and read as their label alone.
fn kind_name(kind: MenuItemKind) -> String {
    if kind == MenuItemKind::Info {
        return String::new();
    }
    let name = kind.name();
    let mut chars = name.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// The boxes of a text block's laid-out lines, top to bottom: runs that share
/// a baseline are one line, spanning the block's width.
fn line_boxes(rendered: &RenderedBlock) -> Vec<Rect> {
    let RenderedBlockKind::Text { glyph_runs } = &rendered.kind else {
        return Vec::new();
    };
    let mut lines: Vec<(f32, f32, f32)> = Vec::new();
    for run in glyph_runs {
        let baseline = run.origin.y + run.baseline_y;
        let top = baseline - run.font_size;
        let bottom = baseline + run.font_size * 0.3;
        match lines
            .iter_mut()
            .find(|(line, _, _)| (line - baseline).abs() < 0.5)
        {
            Some((_, line_top, line_bottom)) => {
                *line_top = line_top.min(top);
                *line_bottom = line_bottom.max(bottom);
            },
            None => lines.push((baseline, top, bottom)),
        }
    }
    lines.sort_by(|a, b| a.0.total_cmp(&b.0));
    lines
        .into_iter()
        .map(|(_, top, bottom)| {
            Rect::from_xywh(
                rendered.bounds.origin.x,
                top,
                rendered.bounds.size.width,
                bottom - top,
            )
        })
        .collect()
}

#[cfg(all(test, feature = "smolweb"))]
mod tests {
    use inker::session_engine::{DocumentSession, SessionClick};
    use inker::{
        DocumentA11yAction, DocumentA11yActionRequest, DocumentA11yNode, DocumentA11yProjection,
        DocumentA11yRole, InlineSpan,
    };

    use crate::{SmolwebDocument, SmolwebDocumentSession, SmolwebTheme};

    const GUIDE: &str = "# Field guide\n\
        Read the intro first.\n\
        => gemini://x.test/next Next page\n\
        => gemini://x.test/other Other page\n\
        * first item\n\
        * second item\n\
        > a quoted line\n";

    fn session(url: &str, body: &str, size: (u32, u32)) -> SmolwebDocumentSession {
        SmolwebDocumentSession::new(SmolwebDocument::parse(url, body, SmolwebTheme::Plain), size)
    }

    fn framed(url: &str, body: &str, size: (u32, u32)) -> SmolwebDocumentSession {
        let mut session = session(url, body, size);
        let _ = session.frame(size.0, size.1);
        session
    }

    /// Names in reading order, depth first from the root.
    fn reading_order(projection: &DocumentA11yProjection) -> Vec<String> {
        fn walk(
            projection: &DocumentA11yProjection,
            node: &DocumentA11yNode,
            out: &mut Vec<String>,
        ) {
            if let Some(name) = &node.name {
                out.push(name.clone());
            }
            for child in &node.children {
                walk(
                    projection,
                    projection.node(*child).expect("child exists"),
                    out,
                );
            }
        }
        let mut out = Vec::new();
        walk(
            projection,
            projection.node(projection.root()).expect("root"),
            &mut out,
        );
        out
    }

    fn named<'a>(projection: &'a DocumentA11yProjection, name: &str) -> &'a DocumentA11yNode {
        projection
            .nodes()
            .iter()
            .find(|node| node.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("no node named {name:?}"))
    }

    fn parent_role(
        projection: &DocumentA11yProjection,
        node: &DocumentA11yNode,
    ) -> DocumentA11yRole {
        projection
            .node(node.parent.expect("has a parent"))
            .expect("parent exists")
            .role
    }

    #[test]
    fn nothing_is_published_before_a_frame() {
        let session = session("gemini://x.test/guide.gmi", GUIDE, (400, 600));
        assert!(session.accessibility_projection().is_none());
    }

    #[test]
    fn a_gemini_page_reads_in_order_with_roles_and_boxes() {
        let session = framed("gemini://x.test/guide.gmi", GUIDE, (400, 600));
        let projection = session
            .accessibility_projection()
            .expect("a framed page projects");
        let root = projection.node(projection.root()).expect("root");
        assert_eq!(root.role, DocumentA11yRole::Document);
        assert_eq!(
            reading_order(&projection)[1..],
            [
                "Field guide",
                "Read the intro first.",
                "Next page",
                "Other page",
                "first item",
                "second item",
                "a quoted line",
            ]
        );

        assert_eq!(
            root.name.as_deref(),
            Some("Field guide"),
            "the page's title"
        );
        let heading = projection
            .nodes()
            .iter()
            .find(|node| node.role == DocumentA11yRole::Heading { level: 1 })
            .expect("the heading");
        assert_eq!(heading.name.as_deref(), Some("Field guide"));
        let link = named(&projection, "Next page");
        assert_eq!(link.role, DocumentA11yRole::Link);
        assert_eq!(
            link.actions,
            [
                DocumentA11yAction::Click,
                DocumentA11yAction::Focus,
                DocumentA11yAction::ScrollIntoView
            ]
        );
        let item = named(&projection, "first item");
        let list_item = projection.node(item.parent.unwrap()).unwrap();
        assert_eq!(list_item.role, DocumentA11yRole::ListItem);
        assert_eq!(parent_role(&projection, list_item), DocumentA11yRole::List);

        for node in projection.nodes() {
            let bounds = node
                .bounds
                .unwrap_or_else(|| panic!("{:?} {:?} has no box", node.role, node.name));
            assert!(bounds.width > 0.0 && bounds.height > 0.0);
        }
        let heading_box = heading.bounds.unwrap();
        let link_box = link.bounds.unwrap();
        assert!(heading_box.y < link_box.y, "boxes follow the page");
    }

    /// A link inside a sentence is its own node between the text around it,
    /// so a reader hears the sentence once and the link once.
    #[test]
    fn an_inline_link_is_read_once_between_its_text() {
        let mut doc = SmolwebDocument::parse("gemini://x.test/a.gmi", "x", SmolwebTheme::Plain);
        let mut document = doc.document().clone();
        document.blocks = vec![inker::Block::Paragraph {
            spans: vec![
                InlineSpan::Text("Read the ".into()),
                InlineSpan::Link {
                    url: "gemini://x.test/guide".into(),
                    title: None,
                    spans: vec![InlineSpan::Text("guide".into())],
                    predicate: None,
                },
                InlineSpan::Text(" first.".into()),
            ],
        }];
        doc.replace_document(document);
        let mut session = SmolwebDocumentSession::new(doc, (400, 300));
        let _ = session.frame(400, 300);
        let projection = session.accessibility_projection().expect("projects");
        assert_eq!(
            reading_order(&projection)[1..],
            ["Read the", "guide", "first."]
        );
        let paragraph = projection
            .nodes()
            .iter()
            .find(|node| node.role == DocumentA11yRole::Paragraph)
            .expect("the paragraph");
        assert_eq!(
            paragraph.name, None,
            "the paragraph does not repeat its text"
        );
        assert_eq!(paragraph.children.len(), 3);
    }

    /// Click goes through the session's own pointer path; Focus moves the
    /// session's keyboard focus; ids outlive a redraw.
    #[test]
    fn click_and_focus_act_on_the_page() {
        let mut session = framed("gemini://x.test/guide.gmi", GUIDE, (400, 600));
        let projection = session.accessibility_projection().unwrap();
        let next = named(&projection, "Next page").id;
        let other = named(&projection, "Other page").id;

        let target = session
            .accessibility_click_target(next)
            .expect("a visible link has a click point");
        assert_eq!(target.revision, projection.revision());
        assert_eq!(
            session.click_at(target.point.x, target.point.y),
            SessionClick::Navigate("gemini://x.test/next".into())
        );

        assert!(
            session.dispatch_accessibility_action(&DocumentA11yActionRequest {
                revision: projection.revision(),
                target: other,
                action: DocumentA11yAction::Focus,
                data: None,
            })
        );
        let _ = session.frame(400, 600);
        let focused = session.accessibility_projection().unwrap();
        assert!(focused.node(other).unwrap().state.focused);
        assert!(!focused.node(next).unwrap().state.focused);
        assert_ne!(
            focused.revision(),
            projection.revision(),
            "focus moves the revision"
        );
        assert_eq!(
            focused.node(next).unwrap().name.as_deref(),
            Some("Next page")
        );

        assert!(
            !session.dispatch_accessibility_action(&DocumentA11yActionRequest {
                revision: focused.revision(),
                target: next,
                action: DocumentA11yAction::Click,
                data: None,
            }),
            "Click travels by the pointer path, not dispatch"
        );
    }

    /// A node below the fold keeps its true place; ScrollIntoView brings it
    /// into the viewport; an action against the old revision is refused.
    #[test]
    fn scroll_into_view_reaches_the_bottom_and_stale_actions_are_refused() {
        let body: String = (0..60)
            .map(|line| format!("Line {line} of a long page.\n"))
            .collect();
        let body = format!("{body}=> gemini://x.test/end The end\n");
        let mut session = framed("gemini://x.test/long.gmi", &body, (400, 200));
        let projection = session.accessibility_projection().unwrap();
        let end = named(&projection, "The end");
        assert!(end.bounds.unwrap().y > 200.0, "below the fold, unclipped");
        assert!(
            session.accessibility_click_target(end.id).is_none(),
            "nothing to click until it is in view"
        );

        let request = DocumentA11yActionRequest {
            revision: projection.revision(),
            target: end.id,
            action: DocumentA11yAction::ScrollIntoView,
            data: None,
        };
        assert!(session.dispatch_accessibility_action(&request));
        let _ = session.frame(400, 200);
        let scrolled = session.accessibility_projection().unwrap();
        let end_box = scrolled.node(end.id).unwrap().bounds.unwrap();
        assert!(end_box.y >= 0.0 && end_box.y + end_box.height <= 200.0);
        assert!(session.accessibility_click_target(end.id).is_some());
        assert!(
            !session.dispatch_accessibility_action(&request),
            "the request named the revision before the scroll"
        );
    }

    /// A gopher menu reads one row per item, each row naming its kind, with
    /// its own line's box; a search row is a button.
    #[test]
    fn a_gopher_menu_reads_row_by_row() {
        let menu = "iWelcome to the hole\t\terror.host\t1\r\n\
            1Phlog\t/phlog\tx.test\t70\r\n\
            0About this server\t/about.txt\tx.test\t70\r\n\
            7Search the hole\t/search\tx.test\t70\r\n.\r\n";
        let session = framed("gopher://x.test/1/", menu, (500, 300));
        let projection = session.accessibility_projection().unwrap();
        let list = projection
            .nodes()
            .iter()
            .find(|node| node.role == DocumentA11yRole::List)
            .expect("the menu is a list");
        assert_eq!(list.children.len(), 4);

        let info = projection.node(list.children[0]).unwrap();
        assert_eq!(info.name.as_deref(), Some("Welcome to the hole"));
        let phlog = named(&projection, "Phlog");
        assert_eq!(phlog.role, DocumentA11yRole::Link);
        assert_eq!(phlog.description.as_deref(), Some("Directory"));
        assert_eq!(
            named(&projection, "About this server")
                .description
                .as_deref(),
            Some("Document")
        );
        let search = named(&projection, "Search the hole");
        assert_eq!(search.role, DocumentA11yRole::Button);

        let rows: Vec<f32> = list
            .children
            .iter()
            .map(|row| projection.node(*row).unwrap().bounds.unwrap().y)
            .collect();
        assert!(
            rows.windows(2).all(|pair| pair[0] < pair[1]),
            "each row has its own line: {rows:?}"
        );
    }

    fn nomadnet(file: &str, source: &str) -> SmolwebDocumentSession {
        use inker::{Engine, EngineInput};

        let address = format!("abb3ebcd03cb2388a838e70c001291f9:/page/{file}");
        let document = nematic::MicronEngine::new()
            .render(&EngineInput::new(address, source))
            .expect("the probe page lowers");
        let document = SmolwebDocument::from_document_with_theme(document, SmolwebTheme::Plain);
        let mut session = SmolwebDocumentSession::new(document, (640, 2400));
        let _ = session.frame(640, 2400);
        session
    }

    /// Real NomadNet pages, the guide's probes from NomadNet 1.4.2: the
    /// structure page reads with its section headings, the links-and-fields
    /// page with working links, and every node is boxed. Fields stay inert
    /// source text here: the live form belongs to the host's form surface.
    #[test]
    fn nomadnet_pages_read_with_headings_and_links() {
        let structure = nomadnet(
            "guide-structure.mu",
            include_str!(
                "../../../../nematic/nematic/tests/fixtures/micron/nomadnet-1.4.2/guide-structure.mu"
            ),
        );
        let projection = structure.accessibility_projection().expect("projects");
        assert!(
            projection
                .nodes()
                .iter()
                .any(|node| matches!(node.role, DocumentA11yRole::Heading { .. })),
            "the page's sections read as headings"
        );
        for node in projection.nodes() {
            assert!(
                node.bounds.is_some(),
                "{:?} {:?} has a box",
                node.role,
                node.name
            );
        }

        let mut links = nomadnet(
            "guide-links-fields.mu",
            include_str!(
                "../../../../nematic/nematic/tests/fixtures/micron/nomadnet-1.4.2/guide-links-fields.mu"
            ),
        );
        let projection = links.accessibility_projection().expect("projects");
        let link = projection
            .nodes()
            .iter()
            .find(|node| {
                node.role == DocumentA11yRole::Link
                    && links.accessibility_click_target(node.id).is_some()
            })
            .expect("a visible link with a click point");
        let target = links.accessibility_click_target(link.id).unwrap();
        assert!(
            matches!(
                links.click_at(target.point.x, target.point.y),
                SessionClick::Navigate(_) | SessionClick::Handled
            ),
            "the click point lands on that link"
        );
        for node in projection.nodes() {
            assert!(
                node.bounds.is_some(),
                "{:?} {:?} has a box",
                node.role,
                node.name
            );
        }
    }
}
