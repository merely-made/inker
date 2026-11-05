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
#[path = "a11y/tests.rs"]
mod tests;

#[path = "a11y/building.rs"]
mod building;
