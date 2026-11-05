// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Host-neutral clipping of selected and engine-generated content.

use image::ImageEncoder;
use inker::{
    Block, EngineDocument, EngineInput, EngineRegistry, EngineRoutePolicy, EngineRouteRequest,
    InlineSpan, WorkspaceRouteId,
};
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq)]
pub struct ClipFragment {
    pub source_url: String,
    pub title: Option<String>,
    pub text: String,
    pub html: Option<String>,
    pub selector: Option<String>,
    pub links: Vec<String>,
    pub rect: Option<ClipRect>,
    pub visual: Option<ClipVisual>,
    blocks: Option<Vec<Block>>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ClipRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClipVisual {
    pub png_bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub data_uri: String,
}

#[derive(Deserialize)]
struct WebClipPayload {
    ok: Option<bool>,
    error: Option<String>,
    title: Option<String>,
    #[serde(rename = "pageTitle")]
    page_title: Option<String>,
    text: Option<String>,
    html: Option<String>,
    selector: Option<String>,
    #[serde(default)]
    links: Vec<String>,
    rect: Option<ClipRect>,
}

pub fn web_clip_script(x: i32, y: i32) -> String {
    format!(
        r##"(function() {{
  const rawX = {x};
  const rawY = {y};
  const scale = window.devicePixelRatio || 1;
  const x = rawX / scale;
  const y = rawY / scale;
  function cssPath(el) {{
    const parts = [];
    while (el && el.nodeType === 1 && el !== document.documentElement) {{
      let part = el.localName;
      if (el.id) {{
        part += "#" + CSS.escape(el.id);
        parts.unshift(part);
        break;
      }}
      const cls = Array.from(el.classList || []).slice(0, 2).map(c => "." + CSS.escape(c)).join("");
      part += cls;
      const parent = el.parentElement;
      if (parent) {{
        const same = Array.from(parent.children).filter(n => n.localName === el.localName);
        if (same.length > 1) {{
          part += ":nth-of-type(" + (same.indexOf(el) + 1) + ")";
        }}
      }}
      parts.unshift(part);
      el = parent;
    }}
    return parts.join(" > ");
  }}
  let el = document.elementFromPoint(x, y);
  if (!el) return JSON.stringify({{ ok: false, error: "no element at cursor" }});
  if (el.nodeType !== 1) el = el.parentElement;
  const clone = el.cloneNode(true);
  clone.querySelectorAll("script,style,noscript,iframe,object,embed").forEach(n => n.remove());
  const rect = el.getBoundingClientRect();
  const heading = el.querySelector("h1,h2,h3,[role='heading']");
  const title = ((heading && heading.textContent) ||
    el.getAttribute("aria-label") ||
    el.getAttribute("alt") ||
    document.title ||
    location.href ||
    "").trim();
  const links = Array.from(el.querySelectorAll("a[href]")).slice(0, 50).map(a => a.href);
  return JSON.stringify({{
    ok: true,
    title,
    pageTitle: document.title || "",
    text: (el.innerText || el.textContent || "").trim(),
    html: clone.outerHTML,
    selector: cssPath(el),
    links,
    rect: {{ x: rect.x, y: rect.y, width: rect.width, height: rect.height }}
  }});
}})()"##
    )
}

pub fn parse_web_clip(raw: &str, source_url: &str) -> Result<ClipFragment, String> {
    let payload = parse_payload(raw)?;
    if payload.ok == Some(false) {
        return Err(payload
            .error
            .unwrap_or_else(|| "selected element could not be captured".to_string()));
    }
    let title = first_nonempty([payload.title, payload.page_title]);
    let mut text = payload
        .text
        .as_deref()
        .map(clean_multiline)
        .unwrap_or_default();
    if text.is_empty() {
        text = title.clone().unwrap_or_default();
    }
    if text.is_empty() {
        return Err("selected element had no text".to_string());
    }
    Ok(ClipFragment {
        source_url: source_url.to_string(),
        title,
        text,
        html: payload.html.filter(|html| !html.trim().is_empty()),
        selector: payload
            .selector
            .filter(|selector| !selector.trim().is_empty()),
        links: payload
            .links
            .into_iter()
            .filter(|link| !link.trim().is_empty())
            .collect(),
        rect: payload.rect,
        visual: None,
        blocks: None,
    })
}

pub fn fragment_from_body(
    source_url: &str,
    title: Option<String>,
    content_type: Option<&str>,
    body: &str,
    registry: &EngineRegistry,
    policy: &EngineRoutePolicy,
) -> ClipFragment {
    if let Some(doc) = engine_document(source_url, content_type, body, registry, policy)
        && !doc.blocks.is_empty()
    {
        let text = clean_multiline(&blocks_text(&doc.blocks));
        if !text.is_empty() {
            let links = block_links(&doc.blocks);
            return ClipFragment {
                source_url: source_url.to_string(),
                title: first_nonempty([doc.title.clone(), title]),
                text,
                html: None,
                selector: None,
                links,
                rect: None,
                visual: None,
                blocks: Some(doc.blocks),
            };
        }
    }

    let text = clean_multiline(&document_text(content_type, body));
    ClipFragment {
        source_url: source_url.to_string(),
        title: title.filter(|title| !title.trim().is_empty()),
        text,
        html: None,
        selector: None,
        links: Vec::new(),
        rect: None,
        visual: None,
        blocks: None,
    }
}

