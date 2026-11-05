// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! In-document navigation over parsed Micron, in syntax line space: fold
//! extents, anchor declarations and the next-heading jump. No IO, no Inker.
//!
//! Rules come from the stock NomadNet 1.4.2 navigation receipt
//! (`tests/fixtures/micron/nomadnet-1.4.2/navigation/NAVIGATION_RECEIPT.md`)
//! and the Micron navigation plan's N1 phase.

use std::ops::Range;

use super::syntax::{self, Document, Line, LineKind, Span};

/// A heading slug or an explicit `` `:name ``, in source order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnchorDeclaration {
    pub name: String,
    /// Index into [`Document::lines`].
    pub line: usize,
    /// False when an earlier declaration already bound the name.
    pub active: bool,
}

/// A collapsible named heading and the lines it hides while closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fold {
    /// Index into [`Document::lines`].
    pub line: usize,
    pub initially_open: bool,
    /// Half-open; starts at `line + 1` and may be empty.
    pub lines: Range<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Navigation {
    pub anchors: Vec<AnchorDeclaration>,
    /// Source order. Extents nest; none partially overlap.
    pub folds: Vec<Fold>,
    named_headings: Vec<usize>,
}

impl Navigation {
    pub fn new(document: &Document) -> Self {
        let mut navigation = Self::default();
        // Folds still open, with their heading depth.
        let mut open: Vec<(usize, usize)> = Vec::new();
        for (index, line) in document.lines.iter().enumerate() {
            let named = is_named_heading(line);
            let less_than = is_less_than_led(line);
            let heading_depth = match line.kind {
                LineKind::Heading { depth, .. } => Some(depth),
                _ => None,
            };
            open.retain(|&(fold, depth)| {
                let stops = less_than
                    || heading_depth.is_some_and(|d| if named { d <= depth } else { d < depth });
                if stops {
                    navigation.folds[fold].lines.end = index;
                }
                !stops
            });
            if let LineKind::Heading {
                depth,
                initially_open,
                anchor,
            } = &line.kind
                && named
            {
                navigation.named_headings.push(index);
                if let Some(slug) = syntax::heading_anchor(&line.spans) {
                    navigation.anchors.push(AnchorDeclaration {
                        name: slug,
                        line: index,
                        active: anchor.is_some(),
                    });
                }
                if let Some(initially_open) = *initially_open {
                    open.push((navigation.folds.len(), *depth));
                    navigation.folds.push(Fold {
                        line: index,
                        initially_open,
                        lines: index + 1..document.lines.len(),
                    });
                }
            }
            for span in &line.spans {
                if let Span::Anchor { name, active } = span {
                    navigation.anchors.push(AnchorDeclaration {
                        name: name.clone(),
                        line: index,
                        active: *active,
                    });
                }
            }
        }
        navigation
    }

    /// The winning (first) declaration of `name`.
    pub fn resolve_anchor(&self, name: &str) -> Option<&AnchorDeclaration> {
        self.anchors
            .iter()
            .find(|anchor| anchor.active && anchor.name == name)
    }

    /// The first named heading strictly after `from_line` (decision 6).
    pub fn next_heading(&self, from_line: usize) -> Option<usize> {
        let next = self
            .named_headings
            .partition_point(|&line| line <= from_line);
        self.named_headings.get(next).copied()
    }
}

/// A heading with visible content. A bare `>` run is an unnamed section: it
/// is neither a fold nor a next-heading target.
pub fn is_named_heading(line: &Line) -> bool {
    matches!(line.kind, LineKind::Heading { .. })
        && line
            .spans
            .iter()
            .any(|span| !matches!(span, Span::Anchor { .. }))
}

/// `<`, `<<` or `< text`. It ends every open fold before its own line and
/// otherwise keeps its source (decision 5).
pub fn is_less_than_led(line: &Line) -> bool {
    line.kind == LineKind::Text && line.source.starts_with('<')
}

#[cfg(test)]
#[path = "navigation_tests.rs"]
mod tests;
