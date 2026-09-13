// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

//! Projection of the captured Micron grammar into portable document blocks.
//! The syntax tree retains constructs this particular projection cannot render.

use super::syntax::{self, Alignment, LineKind, Span, Style};
use inker::{
    Block, DocumentDiagnostic, DocumentProvenance, DocumentTrustState, Engine, EngineDocument,
    EngineError, EngineInput, InlineSpan,
};

pub const ENGINE_ID: &str = "nematic.micron";

#[derive(Default)]
pub struct MicronEngine;

impl MicronEngine {
    pub fn new() -> Self {
        Self
    }
}

impl Engine for MicronEngine {
    fn engine_id(&self) -> &str {
        ENGINE_ID
    }

    fn render(&self, input: &EngineInput) -> Result<EngineDocument, EngineError> {
        let parsed = syntax::parse(&input.body);
        let mut blocks = Vec::new();
        let mut diagnostics = Vec::new();
        let mut title = None;
        for line in &parsed.lines {
            if line.alignment != Alignment::Default && line.alignment != Alignment::Left {
                loss(
                    &mut diagnostics,
                    "Micron alignment retained in syntax; native projection uses reader alignment",
                );
            }
            match &line.kind {
                LineKind::Comment | LineKind::LiteralDelimiter => {},
                LineKind::Header { .. } => {
                    loss(
                        &mut diagnostics,
                        "Micron page directives retained in source; not applied by this projection",
                    );
                },
                LineKind::Blank => blocks.push(Block::Preformatted {
                    text: String::new(),
                }),
                LineKind::Literal => blocks.push(Block::Preformatted {
                    text: line.source.clone(),
                }),
                LineKind::Divider => blocks.push(Block::Rule),
                LineKind::Heading {
                    depth,
                    initially_open,
                    ..
                } => {
                    let spans = lower_spans(&line.spans, &input.address, &mut diagnostics);
                    let heading = inker::inline_text(&spans);
                    if *depth == 1 && title.is_none() && !heading.trim().is_empty() {
                        title = Some(heading.trim().to_owned());
                    }
                    if initially_open.is_some() {
                        loss(
                            &mut diagnostics,
                            "Micron collapsible section retained in syntax; native projection shows its contents expanded",
                        );
                    }
                    if *depth > 6 {
                        loss(
                            &mut diagnostics,
                            "Micron heading depth exceeds portable heading levels",
                        );
                    }
                    if !heading.is_empty() {
                        blocks.push(Block::Heading {
                            level: (*depth).min(6) as u8,
                            spans,
                        });
                    }
                },
                LineKind::Table { options, body } => {
                    if !options.is_empty() {
                        loss(
                            &mut diagnostics,
                            "Micron table width and block alignment retained in syntax; native projection uses reader geometry",
                        );
                    }
                    if let Some(table) =
                        lower_table(body, &line.style, &input.address, &mut diagnostics)
                    {
                        blocks.push(table);
                    } else {
                        loss(
                            &mut diagnostics,
                            "Unrecognized Micron table body kept as literal source",
                        );
                        blocks.push(Block::Preformatted { text: body.clone() });
                    }
                },
                LineKind::Image { alt, target, .. } => {
                    // No image fetch is implied by parsing author-controlled source.
                    blocks.push(Block::Preformatted {
                        text: format!("[Image: {alt}] {target}"),
                    });
                    loss(
                        &mut diagnostics,
                        "Micron network image retained in syntax; native projection shows its alternative text",
                    );
                },
                LineKind::Text => {
                    let spans = lower_spans(&line.spans, &input.address, &mut diagnostics);
                    if !spans.is_empty() {
                        blocks.push(Block::Paragraph { spans });
                    }
                },
            }
            if line.section_depth > 1 {
                loss(
                    &mut diagnostics,
                    "Micron section depth retained in syntax; native projection does not indent sections",
                );
            }
        }
        if !diagnostics.is_empty() {
            blocks.insert(0, Block::Badge {
                text: "Micron preview: some presentation or controls are shown without their native behavior.".into(),
            });
        }
        Ok(EngineDocument {
            address: input.address.clone(),
            title,
            content_type: input.content_type.clone().unwrap_or_default(),
            lang: None,
            provenance: DocumentProvenance::for_engine(ENGINE_ID, &input.address),
            trust: DocumentTrustState::Unknown,
            diagnostics,
            blocks,
        })
    }
}

