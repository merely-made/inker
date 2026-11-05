// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use inker::session_engine::{DocumentSession, SessionClick};
use inker::{
    DocumentA11yAction, DocumentA11yActionRequest, DocumentA11yNode, DocumentA11yProjection,
    DocumentA11yRole, InlineSpan,
};

use crate::{SmolwebDocument, SmolwebDocumentSession, SmolwebTheme};

const GUIDE: &str = "# Field guide\n\
        Read the intro first.\n\
        => gemini://x.test/next Next page\n\
        => gemini://x.test/other Other page\n\
        * first item\n\
        * second item\n\
        > a quoted line\n";

fn session(url: &str, body: &str, size: (u32, u32)) -> SmolwebDocumentSession {
    SmolwebDocumentSession::new(SmolwebDocument::parse(url, body, SmolwebTheme::Plain), size)
}

fn framed(url: &str, body: &str, size: (u32, u32)) -> SmolwebDocumentSession {
    let mut session = session(url, body, size);
    let _ = session.frame(size.0, size.1);
    session
}

/// Names in reading order, depth first from the root.
fn reading_order(projection: &DocumentA11yProjection) -> Vec<String> {
    fn walk(projection: &DocumentA11yProjection, node: &DocumentA11yNode, out: &mut Vec<String>) {
        if let Some(name) = &node.name {
            out.push(name.clone());
        }
        for child in &node.children {
            walk(
                projection,
                projection.node(*child).expect("child exists"),
                out,
            );
        }
    }
    let mut out = Vec::new();
    walk(
        projection,
        projection.node(projection.root()).expect("root"),
        &mut out,
    );
    out
}

fn named<'a>(projection: &'a DocumentA11yProjection, name: &str) -> &'a DocumentA11yNode {
    projection
        .nodes()
        .iter()
        .find(|node| node.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no node named {name:?}"))
}

fn parent_role(projection: &DocumentA11yProjection, node: &DocumentA11yNode) -> DocumentA11yRole {
    projection
        .node(node.parent.expect("has a parent"))
        .expect("parent exists")
        .role
}

#[test]
fn nothing_is_published_before_a_frame() {
    let session = session("gemini://x.test/guide.gmi", GUIDE, (400, 600));
    assert!(session.accessibility_projection().is_none());
}

#[test]
fn a_gemini_page_reads_in_order_with_roles_and_boxes() {
    let session = framed("gemini://x.test/guide.gmi", GUIDE, (400, 600));
    let projection = session
        .accessibility_projection()
        .expect("a framed page projects");
    let root = projection.node(projection.root()).expect("root");
    assert_eq!(root.role, DocumentA11yRole::Document);
    assert_eq!(
        reading_order(&projection)[1..],
        [
            "Field guide",
            "Read the intro first.",
            "Next page",
            "Other page",
            "first item",
            "second item",
            "a quoted line",
        ]
    );

    assert_eq!(
        root.name.as_deref(),
        Some("Field guide"),
        "the page's title"
    );
    let heading = projection
        .nodes()
        .iter()
        .find(|node| node.role == DocumentA11yRole::Heading { level: 1 })
        .expect("the heading");
    assert_eq!(heading.name.as_deref(), Some("Field guide"));
    let link = named(&projection, "Next page");
    assert_eq!(link.role, DocumentA11yRole::Link);
    assert_eq!(
        link.actions,
        [
            DocumentA11yAction::Click,
            DocumentA11yAction::Focus,
            DocumentA11yAction::ScrollIntoView
        ]
    );
    let item = named(&projection, "first item");
    let list_item = projection.node(item.parent.unwrap()).unwrap();
    assert_eq!(list_item.role, DocumentA11yRole::ListItem);
    assert_eq!(parent_role(&projection, list_item), DocumentA11yRole::List);

    for node in projection.nodes() {
        let bounds = node
            .bounds
            .unwrap_or_else(|| panic!("{:?} {:?} has no box", node.role, node.name));
        assert!(bounds.width > 0.0 && bounds.height > 0.0);
    }
    let heading_box = heading.bounds.unwrap();
    let link_box = link.bounds.unwrap();
    assert!(heading_box.y < link_box.y, "boxes follow the page");
}

