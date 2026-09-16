// Copyright 2026 Mark Alan Boykin
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
mod tests {
    use super::*;

    macro_rules! page {
        ($file:literal) => {
            syntax::parse(include_str!(concat!(
                "../../tests/fixtures/micron/nomadnet-1.4.2/",
                $file
            )))
        };
    }

    /// Visible text of a line, or its source when it has none.
    fn label(document: &Document, line: usize) -> String {
        let Some(line) = document.lines.get(line) else {
            return "<end>".into();
        };
        let text: String = line
            .spans
            .iter()
            .filter_map(|span| match span {
                Span::Text { text, .. } => Some(text.as_str()),
                Span::Link { label, .. } => Some(label.as_str()),
                _ => None,
            })
            .collect();
        if text.is_empty() {
            line.source.clone()
        } else {
            text
        }
    }

    /// (heading line, heading label, initially open, extent, label of the line that ends it)
    fn folds(document: &Document) -> Vec<(usize, String, bool, Range<usize>, String)> {
        Navigation::new(document)
            .folds
            .into_iter()
            .map(|fold| {
                (
                    fold.line,
                    label(document, fold.line),
                    fold.initially_open,
                    fold.lines.clone(),
                    label(document, fold.lines.end),
                )
            })
            .collect()
    }

    /// (name, line, active, label of the line)
    fn anchors(document: &Document) -> Vec<(String, usize, bool, String)> {
        Navigation::new(document)
            .anchors
            .into_iter()
            .map(|anchor| {
                let text = label(document, anchor.line);
                (anchor.name, anchor.line, anchor.active, text)
            })
            .collect()
    }

    fn fold(
        line: usize,
        heading: &str,
        open: bool,
        lines: Range<usize>,
        stop: &str,
    ) -> (usize, String, bool, Range<usize>, String) {
        (line, heading.into(), open, lines, stop.into())
    }

    fn anchor(name: &str, line: usize, active: bool, text: &str) -> (String, usize, bool, String) {
        (name.into(), line, active, text.into())
    }

    fn resolved(document: &Document, name: &str) -> Option<(usize, String)> {
        Navigation::new(document)
            .resolve_anchor(name)
            .map(|anchor| (anchor.line, label(document, anchor.line)))
    }

    fn next(document: &Document, from: usize) -> Option<(usize, String)> {
        Navigation::new(document)
            .next_heading(from)
            .map(|line| (line, label(document, line)))
    }

    #[test]
    fn guide_structure_folds_anchors_and_unnamed_section() {
        let doc = page!("guide-structure.mu");
        assert_eq!(
            folds(&doc),
            vec![
                fold(8, "Open fold", true, 9..10, "Closed fold"),
                fold(10, "Closed fold", false, 11..13, "Table example"),
            ],
            "Guide capture: a fold ends at the next heading of equal depth"
        );
        assert_eq!(
            anchors(&doc),
            vec![
                anchor("top-heading", 1, true, "Top heading"),
                anchor("nested-heading", 4, true, "Nested heading"),
                anchor("fold-examples", 7, true, "Fold examples"),
                anchor("open-fold", 8, true, "Open fold"),
                anchor("closed-fold", 10, true, "Closed fold"),
                anchor("table-example", 13, true, "Table example"),
            ],
            "Guide capture: every named heading declares its slug; the bare >>>> does not"
        );
        assert_eq!(label(&doc, 16), ">>>>");
        assert_eq!(
            next(&doc, 13),
            None,
            "C1b probe 11b: the unnamed >>>> is not a next-heading target"
        );
    }

    #[test]
    fn probe_01_duplicate_heading_slugs_first_wins() {
        let doc = page!("navigation/probe-nav-01-duplicate-heading.mu");
        assert_eq!(
            anchors(&doc),
            vec![
                anchor("nav-probe-one", 64, true, "Nav Probe One"),
                anchor("nav-probe-one", 126, false, "Nav Probe One"),
                anchor("probe-01-end", 188, true, "Probe 01 end"),
            ],
            "probe 01: the second heading's slug is an inactive duplicate"
        );
        assert_eq!(
            resolved(&doc, "nav-probe-one"),
            Some((64, "Nav Probe One".into())),
            "probe 01: #nav-probe-one resolves to the first heading"
        );
    }

