// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! In-page navigation facts an engine lowers beside its blocks: anchor
//! declarations and collapsible extents, keyed by top-level block index.
//! Kept off the blocks because they relate blocks to each other, like
//! [`BlockProvenanceMap`](super::BlockProvenanceMap). [`FoldState`] is the
//! reader's session-only view of those extents, and [`FoldMarkers`] the style
//! token every renderer shows that state with.

use std::collections::HashMap;
use std::ops::Range;

use serde::{Deserialize, Serialize};

use super::Block;

/// Indices are positions in [`EngineDocument::blocks`](super::EngineDocument::blocks),
/// valid only while [`is_current`](Self::is_current) holds. A transform that
/// inserts or removes top-level blocks must remap or clear the table.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentNavigation {
    /// `blocks.len()` when the table was computed.
    pub block_count: usize,
    /// Every declaration in source order, duplicates included.
    pub anchors: Vec<DocumentAnchor>,
    /// Collapsible headings in source order. Extents nest; none partially overlap.
    pub folds: Vec<DocumentFold>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentAnchor {
    pub name: String,
    /// First block at or after the declaration; `None` when nothing follows.
    pub block: Option<usize>,
    /// Engine line identity (Micron: index into its syntax lines).
    pub source_line: usize,
    /// False when an earlier declaration already bound the name.
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentFold {
    /// Block of the collapsible heading, possibly `Presented`-wrapped.
    pub heading: usize,
    /// Engine line identity. Shifts when lines are inserted above, so it is
    /// not a fold-state key; see [`FoldKey`].
    pub source_line: usize,
    /// The heading's source spelling as the engine read it; with its
    /// occurrence among the folds, the session fold-state key.
    #[serde(default)]
    pub source_text: String,
    pub initially_open: bool,
    /// Half-open block range hidden while closed; starts at `heading + 1`.
    pub extent: Range<usize>,
}

/// Where an [`InlineSpan::InPage`](super::InlineSpan::InPage) link lands.
/// Its index shares the document table's [`is_current`](DocumentNavigation::is_current)
/// guard, so blocks copied into another document stay inert.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InPageTarget {
    /// A declared name that resolves to `block`, for a host's address.
    pub fragment: Option<String>,
    /// `None` is an inert no-op: a missing anchor, or no later heading.
    pub block: Option<usize>,
}

impl DocumentNavigation {
    /// Whether the indices still describe `blocks`.
    pub fn is_current(&self, blocks: &[Block]) -> bool {
        blocks.len() == self.block_count
    }

    /// Block of the first active declaration of `name`.
    pub fn resolve(&self, name: &str) -> Option<usize> {
        self.anchors
            .iter()
            .find(|anchor| anchor.active && anchor.name == name)
            .and_then(|anchor| anchor.block)
    }

    /// Folds whose extent hides `block`, outermost first.
    pub fn folds_containing(&self, block: usize) -> impl Iterator<Item = &DocumentFold> {
        self.folds
            .iter()
            .filter(move |fold| fold.extent.contains(&block))
    }

    /// The session key of each fold, index for index.
    pub fn fold_keys(&self) -> Vec<FoldKey> {
        let mut seen: HashMap<&str, usize> = HashMap::new();
        self.folds
            .iter()
            .map(|fold| {
                let count = seen.entry(fold.source_text.as_str()).or_default();
                let key = FoldKey {
                    source_text: fold.source_text.clone(),
                    occurrence: *count,
                };
                *count += 1;
                key
            })
            .collect()
    }
}

/// A fold's identity across re-lowering: its heading's source spelling and
/// which occurrence of that spelling it is (plan decisions 10 and 11). Any
/// edit to the heading line, its `+`/`-` marker included, is a new key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FoldKey {
    pub source_text: String,
    pub occurrence: usize,
}

/// Session-only reader overrides of authored fold state (plan decision 2).
/// Methods take the navigation table they index; callers check
/// [`DocumentNavigation::is_current`] first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FoldState {
    open: HashMap<FoldKey, bool>,
}

impl FoldState {
    /// Drop overrides for headings `navigation` no longer has (an edited
    /// heading). Streamed prefixes only add keys, so arrived state survives.
    pub fn reconcile(&mut self, navigation: &DocumentNavigation) {
        let keys = navigation.fold_keys();
        self.open.retain(|key, _| keys.contains(key));
    }

    pub fn is_open(&self, navigation: &DocumentNavigation, fold: usize) -> bool {
        self.open_with(navigation, &navigation.fold_keys(), fold)
    }

    /// Flip `fold`; returns its new state.
    pub fn toggle(&mut self, navigation: &DocumentNavigation, fold: usize) -> bool {
        let keys = navigation.fold_keys();
        let open = !self.open_with(navigation, &keys, fold);
        self.open.insert(keys[fold].clone(), open);
        open
    }

    /// Open every closed fold whose extent hides `block`; true when any was.
    pub fn open_ancestors(&mut self, navigation: &DocumentNavigation, block: usize) -> bool {
        let keys = navigation.fold_keys();
        let mut changed = false;
        for (fold, entry) in navigation.folds.iter().enumerate() {
            if entry.extent.contains(&block) && !self.open_with(navigation, &keys, fold) {
                self.open.insert(keys[fold].clone(), true);
                changed = true;
            }
        }
        changed
    }

