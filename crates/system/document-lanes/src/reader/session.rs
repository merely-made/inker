/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

impl ReaderDocumentSession {
    pub fn reading_anchor(&self) -> Option<crate::ReadingAnchor> {
        self.doc.reading_anchor()
    }
    pub fn restore_reading_anchor(&mut self, anchor: crate::ReadingAnchor) {
        self.doc.restore_reading_anchor(anchor);
        self.bump_accessibility_revision();
    }

    pub(super) fn from_source(source: Arc<ReaderDocumentSource>, viewport: (u32, u32)) -> Self {
        let doc = SmolwebDocument::from_shared_document_with_theme(
            source.document.clone(),
            source.theme.clone(),
        );
        let lineage = source.lineage.clone();
        Self {
            source,
            doc,
            viewport,
            lineage,
            accessibility_revision: 1,
            focused_semantic: None,
            accessibility_nodes: RefCell::new(ReaderAccessibilityNodeMap::default()),
        }
    }

    /// Create an independently scrollable and laid-out presentation of this
    /// document. The source packet is shared; all viewport-dependent state is
    /// fresh for the new appearance.
    pub fn new_appearance(&self, viewport: (u32, u32)) -> Self {
        let mut appearance = Self::from_source(self.source.clone(), viewport);
        appearance.doc = self.doc.new_appearance();
        appearance
    }

    /// Identity receipt for hosts which need to prove two appearances share
    /// document content without reaching into private parser state.
    pub fn source_document(&self) -> Arc<EngineDocument> {
        self.source.document.clone()
    }

    /// The last viewport supplied by the host frame.
    ///
    /// This is an observation only: it neither lays out the document nor
    /// changes this appearance's retained presentation.
    pub fn viewport(&self) -> (u32, u32) {
        self.viewport
    }

    /// Current vertical viewport offset for this appearance, in document
    /// pixels. Each appearance owns this value even when its source document
    /// packet is shared with another appearance.
    pub fn scroll_y(&self) -> f32 {
        self.doc.scroll_y()
    }

    pub fn reading_preferences(&self) -> crate::ReadingPreferences {
        self.doc.reading_preferences()
    }
    pub fn set_reading_preferences(
        &mut self,
        value: crate::ReadingPreferences,
    ) -> Result<bool, String> {
        self.doc.set_reading_preferences(value)
    }
    pub fn color_mode(&self) -> crate::ModeProfile {
        self.doc.color_mode()
    }
    pub fn set_color_mode(&mut self, mode: crate::ModeProfile) -> bool {
        self.doc.set_color_mode(mode)
    }
    pub fn set_font_family(&mut self, family: &str) {
        self.doc.set_font_family(family);
    }
    pub fn set_fonts(&mut self, fonts: Vec<Vec<u8>>) {
        self.doc.set_fonts(fonts);
    }
    /// Host-held generic faces used by this Reader appearance, including code.
    pub fn set_generic_fonts(
        &mut self,
        fonts: Vec<(String, Vec<u8>)>,
    ) -> Result<bool, document_canvas::text::GenericFontError> {
        self.doc.set_generic_fonts(fonts)
    }
    pub fn set_segmenter_dictionaries(
        &mut self,
        dictionaries: Vec<document_canvas::DictionaryBlob>,
    ) {
        self.doc.set_segmenter_dictionaries(dictionaries);
    }
    pub fn wanted_segmenter_dictionaries(&self) -> document_canvas::SegmenterDictionaries {
        self.doc.wanted_segmenter_dictionaries()
    }

    pub fn document(&self) -> &EngineDocument {
        self.doc.document()
    }

    pub fn lineage(&self) -> &ExtractionLineage {
        &self.lineage
    }

