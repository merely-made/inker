// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Evidence-qualified Micron preview.
//!
//! This is deliberately not a general Micron parser. It lowers only the exact
//! byte forms captured in `tests/fixtures/micron/CAPTURE_MANIFEST.md`. Every
//! other physical source line stays inert and visible in a read-only raw block.

use inker::{
    Block, DocumentDiagnostic, DocumentProvenance, DocumentTrustState, Engine, EngineDocument,
    EngineError, EngineInput, InlineSpan,
};

/// Stable engine identifier for the evidence-qualified preview subset.
pub const ENGINE_ID: &str = "nematic.micron-subset";

/// A small, source-preserving Micron preview based only on checked-in captures.
pub struct MicronSubsetEngine;

impl MicronSubsetEngine {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MicronSubsetEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine for MicronSubsetEngine {
    fn engine_id(&self) -> &str {
        ENGINE_ID
    }

    fn render(&self, input: &EngineInput) -> Result<EngineDocument, EngineError> {
        let mut lowering = Lowering::default();
        lowering
            .diagnostics
            .push(DocumentDiagnostic::DegradedRendering(
                "Partial Micron preview; unsupported source remains raw and read-only.".to_string(),
            ));
        lowering.blocks.push(Block::Badge {
            text: "Partial Micron preview. Unsupported source is inert, visible, and read-only."
                .to_string(),
        });

        for physical_line in physical_lines(&input.body) {
            lowering.handle(physical_line);
        }

        Ok(EngineDocument {
            address: input.address.clone(),
            title: None,
            // Micron's media type has not been qualified by these captures. Preserve
            // a host-supplied value and otherwise leave it empty.
            content_type: input.content_type.clone().unwrap_or_default(),
            lang: None,
            provenance: DocumentProvenance::for_engine(self.engine_id(), &input.address),
            trust: DocumentTrustState::Unknown,
            diagnostics: lowering.diagnostics,
            blocks: lowering.blocks,
        })
    }
}

#[derive(Default)]
struct Lowering {
    blocks: Vec<Block>,
    diagnostics: Vec<DocumentDiagnostic>,
}

impl Lowering {
    fn handle(&mut self, line: &str) {
        if line == "---" {
            self.blocks.push(Block::Rule);
        } else if line.is_empty() || line.contains('\r') || has_unqualified_control(line) {
            self.unsupported(line);
        } else if let Some(text) = line.strip_prefix("> ").filter(|text| !text.is_empty()) {
            self.blocks.push(Block::Heading {
                level: 1,
                spans: vec![InlineSpan::Text(text.to_string())],
            });
        } else if let Some(spans) = exact_inline_style(line, "`!", true) {
            self.blocks.push(Block::Paragraph { spans });
        } else if let Some(spans) = exact_inline_style(line, "`*", false) {
            self.blocks.push(Block::Paragraph { spans });
        } else if is_captured_plain(line) {
            self.blocks.push(Block::Paragraph {
                spans: vec![InlineSpan::Text(line.to_string())],
            });
        } else {
            self.unsupported(line);
        }
    }
    fn unsupported(&mut self, line: &str) {
        if !line.is_empty() {
            self.blocks.push(Block::Badge {
                text: "Unsupported Micron source (read-only)".to_string(),
            });
            self.diagnostics
                .push(DocumentDiagnostic::UnsupportedConstruct(
                    "Micron line preserved without interpretation".to_string(),
                ));
            self.diagnostics.push(DocumentDiagnostic::RawSourceFallback);
        }
        self.blocks.push(Block::Preformatted {
            text: line.to_string(),
        });
    }
}

/// Splits only LF-terminated physical lines. A CR before LF remains source text,
/// so CRLF has not silently gained the semantics captured for LF-only fixtures.
fn physical_lines(source: &str) -> impl Iterator<Item = &str> {
    source
        .split_inclusive('\n')
        .map(|fragment| fragment.strip_suffix('\n').unwrap_or(fragment))
}

/// These controls were observed in candidate input but not qualified as plain
/// content. They always remain visible raw unless an exact captured branch
/// above accepts the whole physical line.
fn has_unqualified_control(line: &str) -> bool {
    line.starts_with('#')
        || line.starts_with('-')
        || line.starts_with('|')
        || line.contains('[')
        || line.contains(']')
        || line.contains('|')
        || (line.contains('>') && !line.starts_with("> "))
        || (line.starts_with("> ")
            && (line[2..].starts_with('-')
                || line[2..].contains('>')
                || line[2..].contains('`')
                || line[2..].contains('[')
                || line[2..].contains(']')
                || line[2..].contains('|')))
}

/// The two captured inline controls each occur exactly twice on one LF-only
/// physical line and have nonempty content between them. Other backticks are
/// deliberately not interpreted.
fn exact_inline_style(line: &str, marker: &str, strong: bool) -> Option<Vec<InlineSpan>> {
    if line.matches(marker).count() != 2 || line.contains('`') && line.matches('`').count() != 2 {
        return None;
    }
    let (prefix, rest) = line.split_once(marker)?;
    let (styled, suffix) = rest.split_once(marker)?;
    if styled.is_empty() {
        return None;
    }
    let styled_span = if strong {
        InlineSpan::Strong(vec![InlineSpan::Text(styled.to_string())])
    } else {
        InlineSpan::Emphasis(vec![InlineSpan::Text(styled.to_string())])
    };
    Some(vec![
        InlineSpan::Text(prefix.to_string()),
        styled_span,
        InlineSpan::Text(suffix.to_string()),
    ])
}

/// Plain lines were captured only without control-looking prefixes or markers.
/// Keep known-but-unqualified marker forms visible as raw source instead.
fn is_captured_plain(line: &str) -> bool {
    !line.is_empty()
        && !line.starts_with('#')
        && !line.starts_with('-')
        && !line.starts_with('>')
        && !line.contains('`')
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROBE: &str = include_str!("../tests/fixtures/micron/probe.mu");
    const STYLES_AND_LINKS: &str = include_str!("../tests/fixtures/micron/styles-links.mu");

    fn render(body: &str) -> EngineDocument {
        MicronSubsetEngine::new()
            .render(&EngineInput::new("nomadnet:test/page.mu", body))
            .expect("render")
    }

    #[test]
    fn captured_probe_lowers_only_the_observed_lf_forms() {
        let document = render(PROBE);
        assert!(matches!(document.blocks[0], Block::Badge { .. }));
        assert_eq!(
            document.blocks[1],
            Block::Heading {
                level: 1,
                spans: vec![InlineSpan::Text("Heading".to_string())],
            }
        );
        assert_eq!(document.blocks[2], Block::Rule);
        assert_eq!(
            document.blocks[3],
            Block::Paragraph {
                spans: vec![InlineSpan::Text("plain".to_string())],
            }
        );
        assert_eq!(
            document.diagnostics,
            vec![DocumentDiagnostic::DegradedRendering(
                "Partial Micron preview; unsupported source remains raw and read-only.".to_string()
            )]
        );
    }

    #[test]
    fn captured_inline_styles_lower_but_link_candidates_stay_inert() {
        let document = render(STYLES_AND_LINKS);
        assert_eq!(
            document.blocks[1],
            Block::Paragraph {
                spans: vec![
                    InlineSpan::Text("plain ".to_string()),
                    InlineSpan::Strong(vec![InlineSpan::Text("bold".to_string())]),
                    InlineSpan::Text(" plain".to_string()),
                ],
            }
        );
        assert_eq!(
            document.blocks[2],
            Block::Paragraph {
                spans: vec![
                    InlineSpan::Text("plain ".to_string()),
                    InlineSpan::Emphasis(vec![InlineSpan::Text("italic".to_string())]),
                    InlineSpan::Text(" plain".to_string()),
                ],
            }
        );
        assert!(document.outgoing_links().is_empty());
        assert!(document.blocks.iter().any(|block| matches!(
            block,
            Block::Preformatted { text } if text == "[Local page`:/page/probe.mu]"
        )));
    }

    #[test]
    fn unqualified_or_crlf_source_is_visible_and_inert() {
        let document = render(
            "# heading\n>Heading\n--- \n> Heading\r\n> `!bold`!\n> [label`:/x]\n# `!bold`!\n[x](url)\n|a|\n\n",
        );
        assert!(document.outgoing_links().is_empty());
        for text in [
            "# heading",
            ">Heading",
            "--- ",
            "> Heading\r",
            "> `!bold`!",
            "> [label`:/x]",
            "# `!bold`!",
            "[x](url)",
            "|a|",
            "",
        ] {
            assert!(document.blocks.iter().any(|block| matches!(
                block,
                Block::Preformatted { text: candidate } if candidate == text
            )));
        }
        assert!(
            document
                .diagnostics
                .iter()
                .any(|diagnostic| matches!(diagnostic, DocumentDiagnostic::RawSourceFallback))
        );
        assert!(!document.blocks.iter().any(|block| matches!(
            block,
            Block::Heading { spans, .. } if spans == &vec![InlineSpan::Text("bold".to_string())]
        )));
        assert!(!document.blocks.iter().any(|block| matches!(
            block,
            Block::Paragraph { spans } if spans.iter().any(|span| matches!(span, InlineSpan::Strong(_)))
        )));
    }
    #[test]
    fn host_content_type_is_preserved_without_inventing_one() {
        let document = MicronSubsetEngine::new()
            .render(
                &EngineInput::new("nomadnet:test/page.mu", "plain").with_content_type("host/type"),
            )
            .expect("render");
        assert_eq!(document.content_type, "host/type");
        assert_eq!(render("plain").content_type, "");
    }
}
