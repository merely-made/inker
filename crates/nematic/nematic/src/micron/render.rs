// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

//! Projection of the captured Micron grammar into portable document blocks.
//! The syntax tree retains constructs this particular projection cannot render.

use super::navigation::{self, Navigation};
use super::syntax::{self, Alignment, LineKind, LinkEffect, Span, Style};
use inker::{
    Block, BlockAlignment, BlockPresentation, DocumentAnchor, DocumentDiagnostic, DocumentFold,
    DocumentNavigation, DocumentProvenance, DocumentTrustState, Engine, EngineDocument,
    EngineError, EngineInput, InPageTarget, InlinePresentation, InlineSpan,
};

/// What span lowering needs beyond the spans: the source address, and the
/// line-space navigation model for in-page links on `line`.
struct Context<'a> {
    address: &'a str,
    navigation: &'a Navigation,
    line: usize,
}

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
        let model = Navigation::new(&parsed);
        let mut blocks = Vec::new();
        let mut diagnostics = Vec::new();
        let mut title = None;
        // Blocks emitted before each line; the last entry is the total.
        let mut first_block = Vec::with_capacity(parsed.lines.len() + 1);
        for (index, line) in parsed.lines.iter().enumerate() {
            let block_start = blocks.len();
            first_block.push(block_start);
            let context = Context {
                address: &input.address,
                navigation: &model,
                line: index,
            };
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
                    let spans = lower_spans(&line.spans, &context, &mut diagnostics);
                    let heading = inker::inline_text(&spans);
                    if *depth == 1 && title.is_none() && !heading.trim().is_empty() {
                        title = Some(heading.trim().to_owned());
                    }
                    let named = navigation::is_named_heading(line);
                    if initially_open.is_some() && !named {
                        // Named folds travel in the navigation table; this spelling is uncaptured.
                        loss(
                            &mut diagnostics,
                            "Micron collapse marker on an unnamed section retained in syntax; not applied",
                        );
                    }
                    if *depth > 6 {
                        loss(
                            &mut diagnostics,
                            "Micron heading depth exceeds portable heading levels",
                        );
                    }
                    if named {
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
                            "Micron table width retained in syntax; native projection uses reader geometry",
                        );
                    }
                    if let Some(table) = lower_table(body, &line.style, &context, &mut diagnostics)
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
                    if navigation::is_less_than_led(line) {
                        // It ends folds (decision 5) but its own meaning is uncaptured.
                        loss(
                            &mut diagnostics,
                            "Micron leading < line retained as source; section exit is not applied",
                        );
                    }
                    let spans = lower_spans(&line.spans, &context, &mut diagnostics);
                    if !spans.is_empty() {
                        blocks.push(Block::Paragraph { spans });
                    }
                },
            }
            for block in &mut blocks[block_start..] {
                *block = present_block(std::mem::replace(block, Block::Rule), line);
            }
        }
        first_block.push(blocks.len());
        let offset = usize::from(!diagnostics.is_empty());
        if offset == 1 {
            blocks.insert(0, Block::Badge {
                text: "Micron preview: some presentation or controls are shown without their native behavior.".into(),
            });
        }
        let navigation = lower_navigation(&model, &first_block, offset);
        for block in &mut blocks {
            for_each_in_page(block, &mut |target| {
                // Lowering stored the target line; the blocks are now known.
                let line = target.block.take();
                target.block = line.and_then(|line| block_at(&first_block, line, offset));
                if target.block.is_none() {
                    target.fragment = None;
                } else if target.fragment.is_none() {
                    target.fragment = navigation
                        .anchors
                        .iter()
                        .find(|anchor| anchor.active && anchor.block == target.block)
                        .map(|anchor| anchor.name.clone());
                }
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
            navigation,
            blocks,
        })
    }
}

/// The first block at or after `line`, shifted past a leading Badge.
fn block_at(first_block: &[usize], line: usize, offset: usize) -> Option<usize> {
    let block = first_block[line];
    (block < first_block[first_block.len() - 1]).then_some(block + offset)
}

fn lower_navigation(
    model: &Navigation,
    first_block: &[usize],
    offset: usize,
) -> DocumentNavigation {
    DocumentNavigation {
        block_count: first_block[first_block.len() - 1] + offset,
        anchors: model
            .anchors
            .iter()
            .map(|anchor| DocumentAnchor {
                name: anchor.name.clone(),
                block: block_at(first_block, anchor.line, offset),
                source_line: anchor.line,
                active: anchor.active,
            })
            .collect(),
        folds: model
            .folds
            .iter()
            .map(|fold| DocumentFold {
                // A named heading always emits exactly one block.
                heading: first_block[fold.line] + offset,
                source_line: fold.line,
                initially_open: fold.initially_open,
                extent: first_block[fold.lines.start] + offset
                    ..first_block[fold.lines.end] + offset,
            })
            .collect(),
    }
}

/// Micron puts in-page links only at the top level of a heading, paragraph
/// or table cell.
fn for_each_in_page(block: &mut Block, patch: &mut impl FnMut(&mut InPageTarget)) {
    let lists: Vec<&mut Vec<InlineSpan>> = match block {
        Block::Presented { block, .. } => return for_each_in_page(block, patch),
        Block::Heading { spans, .. } | Block::Paragraph { spans } => vec![spans],
        Block::Table { header, rows, .. } => {
            header.iter_mut().chain(rows.iter_mut().flatten()).collect()
        },
        _ => Vec::new(),
    };
    for span in lists.into_iter().flatten() {
        if let InlineSpan::InPage { target, .. } = span {
            patch(target);
        }
    }
}

