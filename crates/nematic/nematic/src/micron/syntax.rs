// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Source-preserving Micron syntax. Presentation and network effects are separate.
//!
//! The reference is the stock NomadNet 1.4.2 in-app Guide, observed through its
//! terminal UI. See the adjacent capture manifest for evidence and open cases.

use std::collections::HashSet;

const CONTROL: char = '\u{60}';

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Alignment {
    #[default]
    Default,
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub foreground: Option<[u8; 3]>,
    pub background: Option<[u8; 3]>,
    pub alignment: Alignment,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkEffect {
    /// Pipes selecting submitted fields and fixed `key=value` variables.
    RequestSelectors(String),
    /// The Guide's external-page anchor modifier, `anchor=name`.
    Anchor(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Text {
        name: String,
        width: Option<usize>,
        rows: Option<usize>,
        masked: bool,
    },
    Checkbox {
        name: String,
        value: String,
        prechecked: bool,
    },
    Radio {
        name: String,
        value: String,
        prechecked: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Span {
    Text {
        text: String,
        style: Style,
    },
    Link {
        label: String,
        target: String,
        /// A preserved modifier. Interpretation and serialization belong to a
        /// reviewed request, never an ordinary navigation URL.
        effect: Option<LinkEffect>,
        style: Style,
    },
    Anchor {
        name: String,
        active: bool,
    },
    Field {
        kind: FieldKind,
        options: String,
        initial: String,
        source: String,
    },
    Partial {
        target: String,
        options: Vec<String>,
        source: String,
    },
    /// Preserve an unqualified or malformed control without giving it effects.
    Unsupported(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LineKind {
    Text,
    Heading {
        depth: usize,
        initially_open: Option<bool>,
        anchor: Option<String>,
    },
    Divider,
    Literal,
    LiteralDelimiter,
    Comment,
    Header {
        name: String,
        value: String,
    },
    Table {
        options: String,
        body: String,
    },
    Image {
        alt: String,
        attributes: Vec<String>,
        target: String,
    },
    Blank,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub source: String,
    pub kind: LineKind,
    pub spans: Vec<Span>,
    pub section_depth: usize,
    pub alignment: Alignment,
    /// Style entering the line, including a table's first cell.
    pub style: Style,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Document {
    pub source: String,
    pub lines: Vec<Line>,
}

/// Parse controls while retaining the physical source for diagnostics and
/// authoring. This function performs no fetches, submissions or filesystem IO.
pub fn parse(source: &str) -> Document {
    let mut document = Document {
        source: source.to_owned(),
        lines: Vec::new(),
    };
    let mut style = Style::default();
    let mut depth = 0;
    let mut literal = false;
    let mut table: Option<(Line, String)> = None;
    let mut declared_anchors = HashSet::new();
    for physical in source.split_inclusive('\n') {
        let source_line = lexeme(physical);
        if let Some((mut opening, mut body)) = table.take() {
            if source_line == "\u{60}t" {
                let options = opening.source[2..].to_owned();
                opening.kind = LineKind::Table { options, body };
                document.lines.push(opening);
            } else {
                body.push_str(physical);
                // Inline controls inside cells participate in document state,
                // as confirmed by the independent table/state capture.
                let mut spans = inline(source_line, &mut style);
                register_explicit_anchors(&mut spans, &mut declared_anchors);
                // Cell anchors stay on the table line so navigation can target it.
                opening.spans.extend(
                    spans
                        .into_iter()
                        .filter(|span| matches!(span, Span::Anchor { .. })),
                );
                table = Some((opening, body));
            }
            continue;
        }
        let mut line = Line {
            source: source_line.to_owned(),
            kind: LineKind::Text,
            spans: Vec::new(),
            section_depth: depth,
            alignment: style.alignment,
            style: style.clone(),
        };
        if source_line == "\u{60}=" {
            literal = !literal;
            line.kind = LineKind::LiteralDelimiter;
        } else if literal {
            line.kind = LineKind::Literal;
            line.spans.push(Span::Text {
                text: source_line.to_owned(),
                style: Style::default(),
            });
        } else if source_line
            .strip_prefix("\u{60}t")
            .is_some_and(table_options)
        {
            table = Some((line, String::new()));
            continue;
        } else if let Some(image) = source_line
            .strip_prefix("\u{60}(")
            .and_then(|body| body.strip_suffix(')'))
        {
            let parts: Vec<_> = image.split(CONTROL).collect();
            if parts.len() >= 2 && !parts[parts.len() - 1].is_empty() {
                line.kind = LineKind::Image {
                    alt: parts[0].to_owned(),
                    target: parts[parts.len() - 1].to_owned(),
                    attributes: parts[1..parts.len() - 1]
                        .iter()
                        .map(|part| (*part).to_owned())
                        .collect(),
                };
            } else {
                line.spans.push(Span::Unsupported(source_line.to_owned()));
            }
        } else if let Some(header) = source_line.strip_prefix("#!") {
            let (name, value) = header.split_once(['=', ' ']).unwrap_or((header, ""));
            line.kind = LineKind::Header {
                name: name.to_owned(),
                value: value.to_owned(),
            };
        } else if source_line.starts_with('#') {
            line.kind = LineKind::Comment;
        } else if source_line.is_empty() {
            line.kind = LineKind::Blank;
        } else if source_line == "---" {
            line.kind = LineKind::Divider;
        } else {
            let (body, initially_open) = if let Some(body) = source_line.strip_prefix("\u{60}+") {
                (body, Some(true))
            } else if let Some(body) = source_line.strip_prefix("\u{60}-") {
                (body, Some(false))
            } else {
                (source_line, None)
            };
            let markers = body.chars().take_while(|c| *c == '>').count();
            let body = if markers > 0 {
                depth = markers;
                line.section_depth = depth;
                line.kind = LineKind::Heading {
                    depth,
                    initially_open,
                    anchor: None,
                };
                &body[markers..]
            } else {
                // A collapse marker is only meaningful on a heading.
                source_line
            };
            line.spans = inline(body, &mut style);
            if matches!(line.kind, LineKind::Heading { .. }) {
                if let Some(anchor) = heading_anchor(&line.spans)
                    && declared_anchors.insert(anchor.clone())
                    && let LineKind::Heading { anchor: slot, .. } = &mut line.kind
                {
                    *slot = Some(anchor);
                }
            }
            register_explicit_anchors(&mut line.spans, &mut declared_anchors);
            line.alignment = style.alignment;
        }
        document.lines.push(line);
    }
    if let Some((mut opening, body)) = table {
        // A missing closing delimiter is not silently repaired into a table.
        opening
            .spans
            .push(Span::Unsupported(format!("{}\n{body}", opening.source)));
        document.lines.push(opening);
    }
    document
}

fn lexeme(physical: &str) -> &str {
    physical
        .strip_suffix("\r\n")
        .or_else(|| physical.strip_suffix('\n'))
        .unwrap_or(physical)
}

pub(super) fn heading_anchor(spans: &[Span]) -> Option<String> {
    let mut text = String::new();
    for span in spans {
        match span {
            Span::Text { text: value, .. } => text.push_str(value),
            Span::Link { label, .. } => text.push_str(label),
            Span::Anchor { .. }
            | Span::Field { .. }
            | Span::Partial { .. }
            | Span::Unsupported(_) => {},
        }
    }
    let mut anchor = String::new();
    let mut separator = false;
    for character in text.chars() {
        if character.is_alphanumeric() {
            if separator && !anchor.is_empty() {
                anchor.push('-');
            }
            anchor.extend(character.to_lowercase());
            separator = false;
        } else {
            separator = true;
        }
    }
    (!anchor.is_empty()).then_some(anchor)
}

fn register_explicit_anchors(spans: &mut [Span], declared: &mut HashSet<String>) {
    for span in spans {
        if let Span::Anchor { name, active } = span {
            *active = declared.insert(name.clone());
        }
    }
}

fn push_text(spans: &mut Vec<Span>, text: &str, style: &Style) {
    if text.is_empty() {
        return;
    }
    if let Some(Span::Text {
        text: previous,
        style: previous_style,
    }) = spans.last_mut()
        && previous_style == style
    {
        previous.push_str(text);
    } else {
        spans.push(Span::Text {
            text: text.to_owned(),
            style: style.clone(),
        });
    }
}

pub(super) fn inline(source: &str, style: &mut Style) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut cursor = 0;
    while cursor < source.len() {
        let rest = &source[cursor..];
        let Some(offset) = rest.find(CONTROL) else {
            push_text(&mut spans, rest, style);
            break;
        };
        push_text(&mut spans, &rest[..offset], style);
        cursor += offset;
        let start = cursor;
        cursor += 1;
        let Some(command) = source[cursor..].chars().next() else {
            spans.push(Span::Unsupported(source[start..].to_owned()));
            break;
        };
        cursor += command.len_utf8();
        match command {
            CONTROL => *style = Style::default(),
            '!' => style.bold = !style.bold,
            '*' => style.italic = !style.italic,
            '_' => style.underline = !style.underline,
            'c' if start == 0 => style.alignment = Alignment::Center,
            'l' if start == 0 => style.alignment = Alignment::Left,
            'a' if start == 0 => style.alignment = Alignment::Default,
            'r' if start == 0 => style.alignment = Alignment::Right,
            'f' => style.foreground = None,
            'b' => style.background = None,
            'F' | 'B' => {
                if let Some((color, used)) = color(&source[cursor..]) {
                    if command == 'F' {
                        style.foreground = Some(color)
                    } else {
                        style.background = Some(color)
                    }
                    cursor += used;
                } else {
                    spans.push(Span::Unsupported(source[start..cursor].to_owned()));
                }
            },
            ':' => {
                let end = source[cursor..]
                    .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
                    .map_or(source.len(), |offset| cursor + offset);
                if end == cursor {
                    spans.push(Span::Unsupported(source[start..cursor].to_owned()));
                } else {
                    spans.push(Span::Anchor {
                        name: source[cursor..end].to_owned(),
                        active: false,
                    });
                    cursor = end;
                }
            },
            '[' | '<' | '{' => {
                let closer = match command {
                    '[' => ']',
                    '<' => '>',
                    _ => '}',
                };
                let Some(offset) = source[cursor..].find(closer) else {
                    spans.push(Span::Unsupported(source[start..].to_owned()));
                    break;
                };
                let end = cursor + offset;
                let parts: Vec<_> = source[cursor..end].split(CONTROL).collect();
                let raw = source[start..end + 1].to_owned();
                cursor = end + 1;
                match command {
                    '[' if parts.len() <= 3
                        && !parts[0].is_empty()
                        && (parts.len() == 1 || !parts[1].is_empty())
                        && (parts.len() < 3 || !parts[2].is_empty()) =>
                    {
                        let (label, target) = if parts.len() == 1 {
                            (parts[0], parts[0])
                        } else {
                            (parts[0], parts[1])
                        };
                        spans.push(Span::Link {
                            label: label.to_owned(),
                            target: target.to_owned(),
                            effect: parts.get(2).map(|value| link_effect(value)),
                            style: style.clone(),
                        });
                    },
                    '<' if parts.len() == 2 => match field_kind(parts[0], parts[1]) {
                        Some(kind) => spans.push(Span::Field {
                            kind,
                            options: parts[0].to_owned(),
                            initial: parts[1].to_owned(),
                            source: raw,
                        }),
                        None => spans.push(Span::Unsupported(raw)),
                    },
                    '{' if !parts[0].is_empty() => spans.push(Span::Partial {
                        target: parts[0].to_owned(),
                        options: parts[1..].iter().map(|part| (*part).to_owned()).collect(),
                        source: raw,
                    }),
                    _ => spans.push(Span::Unsupported(raw)),
                }
            },
            _ => spans.push(Span::Unsupported(source[start..cursor].to_owned())),
        }
    }
    spans
}

fn link_effect(value: &str) -> LinkEffect {
    if let Some(anchor) = value
        .strip_prefix("anchor=")
        .filter(|anchor| !anchor.is_empty() && !anchor.contains('|'))
    {
        LinkEffect::Anchor(anchor.to_owned())
    } else {
        LinkEffect::RequestSelectors(value.to_owned())
    }
}

fn field_kind(options: &str, initial: &str) -> Option<FieldKind> {
    for (prefix, radio) in [("?", false), ("^", true)] {
        if let Some(values) = options.strip_prefix(prefix) {
            let parts: Vec<_> = values.split('|').collect();
            let (empty, name, value, marker) = match parts.as_slice() {
                [empty, name, value] => (*empty, *name, *value, false),
                [empty, name, value, "*"] => (*empty, *name, *value, true),
                _ => return None,
            };
            if !empty.is_empty() || name.is_empty() || value.is_empty() || !initial.is_empty() {
                return None;
            }
            return Some(if radio {
                FieldKind::Radio {
                    name: name.to_owned(),
                    value: value.to_owned(),
                    prechecked: marker,
                }
            } else {
                FieldKind::Checkbox {
                    name: name.to_owned(),
                    value: value.to_owned(),
                    prechecked: marker,
                }
            });
        }
    }

    let (masked, options) = match options.strip_prefix('!') {
        Some(options) => (true, options),
        None => (false, options),
    };
    let (width, rows, name) = if let Some((size, name)) = options.split_once('|') {
        let (width, rows) = parse_field_size(size)?;
        (width, rows, name)
    } else if masked {
        return None;
    } else {
        (None, None, options)
    };
    (!name.is_empty()).then(|| FieldKind::Text {
        name: name.to_owned(),
        width,
        rows,
        masked,
    })
}

fn parse_field_size(size: &str) -> Option<(Option<usize>, Option<usize>)> {
    if size.is_empty() {
        return Some((None, None));
    }
    let (width, rows) = match size.split_once('x') {
        Some((width, rows)) => (width.parse().ok()?, Some(rows.parse().ok()?)),
        None => (size.parse().ok()?, None),
    };
    Some((Some(width), rows))
}

fn color(source: &str) -> Option<([u8; 3], usize)> {
    let bytes = source.as_bytes().get(..3)?;
    let mut color = [0; 3];
    for (channel, byte) in color.iter_mut().zip(bytes) {
        *channel = (*byte as char).to_digit(16)? as u8 * 17;
    }
    Some((color, 3))
}

fn table_options(options: &str) -> bool {
    let width = options.strip_prefix(['l', 'c', 'r']).unwrap_or(options);
    width.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
#[path = "syntax/tests.rs"]
mod tests;