    #[test]
    fn probe_02_heading_and_explicit_anchor_first_declaration_wins() {
        let doc = page!("navigation/probe-nav-02a-heading-first.mu");
        assert_eq!(
            anchors(&doc)[..2],
            [
                anchor("same-name", 64, true, "Same Name"),
                anchor("same-name", 126, false, "`:same-name"),
            ],
            "probe 02a: the later explicit anchor is inactive"
        );
        assert_eq!(
            resolved(&doc, "same-name"),
            Some((64, "Same Name".into())),
            "probe 02a: the heading declared first wins"
        );

        let doc = page!("navigation/probe-nav-02b-anchor-first.mu");
        assert_eq!(
            anchors(&doc)[..2],
            [
                anchor("other-name", 64, true, "`:other-name"),
                anchor("other-name", 126, false, "Other Name"),
            ],
            "probe 02b: the later heading slug is inactive"
        );
        assert_eq!(
            resolved(&doc, "other-name"),
            Some((64, "`:other-name".into())),
            "probe 02b: the explicit anchor declared first wins"
        );
    }

    #[test]
    fn probe_03_missing_anchor_resolves_to_nothing() {
        let doc = page!("navigation/probe-nav-03-missing-anchor.mu");
        assert_eq!(
            resolved(&doc, "no-such-anchor-here"),
            None,
            "probe 03: a missing anchor has no declaration"
        );
        assert_eq!(
            resolved(&doc, "present-control"),
            Some((65, "`:present-control".into())),
            "probe 03: the positive control on the same page resolves"
        );
    }

    #[test]
    fn probe_04_next_heading_counts_from_the_link_line() {
        let doc = page!("navigation/probe-nav-04-next-heading-tail.mu");
        assert_eq!(label(&doc, 2), "next heading from the top");
        assert_eq!(
            next(&doc, 2),
            Some((63, "First Heading".into())),
            "probe 04 / decision 6: # from the top link lands on the first heading"
        );
        assert_eq!(
            next(&doc, 63),
            Some((125, "Last Heading".into())),
            "decision 6: a heading line counts strictly after itself"
        );
        assert_eq!(label(&doc, 187), "next heading from the tail");
        assert_eq!(
            next(&doc, 187),
            None,
            "probe 04: # below the last heading has no target"
        );
    }

    #[test]
    fn probe_05_targets_inside_a_closed_fold() {
        let doc = page!("navigation/probe-nav-05-closed-target.mu");
        assert_eq!(
            folds(&doc),
            vec![fold(64, "Closed Outer", false, 65..70, "Sentinel After")],
            "probe 05: the closed fold ends at the depth-one sentinel"
        );
        let extent = &Navigation::new(&doc).folds[0].lines;
        for (name, line, text) in [
            ("hidden-target", 66, "Hidden Target"),
            ("hidden-explicit", 68, "`:hidden-explicit"),
        ] {
            assert_eq!(
                resolved(&doc, name),
                Some((line, text.into())),
                "probe 05: {name}"
            );
            assert!(
                extent.contains(&line),
                "probe 05: {name} lies inside the fold"
            );
        }
    }

    #[test]
    fn probe_06_extents_ignore_collapsibility_and_nest() {
        let doc = page!("navigation/probe-nav-06a-same-depth-collapsible.mu");
        assert_eq!(
            folds(&doc),
            vec![
                fold(2, "Outer Closed", false, 3..4, "Inner Authored Open"),
                fold(
                    4,
                    "Inner Authored Open",
                    true,
                    5..6,
                    "Inner Authored Closed"
                ),
                fold(6, "Inner Authored Closed", false, 7..8, "Sentinel After"),
                fold(11, "Outer Open", true, 12..13, "Nested Under Open"),
                fold(13, "Nested Under Open", false, 14..15, "Probe 06a end"),
            ],
            "probe 06a: a same-depth heading ends a fold whether or not it is collapsible"
        );

        let doc = page!("navigation/probe-nav-06b-nested-collapsible.mu");
        assert_eq!(
            folds(&doc),
            vec![
                fold(2, "Outer Closed", false, 3..8, "Sentinel After"),
                fold(
                    4,
                    "Inner Authored Open",
                    true,
                    5..6,
                    "Inner Authored Closed"
                ),
                fold(6, "Inner Authored Closed", false, 7..8, "Sentinel After"),
            ],
            "probe 06b: depth-two folds nest inside the outer fold with their authored state"
        );
    }