/// Build a host-neutral semantic clip when a retained document lane exposes
/// selected or document text rather than raw response bytes.
pub fn fragment_from_text(
    source_url: impl Into<String>,
    title: Option<String>,
    text: impl Into<String>,
    selector: Option<String>,
    links: Vec<String>,
) -> ClipFragment {
    ClipFragment {
        source_url: source_url.into(),
        title: title.filter(|title| !title.trim().is_empty()),
        text: clean_multiline(&text.into()),
        html: None,
        selector: selector.filter(|selector| !selector.trim().is_empty()),
        links: links
            .into_iter()
            .filter(|link| !link.trim().is_empty())
            .collect(),
        rect: None,
        visual: None,
        blocks: None,
    }
}

pub fn clip_title(fragment: &ClipFragment) -> String {
    let base = fragment
        .title
        .as_deref()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or(&fragment.source_url);
    let mut title = format!("Clip: {base}");
    if title.chars().count() > 96 {
        title = title.chars().take(93).collect::<String>();
        title.push_str("...");
    }
    title
}

fn parse_payload(raw: &str) -> Result<WebClipPayload, String> {
    let trimmed = raw.trim();
    if let Ok(decoded) = serde_json::from_str::<String>(trimmed) {
        serde_json::from_str(&decoded).map_err(|err| err.to_string())
    } else {
        serde_json::from_str(trimmed).map_err(|err| err.to_string())
    }
}

fn engine_document(
    source_url: &str,
    content_type: Option<&str>,
    body: &str,
    registry: &EngineRegistry,
    policy: &EngineRoutePolicy,
) -> Option<EngineDocument> {
    let request = EngineRouteRequest {
        workspace_id: WorkspaceRouteId::new("meerkat"),
        view: None,
        node: None,
        address: source_url.to_string(),
        content_type: content_type.map(str::to_string),
        pinned_engine: None,
    };
    let decision = policy.route_filtered(&request, |id| {
        registry.contains(id) || id == inker::routing::ENGINE_GENET_WEB
    });
    if decision.engine_id == inker::routing::ENGINE_GENET_WEB {
        return None;
    }
    let mut input = EngineInput::new(source_url, body.to_string());
    if let Some(content_type) = content_type {
        input = input.with_content_type(content_type.to_string());
    }
    registry.dispatch(&decision, &input).ok()
}

fn document_text(content_type: Option<&str>, body: &str) -> String {
    if content_type
        .map(|ct| ct.to_ascii_lowercase().contains("html"))
        .unwrap_or(false)
    {
        let doc = genet_static_dom::StaticDocument::parse(body);
        if let Some(text) = crate::extract_main_text(&doc) {
            return text;
        }
    }
    body.to_string()
}

/// Semantic blocks of the clip, including its optional visual.
/// The receiving authoring or graph adapter chooses the destination format.
pub fn fragment_blocks(fragment: &ClipFragment) -> Vec<Block> {
    if let Some(source_blocks) = &fragment.blocks {
        let mut blocks = Vec::new();
        if let Some(visual) = &fragment.visual {
            blocks.push(visual_block(fragment, visual));
        }
        blocks.extend(source_blocks.clone());
        return blocks;
    }

    let mut blocks = Vec::new();
    if let Some(title) = fragment
        .title
        .as_ref()
        .filter(|title| !title.trim().is_empty())
    {
        blocks.push(Block::Heading {
            level: 1,
            spans: vec![InlineSpan::Text(title.clone())],
        });
    }
    if let Some(visual) = &fragment.visual {
        blocks.push(visual_block(fragment, visual));
    }
    blocks.push(Block::Paragraph {
        spans: vec![InlineSpan::Text(fragment.text.clone())],
    });
    if !fragment.links.is_empty() {
        blocks.push(Block::List {
            ordered: false,
            items: fragment
                .links
                .iter()
                .take(20)
                .map(|url| {
                    vec![Block::Paragraph {
                        spans: vec![InlineSpan::Link {
                            url: url.clone(),
                            title: None,
                            spans: vec![InlineSpan::Text(url.clone())],
                            predicate: None,
                        }],
                    }]
                })
                .collect(),
        });
    }
    blocks
}

fn blocks_text(blocks: &[Block]) -> String {
    let mut out = String::new();
    for block in blocks {
        append_block_text(block, &mut out);
    }
    out
}