    /// Return the current renderer-neutral Reader semantic snapshot without
    /// forcing layout. It is absent before Reader has completed its first
    /// frame, then reflects the retained layout and current scroll position.
    pub fn accessibility_snapshot(&self) -> Option<ReaderAccessibilitySnapshot> {
        let links = self.doc.retained_accessible_links()?;
        Some(ReaderAccessibilitySnapshot {
            root_title: self.doc.document().title.clone(),
            links: links
                .into_iter()
                .map(|link| ReaderAccessibilityLink {
                    identity: link.identity,
                    label: link.label,
                    url: link.url,
                    rects: link.rects,
                })
                .collect(),
        })
    }

    pub(super) fn accessibility_node_id(
        &self,
        identity: document_canvas::SemanticInteractionId,
    ) -> DocumentA11yNodeId {
        let mut nodes = self.accessibility_nodes.borrow_mut();
        if let Some(id) = nodes.ids.get(&identity) {
            return *id;
        }
        // Reserve 1 for the document root. Never recycle a local identity:
        // a queued virtual Focus may then only become stale, never point at a
        // later visible link after reflow.
        let next = nodes.next.max(2);
        nodes.next = next
            .checked_add(1)
            .expect("Reader accessibility node IDs exhausted");
        let id = DocumentA11yNodeId::new(next);
        nodes.ids.insert(identity, id);
        id
    }

    pub(super) fn bump_accessibility_revision(&mut self) {
        self.accessibility_revision = self
            .accessibility_revision
            .checked_add(1)
            .expect("Reader accessibility revision exhausted");
    }

    /// Project the completed document-canvas presentation into Inker's
    /// renderer-neutral semantic contract. Reader currently exposes visible
    /// links only; Focus is intentionally virtual and activation remains
    /// unavailable until a host-held destination-body handoff exists.
    pub(super) fn document_accessibility_projection(&self) -> Option<DocumentA11yProjection> {
        let snapshot = self.accessibility_snapshot()?;
        let root = DocumentA11yNodeId::new(1);
        let title = snapshot
            .root_title
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| "Reader document".to_owned());
        let mut links = Vec::new();
        for link in snapshot.links {
            let Some(bounds) = reader_link_bounds(&link.rects) else {
                continue;
            };
            let id = self.accessibility_node_id(link.identity);
            links.push(DocumentA11yNode {
                id,
                parent: Some(root),
                children: Vec::new(),
                role: DocumentA11yRole::Link,
                description: None,
                name: Some(link.label),
                value: None,
                numeric_value: None,
                numeric_minimum: None,
                numeric_maximum: None,
                bounds: Some(bounds),
                state: DocumentA11yState {
                    focused: self.focused_semantic == Some(id),
                    ..Default::default()
                },
                actions: vec![DocumentA11yAction::Focus],
            });
        }
        let mut nodes = vec![DocumentA11yNode {
            id: root,
            parent: None,
            children: links.iter().map(|link| link.id).collect(),
            role: DocumentA11yRole::Document,
            description: None,
            name: Some(title),
            value: None,
            numeric_value: None,
            numeric_minimum: None,
            numeric_maximum: None,
            bounds: None,
            state: DocumentA11yState::default(),
            actions: Vec::new(),
        }];
        nodes.extend(links);
        let support = DocumentA11ySupport::new(
            inker::A11yCapability::Partial,
            ["Visible Reader links are composed with virtual Focus; activation remains unavailable until the host holds the destination body."],
        )
        .expect("Reader's partial accessibility limitation is non-empty");
        Some(DocumentA11yProjection::new(
            self.accessibility_revision,
            support,
            root,
            nodes,
        ))
    }

    /// Re-resolve a current visible point for a Reader semantic link token.
    /// The query reads retained geometry only, validates against current scroll,
    /// and returns no point after that link has left the content hole.
    pub fn accessibility_pointer_target(
        &self,
        identity: document_canvas::SemanticInteractionId,
    ) -> Option<(f32, f32)> {
        self.doc.retained_accessible_pointer_target(identity)
    }
}
