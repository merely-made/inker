// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

impl Builder<'_> {
    pub(super) fn regions_bounds(&self, regions: &[usize]) -> Option<Rect> {
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
    pub(super) fn take(&mut self, within: Rect, wanted: impl Fn(&Focus) -> bool) -> Option<Stop> {
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
    pub(super) fn next_stop(&mut self, within: Rect, submit: bool) -> Option<Stop> {
        self.take(within, |focus| match focus {
            Focus::Submit(..) => submit,
            Focus::Link(_) => !submit,
            Focus::Fold(_) => false,
        })
    }

    /// The collapsible-heading stop inside `within`, and whether it is open.
    pub(super) fn fold_stop(&mut self, within: Rect) -> Option<(Stop, bool)> {
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
    pub(super) fn stops_within(&mut self, within: Rect) -> Vec<Stop> {
        let mut out = Vec::new();
        while let Some(stop) = self.take(within, |focus| !matches!(focus, Focus::Fold(_))) {
            out.push(stop);
        }
        out
    }

    pub(super) fn stop_node(
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

    pub(super) fn text_node(
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
    pub(super) fn inline_block(
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
    pub(super) fn container(
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
    pub(super) fn text_block(
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

    pub(super) fn block(
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

    pub(super) fn list(
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
    pub(super) fn menu(
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
    pub(super) fn table(
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
