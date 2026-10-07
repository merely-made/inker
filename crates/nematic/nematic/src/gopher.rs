// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Gopher menu engine — parses RFC 1436 gopher menus into a portable
//! document.
//!
//! A gopher menu is a sequence of tab-delimited lines, each of the form:
//!
//! ```text
//! <type><display>\t<selector>\t<host>\t<port>\r\n
//! ```
//!
//! The first character of each line is the item type. A bare `.` on its own
//! line terminates the menu.
//!
//! The menu lowers to one [`Block::Menu`]: every item is a row naming its
//! portable kind and keeping its raw type character, so the type column and
//! the fixed-width alignment survive to every host (smolweb fidelity plan,
//! WS4 R2).
//!
//! - `i` informational text and `3` server errors: rows without a target
//! - `7` full-text search: a row whose label is an [`InlineSpan::Submit`]
//! - `h` URL items: an external row targeting the extracted URL
//! - every other type: a row targeting its synthesised `gopher://` URL
//!
//! The raw character comes from today's gopher-protocol kinds: a known kind
//! maps to the character that produced it (an image to `I`, since `g` and `I`
//! share a kind), and an unknown kind keeps its own. Telnet `8` and tn3270
//! `T` both read as telnet here, whatever the grammar's coarse kind says.
//!
//! References:
//! - RFC 1436 (The Internet Gopher Protocol)
//! - RFC 4266 (gopher URI scheme)

use errand::parse::gopher::{GopherItem, GopherKind, parse as parse_gopher};
use inker::{
    Block, DocumentProvenance, DocumentTrustState, Engine, EngineDocument, EngineError,
    EngineInput, InlineSpan, MenuItemKind, MenuRow,
};

/// Stable engine identifier.
pub const ENGINE_ID: &str = "nematic.gopher";

/// Gopher menu engine.
pub struct GopherEngine;

impl GopherEngine {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GopherEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine for GopherEngine {
    fn engine_id(&self) -> &str {
        ENGINE_ID
    }