/// `#name` or `#` resolved in line space. `block` holds the target *line*
/// until the blocks are known.
fn in_page_target(context: &Context, name: &str) -> InPageTarget {
    let model = context.navigation;
    let (line, fragment) = if name.is_empty() {
        // Decision 6: the next named heading after the link's own line.
        let heading = model.next_heading(context.line);
        let fragment = heading.and_then(|heading| {
            model
                .anchors
                .iter()
                .find(|anchor| anchor.line == heading && anchor.active)
                .map(|anchor| anchor.name.clone())
        });
        (heading, fragment)
    } else {
        let declaration = model.resolve_anchor(name);
        (
            declaration.map(|anchor| anchor.line),
            declaration.map(|anchor| anchor.name.clone()),
        )
    };
    InPageTarget {
        fragment,
        block: line,
    }
}

fn present_block(block: Block, line: &syntax::Line) -> Block {
    let alignment = match &line.kind {
        LineKind::Table { options, .. } => match options.chars().next() {
            Some('c') => BlockAlignment::Center,
            Some('r') => BlockAlignment::End,
            Some('l') => BlockAlignment::Start,
            _ => block_alignment(line.alignment),
        },
        _ => block_alignment(line.alignment),
    };
    let presentation = BlockPresentation {
        alignment,
        indent_level: u32::try_from(line.section_depth.saturating_sub(1)).unwrap_or(u32::MAX),
    };
    if presentation == BlockPresentation::default() {
        block
    } else {
        Block::Presented {
            presentation,
            block: Box::new(block),
        }
    }
}

fn block_alignment(alignment: Alignment) -> BlockAlignment {
    match alignment {
        Alignment::Default | Alignment::Left => BlockAlignment::Start,
        Alignment::Center => BlockAlignment::Center,
        Alignment::Right => BlockAlignment::End,
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
    context: &Context,
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
        |cell: &str| lower_spans(&syntax::inline(cell, &mut style), context, diagnostics);
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
    let _ = diagnostics;
    let mut span = InlineSpan::Text(text.to_owned());
    if style.italic {
        span = InlineSpan::Emphasis(vec![span]);
    }
    if style.bold {
        span = InlineSpan::Strong(vec![span]);
    }
    let presentation = InlinePresentation {
        foreground: style.foreground,
        background: style.background,
        underline: style.underline,
    };
    if presentation == InlinePresentation::default() {
        span
    } else {
        InlineSpan::Presented {
            presentation,
            spans: vec![span],
        }
    }
}

fn lower_spans(
    spans: &[Span],
    context: &Context,
    diagnostics: &mut Vec<DocumentDiagnostic>,
) -> Vec<InlineSpan> {
    let address = context.address;
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
                // Neither a form action nor local scroll becomes a URL fetch.
                if let Some(effect) = effect {
                    output.push(text_span(label, style, diagnostics));
                    loss(
                        diagnostics,
                        match effect {
                            LinkEffect::RequestSelectors(_) => {
                                "Micron request link retained in syntax; native interaction is not implemented"
                            },
                            LinkEffect::Anchor(_) => {
                                "Micron cross-page anchor link retained in syntax; native interaction is not implemented"
                            },
                        },
                    );
                } else if let Some(name) = target.strip_prefix('#') {
                    // A missing target stays an inert label with no diagnostic, as stock is silent.
                    output.push(InlineSpan::InPage {
                        target: in_page_target(context, name),
                        spans: vec![text_span(label, style, diagnostics)],
                    });
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
            // Zero-width; declarations travel in the navigation table.
            Span::Anchor { .. } => {},
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
mod navigation_tests;

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
    fn captured_presentation_becomes_typed_source_wrappers() {
        let source = "\u{60}c\u{60}F123\u{60}Babc\u{60}_Styled\n>> Nested\n\u{60}tc30\n| Name | Count |\n| --- | ---: |\n| A | 2 |\n\u{60}t";
        let doc = MicronEngine::new()
            .render(&EngineInput::new(BASE, source))
            .unwrap();

        let paragraph = doc.blocks.iter().find_map(|block| match block {
            Block::Presented {
                presentation:
                    BlockPresentation {
                        alignment: BlockAlignment::Center,
                        indent_level: 0,
                    },
                block,
            } => match block.as_ref() {
                Block::Paragraph { spans } => Some(spans),
                _ => None,
            },
            _ => None,
        });
        let spans = paragraph.expect("centered source paragraph");
        assert!(matches!(
            spans.first(),
            Some(InlineSpan::Presented {
                presentation: InlinePresentation {
                    foreground: Some([0x11, 0x22, 0x33]),
                    background: Some([0xaa, 0xbb, 0xcc]),
                    underline: true,
                },
                ..
            })
        ));
        assert!(doc.blocks.iter().any(|block| {
            matches!(block,
                Block::Presented {
                    presentation: BlockPresentation { alignment: BlockAlignment::Center, indent_level: 1 },
                    block,
                } if matches!(block.as_ref(), Block::Heading { .. }))
        }));
        assert!(doc.blocks.iter().any(|block| {
            matches!(block,
                Block::Presented {
                    presentation: BlockPresentation { alignment: BlockAlignment::Center, .. },
                    block,
                } if matches!(block.as_ref(), Block::Table { .. }))
        }));
        assert!(
            !doc.diagnostics.iter().any(|diagnostic| matches!(diagnostic,
                DocumentDiagnostic::UnsupportedConstruct(message)
                    if message.contains("colors, underline, alignment, or section indentation"))),
            "captured reader presentation is now carried through the document model"
        );
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