/// A link inside a sentence is its own node between the text around it,
/// so a reader hears the sentence once and the link once.
#[test]
fn an_inline_link_is_read_once_between_its_text() {
    let mut doc = SmolwebDocument::parse("gemini://x.test/a.gmi", "x", SmolwebTheme::Plain);
    let mut document = doc.document().clone();
    document.blocks = vec![inker::Block::Paragraph {
        spans: vec![
            InlineSpan::Text("Read the ".into()),
            InlineSpan::Link {
                url: "gemini://x.test/guide".into(),
                title: None,
                spans: vec![InlineSpan::Text("guide".into())],
                predicate: None,
            },
            InlineSpan::Text(" first.".into()),
        ],
    }];
    doc.replace_document(document);
    let mut session = SmolwebDocumentSession::new(doc, (400, 300));
    let _ = session.frame(400, 300);
    let projection = session.accessibility_projection().expect("projects");
    assert_eq!(
        reading_order(&projection)[1..],
        ["Read the", "guide", "first."]
    );
    let paragraph = projection
        .nodes()
        .iter()
        .find(|node| node.role == DocumentA11yRole::Paragraph)
        .expect("the paragraph");
    assert_eq!(
        paragraph.name, None,
        "the paragraph does not repeat its text"
    );
    assert_eq!(paragraph.children.len(), 3);
}

/// Click goes through the session's own pointer path; Focus moves the
/// session's keyboard focus; ids outlive a redraw.
#[test]
fn click_and_focus_act_on_the_page() {
    let mut session = framed("gemini://x.test/guide.gmi", GUIDE, (400, 600));
    let projection = session.accessibility_projection().unwrap();
    let next = named(&projection, "Next page").id;
    let other = named(&projection, "Other page").id;

    let target = session
        .accessibility_click_target(next)
        .expect("a visible link has a click point");
    assert_eq!(target.revision, projection.revision());
    assert_eq!(
        session.click_at(target.point.x, target.point.y),
        SessionClick::Navigate("gemini://x.test/next".into())
    );

    assert!(
        session.dispatch_accessibility_action(&DocumentA11yActionRequest {
            revision: projection.revision(),
            target: other,
            action: DocumentA11yAction::Focus,
            data: None,
        })
    );
    let _ = session.frame(400, 600);
    let focused = session.accessibility_projection().unwrap();
    assert!(focused.node(other).unwrap().state.focused);
    assert!(!focused.node(next).unwrap().state.focused);
    assert_ne!(
        focused.revision(),
        projection.revision(),
        "focus moves the revision"
    );
    assert_eq!(
        focused.node(next).unwrap().name.as_deref(),
        Some("Next page")
    );

    assert!(
        !session.dispatch_accessibility_action(&DocumentA11yActionRequest {
            revision: focused.revision(),
            target: next,
            action: DocumentA11yAction::Click,
            data: None,
        }),
        "Click travels by the pointer path, not dispatch"
    );
}

/// A node below the fold keeps its true place; ScrollIntoView brings it
/// into the viewport; an action against the old revision is refused.
#[test]
fn scroll_into_view_reaches_the_bottom_and_stale_actions_are_refused() {
    let body: String = (0..60)
        .map(|line| format!("Line {line} of a long page.\n"))
        .collect();
    let body = format!("{body}=> gemini://x.test/end The end\n");
    let mut session = framed("gemini://x.test/long.gmi", &body, (400, 200));
    let projection = session.accessibility_projection().unwrap();
    let end = named(&projection, "The end");
    assert!(end.bounds.unwrap().y > 200.0, "below the fold, unclipped");
    assert!(
        session.accessibility_click_target(end.id).is_none(),
        "nothing to click until it is in view"
    );

    let request = DocumentA11yActionRequest {
        revision: projection.revision(),
        target: end.id,
        action: DocumentA11yAction::ScrollIntoView,
        data: None,
    };
    assert!(session.dispatch_accessibility_action(&request));
    let _ = session.frame(400, 200);
    let scrolled = session.accessibility_projection().unwrap();
    let end_box = scrolled.node(end.id).unwrap().bounds.unwrap();
    // Subtracting the scroll offset can leave a few f32 ULPs at the
    // viewport edge (for example 200.00002 after revealing the last row).
    assert!(
        end_box.y >= -0.01 && end_box.y + end_box.height <= 200.01,
        "revealed link bounds: {end_box:?}"
    );
    assert!(session.accessibility_click_target(end.id).is_some());
    assert!(
        !session.dispatch_accessibility_action(&request),
        "the request named the revision before the scroll"
    );
}

