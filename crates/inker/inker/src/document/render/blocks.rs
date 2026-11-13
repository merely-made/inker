//! Block-level Markdown and Gemini writers.

use super::*;

impl Block {
    pub(super) fn write_markdown(&self, out: &mut String, indent: usize) {
        let pad = "  ".repeat(indent);
        match self {
            Self::Presented {
                presentation,
                block,
            } => block.write_markdown(
                out,
                indent.saturating_add(presentation.indent_level as usize),
            ),
            Self::Heading { level, spans } => {
                let level = (*level).clamp(1, 6) as usize;
                out.push_str(&pad);
                out.push_str(&"#".repeat(level));
                out.push(' ');
                write_inline_markdown(spans, out);
                out.push_str("\n\n");
            },
            Self::Paragraph { spans } => {
                out.push_str(&pad);
                write_inline_markdown(spans, out);
                out.push_str("\n\n");
            },
            Self::CodeBlock { language, text } => {
                out.push_str(&pad);
                out.push_str("```");
                if let Some(lang) = language {
                    out.push_str(lang);
                }
                out.push('\n');
                out.push_str(text);
                if !text.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(&pad);
                out.push_str("```\n\n");
            },
            Self::Quote { blocks } => {
                let mut inner = String::new();
                for block in blocks {
                    block.write_markdown(&mut inner, 0);
                }
                for line in inner.lines() {
                    out.push_str(&pad);
                    out.push_str("> ");
                    out.push_str(line);
                    out.push('\n');
                }
                out.push('\n');
            },
            Self::List { ordered, items } => {
                for (i, item) in items.iter().enumerate() {
                    out.push_str(&pad);
                    if *ordered {
                        out.push_str(&format!("{}. ", i + 1));
                    } else {
                        out.push_str("- ");
                    }
                    let mut first = true;
                    for block in item {
                        if first {
                            // Render the first block inline with the marker
                            // by trimming the leading newline-pair it produces.
                            let mut piece = String::new();
                            block.write_markdown(&mut piece, 0);
                            out.push_str(piece.trim_end());
                            out.push('\n');
                            first = false;
                        } else {
                            block.write_markdown(out, indent + 1);
                        }
                    }
                }
                out.push('\n');
            },
            Self::Image { url, alt } => {
                out.push_str(&pad);
                out.push_str(&format!("![{alt}]({url})\n\n"));
            },
            Self::Preformatted { text } => {
                out.push_str(&pad);
                out.push_str("```\n");
                out.push_str(text);
                if !text.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(&pad);
                out.push_str("```\n\n");
            },
            Self::Rule => out.push_str("---\n\n"),
            Self::FeedHeader {
                title,
                subtitle,
                summary,
                source_url,
            } => {
                out.push_str(&format!("# {title}\n\n"));
                if let Some(subtitle) = subtitle {
                    out.push_str(&format!("## {subtitle}\n\n"));
                }
                if let Some(summary) = summary {
                    out.push_str(summary);
                    out.push_str("\n\n");
                }
                if let Some(url) = source_url {
                    out.push_str(&format!("[Open source]({url})\n\n"));
                }
            },
            Self::FeedEntry {
                title,
                date,
                summary,
                article_url,
                source_url,
                ..
            } => {
                out.push_str(&format!("## {title}\n\n"));
                if let Some(date) = date {
                    out.push_str(&format!("*{date}*\n\n"));
                }
                if let Some(summary) = summary {
                    out.push_str(summary);
                    out.push_str("\n\n");
                }
                if let Some(url) = article_url {
                    out.push_str(&format!("[Open article]({url})\n\n"));
                }
                if let Some(url) = source_url {
                    out.push_str(&format!("[Open source]({url})\n\n"));
                }
            },
            Self::MetadataRow { label, value } => {
                out.push_str(&format!("**{label}:** {value}\n\n"));
            },
            Self::Badge { text } => {
                out.push_str(&format!("> *{text}*\n\n"));
            },
            Self::Table {
                alignments,
                header,
                rows,
            } => {
                write_markdown_table(alignments, header, rows, out, &pad);
            },
            Self::Menu { rows } => {
                for block in menu_fallback_blocks(rows) {
                    block.write_markdown(out, indent);
                }
            },
        }
    }