    #[test]
    fn probe_07_less_than_ends_folds_and_keeps_depth() {
        let doc = page!("navigation/probe-nav-07c-section-exit-fold.mu");
        assert_eq!(
            folds(&doc),
            vec![
                fold(2, "Closed One A", false, 3..4, "<"),
                fold(8, "Closed One B", false, 9..12, "<"),
            ],
            "probe 07c / decision 5: < ends the fold before itself, even past a depth-two heading"
        );

        for doc in [
            page!("navigation/probe-nav-07a-section-exit.mu"),
            page!("navigation/probe-nav-07b-section-exit-deep.mu"),
        ] {
            assert!(Navigation::new(&doc).folds.is_empty());
            let exits: Vec<_> = (1..doc.lines.len())
                .filter(|&index| is_less_than_led(&doc.lines[index]))
                .collect();
            assert!(!exits.is_empty());
            for index in exits {
                assert_eq!(
                    doc.lines[index].section_depth,
                    doc.lines[index - 1].section_depth,
                    "probes 07a/07b: `{}` at L{index} gains no depth meaning",
                    doc.lines[index].source
                );
            }
        }
        let doc = page!("navigation/probe-nav-07a-section-exit.mu");
        let spellings: Vec<_> = doc
            .lines
            .iter()
            .filter(|line| is_less_than_led(line))
            .map(|line| line.source.as_str())
            .collect();
        assert_eq!(
            spellings,
            ["<", "<", "<", "<<", "< trailing text on the same line"],
            "probe 07a: every <-led spelling is recognised"
        );
    }

    #[test]
    fn probe_08_toggle_page_folds() {
        let doc = page!("navigation/probe-nav-08-toggle-keys.mu");
        assert_eq!(
            folds(&doc),
            vec![
                fold(2, "Enter Target", false, 3..4, "Sentinel One"),
                fold(6, "Space Target", false, 7..8, "Sentinel Two"),
                fold(10, "Already Open", true, 11..12, "Probe 08 end"),
            ],
            "probe 08: three one-line folds with authored state"
        );
    }

    #[test]
    fn probe_11_unnamed_sections_end_only_shallower_folds() {
        let doc = page!("navigation/probe-nav-11a-unnamed-fold.mu");
        assert_eq!(
            folds(&doc),
            vec![
                fold(2, "Closed One A", false, 3..7, "Sentinel A"),
                fold(9, "Closed Four B", false, 10..14, "Sentinel B"),
                fold(16, "Closed Two C", false, 17..18, ">"),
            ],
            "C1b probe 11a: a deeper or equal bare run stays inside; a shallower one ends the fold"
        );
        assert!(
            !anchors(&doc)
                .iter()
                .any(|(_, line, ..)| [4, 11, 18].contains(line)),
            "C1b probe 11a: unnamed sections declare no anchor"
        );

        let doc = page!("navigation/probe-nav-11c-unnamed-fold-depths.mu");
        assert_eq!(
            folds(&doc),
            vec![
                fold(2, "Closed One D", false, 3..6, "Sentinel D"),
                fold(8, "Closed Two E", false, 9..12, "Sentinel E"),
                fold(14, "Closed Four F", false, 15..16, ">>"),
            ],
            "C1b probe 11c: equal-depth bare runs stay inside; a shallower one ends the fold"
        );

        let doc = page!("navigation/probe-nav-11b-next-heading-unnamed.mu");
        assert_eq!(label(&doc, 63), ">>>>");
        assert_eq!(
            next(&doc, 2),
            Some((125, "Named After Unnamed".into())),
            "C1b probe 11b: # skips the bare >>>> for the next named heading"
        );
    }

