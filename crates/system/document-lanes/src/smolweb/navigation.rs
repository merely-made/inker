/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

impl SmolwebDocument {
    /// The reader's fold state for this document.
    pub fn folds(&self) -> &FoldState {
        &self.folds
    }

    pub(super) fn navigation_is_current(&self) -> bool {
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

    pub(super) fn scroll_to_block(&mut self, block: usize) {
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
    pub(super) fn focus_stops(&self) -> Vec<(Focus, Vec<usize>)> {
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

    pub(super) fn focused_regions(&self) -> Option<Vec<usize>> {
        let focus = self.focus.as_ref()?;
        self.focus_stops()
            .into_iter()
            .find(|(stop, _)| stop == focus)
            .map(|(_, regions)| regions)
    }

    pub(super) fn scroll_into_view(&mut self, regions: &[usize]) {
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
    pub(super) fn paint_focus(&self, scene: &mut Scene) {
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
    pub(super) fn viewport_rect(&self, bounds: Rect) -> Option<[f32; 4]> {
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