    pub(super) fn write_gemini(&self, out: &mut String) {
        match self {
            Self::Presented { block, .. } => block.write_gemini(out),
            Self::Menu { rows } => {
                for block in menu_fallback_blocks(rows) {
                    block.write_gemini(out);
                }
            },
            Self::Table { header, rows, .. } => {
                out.push_str("```\n");
                for line in table_lines(header, rows) {
                    out.push_str(&line);
                    out.push('\n');
                }
                out.push_str("```\n");
            },
            Self::Heading { level, spans } => {
                let prefix = match level {
                    1 => "# ",
                    2 => "## ",
                    _ => "### ",
                };
                out.push_str(prefix);
                out.push_str(&inline_text(spans));
                out.push('\n');
            },
            Self::Paragraph { spans } => {
                // A link-only paragraph (e.g. a parsed `=>` line, or a bare
                // link on its own line) IS the link line — emitting its text
                // first would double it.
                let text = inline_text(spans);
                if !text.is_empty() && !is_link_only(spans) && !is_submit_only(spans) {
                    out.push_str(&text);
                    out.push('\n');
                }
                // Surface inline links as separate gemtext link lines after
                // the paragraph text.
                let mut links = Vec::new();
                for span in spans {
                    collect_link_targets(span, &mut links);
                }
                for (url, label) in links {
                    out.push_str("=> ");
                    out.push_str(&url);
                    if !label.is_empty() && label != url {
                        out.push(' ');
                        out.push_str(&label);
                    }
                    out.push('\n');
                }
                let mut submissions = Vec::new();
                for span in spans {
                    collect_submit_targets(span, &mut submissions);
                }
                for (target, label) in submissions {
                    out.push_str("=: ");
                    out.push_str(&target);
                    if !label.is_empty() && label != target {
                        out.push(' ');
                        out.push_str(&label);
                    }
                    out.push('\n');
                }
            },
            Self::CodeBlock { language, text } => {
                out.push_str("```");
                if let Some(lang) = language {
                    out.push_str(lang);
                }
                out.push('\n');
                out.push_str(text);
                if !text.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str("```\n");
            },
            Self::Quote { blocks } => {
                let mut inner = String::new();
                for block in blocks {
                    block.write_gemini(&mut inner);
                }
                for line in inner.lines() {
                    out.push_str("> ");
                    out.push_str(line);
                    out.push('\n');
                }
            },
            Self::List { items, .. } => {
                for item in items {
                    let mut inner = String::new();
                    for block in item {
                        block.write_gemini(&mut inner);
                    }
                    let trimmed = inner.trim();
                    out.push_str("* ");
                    out.push_str(trimmed);
                    out.push('\n');
                }
            },
            Self::Image { url, alt } => {
                out.push_str("=> ");
                out.push_str(url);
                if !alt.is_empty() {
                    out.push(' ');
                    out.push_str(alt);
                }
                out.push('\n');
            },
            Self::Preformatted { text } => {
                out.push_str("```\n");
                out.push_str(text);
                if !text.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str("```\n");
            },
            Self::Rule => out.push('\n'),
            Self::FeedHeader {
                title,
                subtitle,
                summary,
                source_url,
            } => {
                out.push_str(&format!("# {title}\n"));
                if let Some(subtitle) = subtitle {
                    out.push_str(&format!("## {subtitle}\n"));
                }
                if let Some(summary) = summary {
                    out.push_str(summary);
                    out.push('\n');
                }
                if let Some(url) = source_url {
                    out.push_str(&format!("=> {url} Open source\n"));
                }
            },
            Self::FeedEntry {
                title,
                date,
                summary,
                article_url,
                source_url,
                ..
            } => {
                out.push_str(&format!("## {title}\n"));
                if let Some(date) = date {
                    out.push_str(&format!("> {date}\n"));
                }
                if let Some(summary) = summary {
                    out.push_str(summary);
                    out.push('\n');
                }
                if let Some(url) = article_url {
                    out.push_str(&format!("=> {url} Open article\n"));
                }
                if let Some(url) = source_url {
                    out.push_str(&format!("=> {url} Open source\n"));
                }
            },
            Self::MetadataRow { label, value } => {
                out.push_str(&format!("{label}: {value}\n"));
            },
            Self::Badge { text } => {
                out.push_str(&format!("> {text}\n"));
            },
        }
    }
}