fn append_block_text(block: &Block, out: &mut String) {
    match block {
        Block::Presented { block, .. } => append_block_text(block, out),
        Block::Heading { spans, .. } | Block::Paragraph { spans } => {
            push_text(out, &inker::inline_text(spans));
        },
        Block::CodeBlock { text, .. } | Block::Preformatted { text } => push_text(out, text),
        Block::Quote { blocks } => {
            for block in blocks {
                append_block_text(block, out);
            }
        },
        Block::List { items, .. } => {
            for item in items {
                for block in item {
                    append_block_text(block, out);
                }
            }
        },
        Block::Image { alt, .. } => push_text(out, alt),
        Block::Rule => {},
        Block::FeedHeader {
            title,
            subtitle,
            summary,
            ..
        } => {
            push_text(out, title);
            if let Some(subtitle) = subtitle {
                push_text(out, subtitle);
            }
            if let Some(summary) = summary {
                push_text(out, summary);
            }
        },
        Block::FeedEntry {
            title,
            date,
            summary,
            ..
        } => {
            push_text(out, title);
            if let Some(date) = date {
                push_text(out, date);
            }
            if let Some(summary) = summary {
                push_text(out, summary);
            }
        },
        Block::MetadataRow { label, value } => push_text(out, &format!("{label}: {value}")),
        Block::Badge { text } => push_text(out, text),
        Block::Table { header, rows, .. } => {
            for cell in header {
                push_text(out, &inker::inline_text(cell));
            }
            for row in rows {
                for cell in row {
                    push_text(out, &inker::inline_text(cell));
                }
            }
        },
        Block::Menu { rows } => {
            for row in rows {
                push_text(out, &inker::inline_text(&row.label));
            }
        },
        // A kind this extractor does not know yet leaves a named marker
        // rather than vanishing from the clip's text.
        other => push_text(out, &format!("[unsupported block: {}]", other.kind_name())),
    }
}

fn push_text(out: &mut String, text: &str) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(text);
}

fn block_links(blocks: &[Block]) -> Vec<String> {
    let mut links = Vec::new();
    for block in blocks {
        collect_block_links(block, &mut links);
    }
    links.sort();
    links.dedup();
    links
}

fn collect_block_links(block: &Block, links: &mut Vec<String>) {
    match block {
        Block::Presented { block, .. } => collect_block_links(block, links),
        Block::Heading { spans, .. } | Block::Paragraph { spans } => {
            collect_span_links(spans, links);
        },
        Block::Quote { blocks } => {
            for block in blocks {
                collect_block_links(block, links);
            }
        },
        Block::List { items, .. } => {
            for item in items {
                for block in item {
                    collect_block_links(block, links);
                }
            }
        },
        Block::Image { url, .. } => links.push(url.clone()),
        Block::FeedHeader { source_url, .. } => push_optional_link(source_url, links),
        Block::FeedEntry {
            article_url,
            source_url,
            ..
        } => {
            push_optional_link(article_url, links);
            push_optional_link(source_url, links);
        },
        Block::Table { header, rows, .. } => {
            for cell in header {
                collect_span_links(cell, links);
            }
            for row in rows {
                for cell in row {
                    collect_span_links(cell, links);
                }
            }
        },
        Block::Menu { rows } => {
            for row in rows {
                push_optional_link(&row.target, links);
                collect_span_links(&row.label, links);
            }
        },
        Block::CodeBlock { .. }
        | Block::Preformatted { .. }
        | Block::Rule
        | Block::MetadataRow { .. }
        | Block::Badge { .. } => {},
        // No links to take from a kind this extractor does not know yet; its
        // text keeps a named marker (see `append_block_text`).
        _ => {},
    }
}

fn collect_span_links(spans: &[InlineSpan], links: &mut Vec<String>) {
    for span in spans {
        match span {
            InlineSpan::Link {
                url, spans: inner, ..
            } => {
                links.push(url.clone());
                collect_span_links(inner, links);
            },
            InlineSpan::Presented { spans: inner, .. }
            | InlineSpan::Emphasis(inner)
            | InlineSpan::Strong(inner)
            | InlineSpan::Submit { spans: inner, .. }
            | InlineSpan::InPage { spans: inner, .. } => {
                collect_span_links(inner, links);
            },
            InlineSpan::Text(_)
            | InlineSpan::Code(_)
            | InlineSpan::LineBreak
            | InlineSpan::SoftBreak => {},
        }
    }
}

fn push_optional_link(link: &Option<String>, links: &mut Vec<String>) {
    if let Some(link) = link.as_ref().filter(|link| !link.trim().is_empty()) {
        links.push(link.clone());
    }
}

fn clean_multiline(input: &str) -> String {
    let mut out = String::new();
    let mut previous_blank = true;
    for line in input.lines().map(str::trim) {
        if line.is_empty() {
            if !previous_blank {
                out.push('\n');
                previous_blank = true;
            }
            continue;
        }
        if !out.is_empty() && !previous_blank {
            out.push('\n');
        }
        out.push_str(line);
        previous_blank = false;
    }
    out.trim().to_string()
}

fn first_nonempty(values: impl IntoIterator<Item = Option<String>>) -> Option<String> {
    values
        .into_iter()
        .flatten()
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "clip/tests.rs"]
mod tests;

#[path = "clip/visual.rs"]
mod visual;

pub use visual::attach_cropped_visual;

use visual::encode_png_rgba;

use visual::visual_block;