    /// Closed extents in ascending block order, nested ones merged. A fold
    /// inside a closed fold keeps its own state for when the outer opens.
    pub fn hidden(&self, navigation: &DocumentNavigation) -> Vec<Range<usize>> {
        let keys = navigation.fold_keys();
        let mut hidden: Vec<Range<usize>> = Vec::new();
        for (fold, entry) in navigation.folds.iter().enumerate() {
            let covered = hidden
                .last()
                .is_some_and(|last| last.end >= entry.extent.end);
            if !entry.extent.is_empty() && !covered && !self.open_with(navigation, &keys, fold) {
                hidden.push(entry.extent.clone());
            }
        }
        hidden
    }

    fn open_with(&self, navigation: &DocumentNavigation, keys: &[FoldKey], fold: usize) -> bool {
        self.open
            .get(&keys[fold])
            .copied()
            .unwrap_or(navigation.folds[fold].initially_open)
    }
}

/// Glyphs laid out before a collapsible heading's text, styled as that text
/// and inside its toggle region. The shared style token for fold state (plan
/// decision 17): document-canvas carries one on its style sheet, and a
/// renderer without that sheet, such as Knot's preview, reads the same
/// default here. The defaults are stock NomadNet's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoldMarkers {
    pub open: String,
    pub closed: String,
}

impl FoldMarkers {
    pub fn marker(&self, open: bool) -> &str {
        if open { &self.open } else { &self.closed }
    }
}

impl Default for FoldMarkers {
    fn default() -> Self {
        Self {
            open: "\u{25be} ".into(),
            closed: "\u{25b8} ".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> DocumentNavigation {
        DocumentNavigation {
            block_count: 4,
            anchors: vec![
                DocumentAnchor {
                    name: "a".into(),
                    block: Some(2),
                    source_line: 3,
                    active: true,
                },
                DocumentAnchor {
                    name: "a".into(),
                    block: Some(3),
                    source_line: 5,
                    active: false,
                },
                DocumentAnchor {
                    name: "tail".into(),
                    block: None,
                    source_line: 9,
                    active: true,
                },
            ],
            folds: vec![
                DocumentFold {
                    heading: 0,
                    source_line: 1,
                    source_text: "`->Outer".into(),
                    initially_open: false,
                    extent: 1..4,
                },
                DocumentFold {
                    heading: 1,
                    source_line: 2,
                    source_text: "`+>>Inner".into(),
                    initially_open: true,
                    extent: 2..3,
                },
            ],
        }
    }

    #[test]
    fn resolve_uses_the_first_active_declaration() {
        let navigation = table();
        assert_eq!(navigation.resolve("a"), Some(2));
        assert_eq!(navigation.resolve("tail"), None);
        assert_eq!(navigation.resolve("missing"), None);
    }

    #[test]
    fn folds_containing_lists_every_enclosing_fold() {
        let navigation = table();
        let headings = |block| {
            navigation
                .folds_containing(block)
                .map(|fold| fold.heading)
                .collect::<Vec<_>>()
        };
        assert_eq!(headings(2), [0, 1]);
        assert_eq!(headings(3), [0]);
        assert!(
            headings(0).is_empty(),
            "a heading is outside its own extent"
        );
    }

    #[test]
    fn fold_keys_count_occurrences_of_the_same_source_text() {
        let mut navigation = table();
        navigation.folds[1].source_text = navigation.folds[0].source_text.clone();
        let keys = navigation.fold_keys();
        assert_eq!(
            keys.iter().map(|key| key.occurrence).collect::<Vec<_>>(),
            [0, 1]
        );
        assert_ne!(keys[0], keys[1]);
    }

    #[test]
    fn toggle_overrides_authored_state_and_hidden_merges_nested_extents() {
        let navigation = table();
        let mut state = FoldState::default();
        assert_eq!(state.hidden(&navigation), [1..4], "authored: outer closed");
        assert!(!state.toggle(&navigation, 1), "inner closes");
        assert_eq!(
            state.hidden(&navigation),
            [1..4],
            "the closed inner lies inside the closed outer"
        );
        assert!(state.toggle(&navigation, 0), "outer opens");
        assert_eq!(
            state.hidden(&navigation),
            [2..3],
            "the inner kept its state"
        );
    }

    #[test]
    fn open_ancestors_opens_only_closed_enclosing_folds() {
        let navigation = table();
        let mut state = FoldState::default();
        state.toggle(&navigation, 1);
        assert!(state.open_ancestors(&navigation, 2));
        assert!(state.hidden(&navigation).is_empty());
        assert!(
            !state.open_ancestors(&navigation, 2),
            "nothing left to open"
        );
        assert!(
            !state.open_ancestors(&navigation, 0),
            "a heading has no ancestors here"
        );
    }

    #[test]
    fn reconcile_drops_state_for_a_heading_whose_source_changed() {
        let mut navigation = table();
        let mut state = FoldState::default();
        state.toggle(&navigation, 0);
        state.toggle(&navigation, 1);
        navigation.folds[1].source_text = "`->>Inner".into();
        state.reconcile(&navigation);
        assert!(
            state.is_open(&navigation, 0),
            "untouched heading keeps state"
        );
        assert!(
            state.is_open(&navigation, 1),
            "the edited heading is back to its authored open state"
        );
    }

    #[test]
    fn a_spliced_block_list_is_not_current() {
        let navigation = table();
        let mut blocks = vec![Block::Rule; 4];
        assert!(navigation.is_current(&blocks));
        blocks.insert(0, Block::Rule);
        assert!(!navigation.is_current(&blocks));
    }

    #[test]
    fn fold_markers_pick_by_state_and_default_to_distinct_glyphs() {
        let markers = FoldMarkers::default();
        assert_eq!(markers.marker(true), markers.open);
        assert_eq!(markers.marker(false), markers.closed);
        assert!(!markers.open.is_empty() && !markers.closed.is_empty());
        assert_ne!(markers.open, markers.closed);
    }
}
