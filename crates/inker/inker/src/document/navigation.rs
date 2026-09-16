// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! In-page navigation facts an engine lowers beside its blocks: anchor
//! declarations and collapsible extents, keyed by top-level block index.
//! Kept off the blocks because they relate blocks to each other, like
//! [`BlockProvenanceMap`](super::BlockProvenanceMap).

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
    /// Engine line identity; with the document identity, the fold-state key.
    pub source_line: usize,
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
                    initially_open: false,
                    extent: 1..4,
                },
                DocumentFold {
                    heading: 1,
                    source_line: 2,
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
    fn a_spliced_block_list_is_not_current() {
        let navigation = table();
        let mut blocks = vec![Block::Rule; 4];
        assert!(navigation.is_current(&blocks));
        blocks.insert(0, Block::Rule);
        assert!(!navigation.is_current(&blocks));
    }
}