    fn render(&self, input: &EngineInput) -> Result<EngineDocument, EngineError> {
        // errand parses the RFC 1436 menu into typed items (with synthesised
        // URLs); each becomes one typed row of a single menu block.
        let rows: Vec<MenuRow> = parse_gopher(&input.body)
            .into_iter()
            .filter_map(menu_row)
            .collect();
        let blocks = if rows.is_empty() {
            Vec::new()
        } else {
            vec![Block::Menu { rows }]
        };

        Ok(EngineDocument {
            address: input.address.clone(),
            title: None,
            content_type: input
                .content_type
                .clone()
                .unwrap_or_else(|| "application/gopher-menu".to_string()),
            lang: None,
            provenance: DocumentProvenance::for_engine(self.engine_id(), &input.address),
            trust: DocumentTrustState::Unknown,
            diagnostics: Vec::new(),
            navigation: Default::default(),
            blocks,
        })
    }
}

/// One parsed item as a menu row. `None` for a resource item without a URL,
/// which the grammar never yields but which would have nowhere to go.
fn menu_row(item: GopherItem) -> Option<MenuRow> {
    let (kind, marker) = kind_and_marker(&item.kind);
    let text = vec![InlineSpan::Text(item.display)];
    let (label, target) = match kind {
        MenuItemKind::Info | MenuItemKind::Error => (text, None),
        MenuItemKind::Search => (
            vec![InlineSpan::Submit {
                target: item.url?,
                spans: text,
            }],
            None,
        ),
        _ => (text, Some(item.url?)),
    };
    Some(MenuRow {
        kind,
        marker: Some(marker),
        label,
        target,
    })
}

/// The portable kind and raw type character for a grammar kind.
fn kind_and_marker(kind: &GopherKind) -> (MenuItemKind, char) {
    match kind {
        GopherKind::Info => (MenuItemKind::Info, 'i'),
        GopherKind::Error => (MenuItemKind::Error, '3'),
        GopherKind::Text => (MenuItemKind::Document, '0'),
        GopherKind::Submenu => (MenuItemKind::Directory, '1'),
        GopherKind::Search => (MenuItemKind::Search, '7'),
        GopherKind::Binary => (MenuItemKind::Binary, '9'),
        GopherKind::Image => (MenuItemKind::Image, 'I'),
        GopherKind::Sound => (MenuItemKind::Sound, 's'),
        GopherKind::Telnet => (MenuItemKind::Telnet, 'T'),
        GopherKind::Url => (MenuItemKind::External, 'h'),
        GopherKind::Other(c) => (other_kind(*c), *c),
    }
}

/// RFC 1436 and common gopher+ types the grammar files under `Other`.
fn other_kind(c: char) -> MenuItemKind {
    match c {
        '8' => MenuItemKind::Telnet,
        '4' | '5' | '6' => MenuItemKind::Binary,
        ':' | 'p' => MenuItemKind::Image,
        'd' | 'P' | 'r' => MenuItemKind::Document,
        _ => MenuItemKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(body: &str) -> EngineDocument {
        GopherEngine::new()
            .render(&EngineInput::new("gopher://test/", body))
            .expect("render")
    }

    fn line(t: char, display: &str, selector: &str, host: &str, port: &str) -> String {
        format!("{t}{display}\t{selector}\t{host}\t{port}\r\n")
    }

    #[test]
    fn engine_id_is_stable() {
        assert_eq!(GopherEngine::new().engine_id(), "nematic.gopher");
    }

    /// The document's menu rows; panics unless the document is one menu.
    fn rows(doc: &EngineDocument) -> &[MenuRow] {
        let [Block::Menu { rows }] = doc.blocks.as_slice() else {
            panic!("expected one menu block, got {:?}", doc.blocks);
        };
        rows
    }

    #[test]
    fn standard_text_item_synthesises_gopher_url() {
        let body = line('0', "Welcome text", "/welcome.txt", "example.test", "70");
        let doc = render(&body);
        let [row] = rows(&doc) else {
            panic!("one row");
        };
        assert_eq!(row.kind, MenuItemKind::Document);
        assert_eq!(row.marker, Some('0'));
        assert_eq!(
            row.target.as_deref(),
            Some("gopher://example.test/0/welcome.txt")
        );
    }

    #[test]
    fn non_default_port_appears_in_url() {
        let body = line('1', "Sub", "/sub", "example.test", "7070");
        let doc = render(&body);
        let [row] = rows(&doc) else {
            panic!("one row");
        };
        assert_eq!(row.kind, MenuItemKind::Directory);
        assert_eq!(
            row.target.as_deref(),
            Some("gopher://example.test:7070/1/sub")
        );
    }

    #[test]
    fn url_item_extracts_url_prefix() {
        let body = line('h', "External", "URL:https://example.test/", ".", "70");
        let doc = render(&body);
        let [row] = rows(&doc) else {
            panic!("one row");
        };
        assert_eq!(row.kind, MenuItemKind::External);
        assert_eq!(row.marker, Some('h'));
        assert_eq!(row.target.as_deref(), Some("https://example.test/"));
    }

    #[test]
    fn info_lines_remain_literal_menu_rows() {
        let body = format!(
            "{}{}{}",
            line('i', "Welcome to the menu", "", "example.test", "70"),
            line('i', "More info on a second line", "", "example.test", "70"),
            line('i', "Final info line", "", "example.test", "70"),
        );
        let doc = render(&body);
        let rows = rows(&doc);
        assert_eq!(rows.len(), 3);
        for row in rows {
            assert_eq!(row.kind, MenuItemKind::Info);
            assert_eq!(row.marker, Some('i'));
            assert_eq!(row.target, None);
        }
    }

    #[test]
    fn info_then_resource_then_info_is_one_menu_of_three_rows() {
        let body = format!(
            "{}{}{}",
            line('i', "header", "", "example.test", "70"),
            line('1', "submenu", "/sub", "example.test", "70"),
            line('i', "footer", "", "example.test", "70"),
        );
        let doc = render(&body);
        let kinds: Vec<_> = rows(&doc).iter().map(|row| row.kind).collect();
        assert_eq!(
            kinds,
            [
                MenuItemKind::Info,
                MenuItemKind::Directory,
                MenuItemKind::Info
            ]
        );
    }

    #[test]
    fn period_terminator_stops_parsing() {
        let body = format!(
            "{}{}{}",
            line('i', "before", "", "example.test", "70"),
            ".\r\n",
            line('1', "should-not-appear", "/x", "example.test", "70"),
        );
        let doc = render(&body);
        assert_eq!(rows(&doc).len(), 1);
    }

    #[test]
    fn error_lines_are_untargeted_error_rows() {
        let body = line('3', "host unreachable", "", "example.test", "70");
        let doc = render(&body);
        let [row] = rows(&doc) else {
            panic!("one row");
        };
        assert_eq!(row.kind, MenuItemKind::Error);
        assert_eq!(row.marker, Some('3'));
        assert_eq!(row.target, None);
        assert_eq!(inker::inline_text(&row.label), "host unreachable");
    }

    #[test]
    fn search_item_submits_through_its_label() {
        let body = line('7', "Search the hole", "/find", "example.test", "70");
        let doc = render(&body);
        let [row] = rows(&doc) else {
            panic!("one row");
        };
        assert_eq!(row.kind, MenuItemKind::Search);
        assert_eq!(row.marker, Some('7'));
        assert_eq!(row.target, None, "a search submits, it does not navigate");
        let [InlineSpan::Submit { target, spans }] = row.label.as_slice() else {
            panic!("a submit label: {:?}", row.label);
        };
        assert_eq!(target, "gopher://example.test/7/find");
        assert_eq!(inker::inline_text(spans), "Search the hole");
    }

    #[test]
    fn telnet_8_and_tn3270_t_both_read_as_telnet() {
        let body = format!(
            "{}{}",
            line('8', "telnet", "", "example.test", "23"),
            line('T', "tn3270", "", "example.test", "23"),
        );
        let doc = render(&body);
        let got: Vec<_> = rows(&doc)
            .iter()
            .map(|row| (row.kind, row.marker))
            .collect();
        assert_eq!(
            got,
            [
                (MenuItemKind::Telnet, Some('8')),
                (MenuItemKind::Telnet, Some('T')),
            ]
        );
    }

    #[test]
    fn an_unknown_type_keeps_its_marker() {
        let body = line('2', "CSO phone book", "", "example.test", "105");
        let doc = render(&body);
        let [row] = rows(&doc) else {
            panic!("one row");
        };
        assert_eq!(row.kind, MenuItemKind::Other);
        assert_eq!(row.marker, Some('2'));
        assert!(row.target.is_some(), "still navigable");
    }

    #[test]
    fn resource_with_missing_host_is_skipped() {
        // Tab-delimited but with empty host — malformed but realistic.
        let body = "1Bad item\t/sel\t\t70\r\n";
        let doc = render(body);
        assert!(doc.blocks.is_empty());
    }

    #[test]
    fn outgoing_links_collect_all_resource_urls() {
        let body = format!(
            "{}{}{}",
            line('i', "header", "", ".", "70"),
            line('1', "sub", "/sub", "example.test", "70"),
            line('h', "ext", "URL:https://example.test/", ".", "70"),
        );
        let doc = render(&body);
        let urls = doc.outgoing_links();
        assert_eq!(
            urls,
            vec!["gopher://example.test/1/sub", "https://example.test/"]
        );
    }

    #[test]
    fn dispatches_through_inker_registry() {
        use inker::EngineRegistry;
        use inker::routing::{
            EngineRouteDecision, SurfaceContract, SurfaceContractMode, SurfaceTargetId,
        };

        let mut registry = EngineRegistry::new();
        registry.register(Box::new(GopherEngine::new()));
        let decision = EngineRouteDecision {
            engine_id: ENGINE_ID.to_string(),
            surface_contract: SurfaceContract {
                target: SurfaceTargetId::new("gopher:1"),
                mode: SurfaceContractMode::CompositedTexture,
            },
        };
        let body = line('1', "Submenu", "/s", "example.test", "70");
        let doc = registry
            .dispatch(
                &decision,
                &EngineInput::new("gopher://example.test/", &body),
            )
            .expect("dispatch");
        assert_eq!(doc.outgoing_links(), vec!["gopher://example.test/1/s"]);
    }
}