fn loss(diagnostics: &mut Vec<DocumentDiagnostic>, message: &str) {
    let diagnostic = DocumentDiagnostic::UnsupportedConstruct(message.to_owned());
    if !diagnostics.contains(&diagnostic) {
        diagnostics.push(diagnostic);
    }
}

fn lower_table(
    body: &str,
    inherited: &Style,
    address: &str,
    diagnostics: &mut Vec<DocumentDiagnostic>,
) -> Option<Block> {
    // Table cells contain Micron inline syntax, not Markdown inline syntax.
    // Keep the admitted grammar bounded to the Guide's explicit pipe rows.
    fn cells(line: &str) -> Option<Vec<&str>> {
        Some(
            line.trim()
                .strip_prefix('|')?
                .strip_suffix('|')?
                .split('|')
                .map(str::trim)
                .collect(),
        )
    }
    let mut lines = body.lines();
    let header = cells(lines.next()?)?;
    let separator = cells(lines.next()?)?;
    if header.len() != separator.len() {
        return None;
    }
    let alignments: Option<Vec<_>> = separator
        .into_iter()
        .map(|cell| {
            let middle = cell.strip_prefix(':').unwrap_or(cell);
            let middle = middle.strip_suffix(':').unwrap_or(middle);
            if middle.is_empty() || !middle.bytes().all(|byte| byte == b'-') {
                return None;
            }
            Some(match (cell.starts_with(':'), cell.ends_with(':')) {
                (true, true) => inker::TableAlignment::Center,
                (true, false) => inker::TableAlignment::Left,
                (false, true) => inker::TableAlignment::Right,
                _ => inker::TableAlignment::None,
            })
        })
        .collect();
    let rows: Option<Vec<_>> = lines.map(cells).collect();
    let mut style = inherited.clone();
    let mut lower_cell =
        |cell: &str| lower_spans(&syntax::inline(cell, &mut style), address, diagnostics);
    Some(Block::Table {
        alignments: alignments?,
        header: header.into_iter().map(&mut lower_cell).collect(),
        rows: rows?
            .into_iter()
            .map(|row| row.into_iter().map(&mut lower_cell).collect())
            .collect(),
    })
}

fn text_span(text: &str, style: &Style, diagnostics: &mut Vec<DocumentDiagnostic>) -> InlineSpan {
    if style.underline || style.foreground.is_some() || style.background.is_some() {
        loss(
            diagnostics,
            "Micron underline and colors retained in syntax; portable projection uses reader styling",
        );
    }
    let mut span = InlineSpan::Text(text.to_owned());
    if style.italic {
        span = InlineSpan::Emphasis(vec![span]);
    }
    if style.bold {
        span = InlineSpan::Strong(vec![span]);
    }
    span
}

fn lower_spans(
    spans: &[Span],
    address: &str,
    diagnostics: &mut Vec<DocumentDiagnostic>,
) -> Vec<InlineSpan> {
    let mut output = Vec::new();
    for span in spans {
        match span {
            Span::Text { text, style } => output.push(text_span(text, style, diagnostics)),
            Span::Link {
                label,
                target,
                effect,
                style,
            } => {
                if effect.is_some() || target.starts_with('#') {
                    // Neither a form action nor local scroll becomes a URL fetch.
                    output.push(text_span(label, style, diagnostics));
                    loss(
                        diagnostics,
                        "Micron request or anchor link retained in syntax; native interaction is not implemented",
                    );
                } else if let Some(target) = resolve_target(address, target) {
                    output.push(InlineSpan::Link {
                        url: target,
                        title: None,
                        spans: vec![text_span(label, style, diagnostics)],
                        predicate: None,
                    });
                } else if is_same_node_path(target) {
                    output.push(InlineSpan::Link {
                        url: target.to_owned(),
                        title: None,
                        spans: vec![text_span(label, style, diagnostics)],
                        predicate: None,
                    });
                    loss(
                        diagnostics,
                        "Micron same-node link retained as an unresolved manifest alias",
                    );
                } else {
                    output.push(text_span(label, style, diagnostics));
                    loss(
                        diagnostics,
                        "Micron link has no qualified resolution for this source address",
                    );
                }
            },
            Span::Anchor { .. } => loss(
                diagnostics,
                "Micron anchor retained in syntax; native anchor scrolling is not implemented",
            ),
            Span::Field { source, .. }
            | Span::Partial { source, .. }
            | Span::Unsupported(source) => {
                output.push(InlineSpan::Code(source.clone()));
                loss(
                    diagnostics,
                    "Micron field, partial or unqualified control retained as inert source",
                );
            },
        }
    }
    output
}