    #[test]
    fn probes_12_13_less_than_spellings_stop_before_their_own_line() {
        let doc = page!("navigation/probe-nav-12-double-less-than-fold.mu");
        assert_eq!(
            folds(&doc),
            vec![
                fold(2, "Closed Twelve A", false, 3..3, "<<"),
                fold(8, "Closed Twelve B", false, 9..9, "<"),
                fold(14, "Closed Twelve C", false, 15..16, "<<"),
            ],
            "C1b probe 12: << and < first in a fold leave it empty; << after body ends it there"
        );

        let doc = page!("navigation/probe-nav-13-less-than-text-fold.mu");
        assert_eq!(
            folds(&doc),
            vec![fold(
                2,
                "Closed Thirteen",
                false,
                3..4,
                "< TEXT ON THE LESS-THAN LINE"
            )],
            "C1b probe 13: < text sits outside the fold it follows"
        );
    }

    #[test]
    fn probe_15_explicit_anchor_after_a_heading_never_renames_it() {
        let doc = page!("navigation/probe-nav-15a-heading-anchor-same.mu");
        assert_eq!(
            anchors(&doc)[..2],
            [
                anchor("setup", 63, true, "Setup"),
                anchor("setup", 64, false, "`:setup"),
            ],
            "C1b probe 15a: the same-name explicit anchor is an inactive duplicate"
        );
        assert_eq!(
            resolved(&doc, "setup"),
            Some((63, "Setup".into())),
            "C1b probe 15a: #setup lands on the heading row"
        );

        let doc = page!("navigation/probe-nav-15b-heading-anchor-other.mu");
        assert_eq!(
            resolved(&doc, "setup"),
            Some((64, "Setup".into())),
            "C1b probe 15b: #setup lands on the heading row"
        );
        assert_eq!(
            resolved(&doc, "install"),
            Some((65, "`:install".into())),
            "C1b probe 15b: #install is its own declaration after the heading"
        );
    }

    #[test]
    fn probe_16_duplicate_explicit_anchors_first_wins() {
        let doc = page!("navigation/probe-nav-16-duplicate-explicit.mu");
        assert_eq!(
            anchors(&doc)[..2],
            [
                anchor("dup", 63, true, "`:dup"),
                anchor("dup", 125, false, "`:dup"),
            ],
            "C1b probe 16: the second declaration is inactive"
        );
        assert_eq!(
            resolved(&doc, "dup"),
            Some((63, "`:dup".into())),
            "C1b probe 16"
        );
    }

    #[test]
    fn probe_17_target_inside_two_closed_folds() {
        let doc = page!("navigation/probe-nav-17-nested-closed-target.mu");
        assert_eq!(
            folds(&doc),
            vec![
                fold(63, "Outer Closed", false, 64..69, "Sentinel After"),
                fold(65, "Inner Closed", false, 66..69, "Sentinel After"),
            ],
            "C1b probe 17: the depth-one sentinel ends both folds"
        );
        let navigation = Navigation::new(&doc);
        let target = navigation.resolve_anchor("nested-deep").unwrap().line;
        assert_eq!(label(&doc, target), "`:nested-deep");
        assert_eq!(
            navigation
                .folds
                .iter()
                .filter(|fold| fold.lines.contains(&target))
                .map(|fold| fold.line)
                .collect::<Vec<_>>(),
            [63, 65],
            "C1b probe 17: the target lies inside both closed folds"
        );
    }

    #[test]
    fn table_cell_anchors_are_declared_on_the_table_line() {
        let doc = syntax::parse("\u{60}t\n| Name |\n| --- |\n| \u{60}:cell Here |\n\u{60}t\n>Cell");
        assert_eq!(
            anchors(&doc),
            vec![
                anchor("cell", 0, true, "\u{60}t"),
                anchor("cell", 1, false, "Cell"),
            ],
            "design note §8: a cell anchor targets its table and still wins over a later slug"
        );
    }
}