/// A gopher menu reads one row per item, each row naming its kind, with
/// its own line's box; a search row is a button.
#[test]
fn a_gopher_menu_reads_row_by_row() {
    let menu = "iWelcome to the hole\t\terror.host\t1\r\n\
            1Phlog\t/phlog\tx.test\t70\r\n\
            0About this server\t/about.txt\tx.test\t70\r\n\
            7Search the hole\t/search\tx.test\t70\r\n.\r\n";
    let session = framed("gopher://x.test/1/", menu, (500, 300));
    let projection = session.accessibility_projection().unwrap();
    let list = projection
        .nodes()
        .iter()
        .find(|node| node.role == DocumentA11yRole::List)
        .expect("the menu is a list");
    assert_eq!(list.children.len(), 4);

    let info = projection.node(list.children[0]).unwrap();
    assert_eq!(info.name.as_deref(), Some("Welcome to the hole"));
    let phlog = named(&projection, "Phlog");
    assert_eq!(phlog.role, DocumentA11yRole::Link);
    assert_eq!(phlog.description.as_deref(), Some("Directory"));
    assert_eq!(
        named(&projection, "About this server")
            .description
            .as_deref(),
        Some("Document")
    );
    let search = named(&projection, "Search the hole");
    assert_eq!(search.role, DocumentA11yRole::Button);

    let rows: Vec<f32> = list
        .children
        .iter()
        .map(|row| projection.node(*row).unwrap().bounds.unwrap().y)
        .collect();
    assert!(
        rows.windows(2).all(|pair| pair[0] < pair[1]),
        "each row has its own line: {rows:?}"
    );
}

fn nomadnet(file: &str, source: &str) -> SmolwebDocumentSession {
    use inker::{Engine, EngineInput};

    let address = format!("abb3ebcd03cb2388a838e70c001291f9:/page/{file}");
    let document = nematic::MicronEngine::new()
        .render(&EngineInput::new(address, source))
        .expect("the probe page lowers");
    let document = SmolwebDocument::from_document_with_theme(document, SmolwebTheme::Plain);
    let mut session = SmolwebDocumentSession::new(document, (640, 2400));
    let _ = session.frame(640, 2400);
    session
}

/// Real NomadNet pages, the guide's probes from NomadNet 1.4.2: the
/// structure page reads with its section headings, the links-and-fields
/// page with working links, and every node is boxed. Fields stay inert
/// source text here: the live form belongs to the host's form surface.
#[test]
fn nomadnet_pages_read_with_headings_and_links() {
    let structure = nomadnet(
        "guide-structure.mu",
        include_str!(
            "../../../../../nematic/nematic/tests/fixtures/micron/nomadnet-1.4.2/guide-structure.mu"
        ),
    );
    let projection = structure.accessibility_projection().expect("projects");
    assert!(
        projection
            .nodes()
            .iter()
            .any(|node| matches!(node.role, DocumentA11yRole::Heading { .. })),
        "the page's sections read as headings"
    );
    for node in projection.nodes() {
        assert!(
            node.bounds.is_some(),
            "{:?} {:?} has a box",
            node.role,
            node.name
        );
    }

    let mut links = nomadnet(
        "guide-links-fields.mu",
        include_str!(
            "../../../../../nematic/nematic/tests/fixtures/micron/nomadnet-1.4.2/guide-links-fields.mu"
        ),
    );
    let projection = links.accessibility_projection().expect("projects");
    let link = projection
        .nodes()
        .iter()
        .find(|node| {
            node.role == DocumentA11yRole::Link
                && links.accessibility_click_target(node.id).is_some()
        })
        .expect("a visible link with a click point");
    let target = links.accessibility_click_target(link.id).unwrap();
    assert!(
        matches!(
            links.click_at(target.point.x, target.point.y),
            SessionClick::Navigate(_) | SessionClick::Handled
        ),
        "the click point lands on that link"
    );
    for node in projection.nodes() {
        assert!(
            node.bounds.is_some(),
            "{:?} {:?} has a box",
            node.role,
            node.name
        );
    }
}