fn is_same_node_path(target: &str) -> bool {
    target
        .strip_prefix(':')
        .is_some_and(|path| path.starts_with('/') && !path.contains('\0'))
}

/// Resolve only the ordinary absolute NomadNet spelling and a same-node
/// absolute page path. Generic URI joining does not understand these addresses.
pub fn resolve_target(base: &str, target: &str) -> Option<String> {
    fn destination(address: &str) -> Option<&str> {
        let (node, path) = address.split_once(':')?;
        (node.len() == 32
            && node.bytes().all(|byte| byte.is_ascii_hexdigit())
            && path.starts_with('/')
            && !path.contains('\0'))
        .then_some(node)
    }
    if destination(target).is_some() {
        Some(target.to_owned())
    } else if let Some(path) = target
        .strip_prefix(':')
        .filter(|path| path.starts_with('/') && !path.contains('\0'))
    {
        destination(base).map(|node| format!("{node}:{path}"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "0123456789abcdef0123456789abcdef:/page/index.mu";

    #[test]
    fn ordinary_links_resolve_but_actions_and_anchors_never_become_fetches() {
        let source = ">Hello\n\u{60}[Read\u{60}:/page/read.mu]\n\u{60}[Send\u{60}:/page/action.mu\u{60}*]\n\u{60}[Top\u{60}#hello]";
        let doc = MicronEngine::new()
            .render(&EngineInput::new(BASE, source))
            .unwrap();
        assert_eq!(doc.title.as_deref(), Some("Hello"));
        assert_eq!(
            doc.outgoing_links(),
            vec!["0123456789abcdef0123456789abcdef:/page/read.mu".to_owned()]
        );
        assert!(!doc.diagnostics.is_empty());
    }

    #[test]
    fn same_node_alias_is_retained_without_fabricating_a_destination() {
        let source = "\u{60}[Read\u{60}:/page/read.mu]";
        let doc = MicronEngine::new()
            .render(&EngineInput::new("file:///tmp/index.mu", source))
            .unwrap();
        assert_eq!(doc.outgoing_links(), vec![":/page/read.mu".to_owned()]);
        assert!(doc.diagnostics.iter().any(|diagnostic| matches!(diagnostic,
            DocumentDiagnostic::UnsupportedConstruct(message) if message.contains("unresolved manifest alias"))));
    }

    #[test]
    fn table_lowering_preserves_cells_and_column_alignment() {
        let source = "\u{60}t\n| Name | Number |\n| :--- | ---: |\n| A | 2 |\n\u{60}t";
        let doc = MicronEngine::new()
            .render(&EngineInput::new(BASE, source))
            .unwrap();
        assert!(doc.blocks.iter().any(|block| matches!(block,
            Block::Table { header, rows, alignments }
                if header.len() == 2 && rows.len() == 1 && alignments == &[inker::TableAlignment::Left, inker::TableAlignment::Right])));
    }

    #[test]
    fn invalid_or_contextless_native_targets_do_not_gain_authority() {
        assert_eq!(
            resolve_target("file:///tmp/index.mu", ":/page/next.mu"),
            None
        );
        assert_eq!(resolve_target(BASE, ":/page/a\0b"), None);
        assert_eq!(resolve_target(BASE, "#section"), None);
        assert_eq!(resolve_target(BASE, "next.mu"), None);
    }
}
