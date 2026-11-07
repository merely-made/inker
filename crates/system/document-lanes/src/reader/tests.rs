/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;
use inker::{DocumentA11yAction, DocumentA11yRole, SessionRegistry, SessionSpawnRequest};

fn reader_session(html: &str, width: u32, height: u32) -> Box<dyn DocumentSession<Scene>> {
    ReaderSessionEngine::new(SmolwebTheme::Plain)
        .spawn(
            &SessionSpawnRequest::new("https://example.test/story/index.html")
                .with_body(html)
                .with_viewport(width, height),
        )
        .expect("reader session")
}

fn reader(session: &dyn DocumentSession<Scene>) -> &ReaderDocumentSession {
    session
        .as_any_ref()
        .downcast_ref::<ReaderDocumentSession>()
        .expect("reader session type")
}

#[test]
fn reading_preferences_and_find_reflow_one_appearance_without_changing_its_source() {
    let prose = "Words before the <em>needle</em> and after it give this paragraph enough substance to read comfortably. ".repeat(8);
    let html = format!(
        "<html><body><main><h1>Reading Find</h1><p>{prose}</p><p>Final NEEDLE after the long paragraph.</p></main></body></html>"
    );
    let mut first = reader_session(&html, 520, 180);
    let first = first
        .as_any()
        .downcast_mut::<ReaderDocumentSession>()
        .unwrap();
    let mut second = first.new_appearance((520, 180));
    first.frame(520, 180);
    second.frame(520, 180);
    let source = first.source_document();
    let baseline = second.content_height(520, 180);
    let query = inker::DocumentFindQuery::new("the needle and");
    let found = first.document_find(&query).unwrap();
    assert_eq!(found.count, 8, "inline emphasis is not a search boundary");
    assert_eq!(found.current, Some(0));
    let last = first
        .document_find_step(inker::DocumentFindDirection::Previous)
        .unwrap();
    assert_eq!(last.current, Some(7));
    assert!(
        first.scroll_y() > 0.0,
        "the session reveals the offscreen occurrence"
    );
    let mut value = crate::ReadingPreferences::default();
    value.step_text(true);
    value.step_column(false);
    first.set_reading_preferences(value).unwrap();
    let scene = first.frame(700, 360);
    assert!(
        scene
            .ops
            .iter()
            .any(|op| matches!(op, netrender::SceneOp::GlyphRun(_)))
    );
    assert_eq!(
        first
            .document_find_step(inker::DocumentFindDirection::Next)
            .unwrap()
            .current,
        Some(0)
    );
    assert!(Arc::ptr_eq(&source, &first.source_document()));
    assert!(Arc::ptr_eq(&source, &second.source_document()));
    assert_eq!(second.reading_preferences(), Default::default());
    assert_eq!(second.scroll_y(), 0.0);
    assert_eq!(second.content_height(520, 180), baseline);
    assert!(
        first
            .document_find(&inker::DocumentFindQuery::new("NEEDLE"))
            .unwrap()
            .count
            >= 9
    );
    first.clear_document_find().unwrap();
    assert_eq!(
        first
            .document_find_step(inker::DocumentFindDirection::Next)
            .unwrap()
            .count,
        0
    );
}

#[test]
fn reader_route_lowers_renders_links_and_reports_lineage() {
    let mut registry = SessionRegistry::<Scene>::new();
    registry.register(Box::new(ReaderSessionEngine::new(SmolwebTheme::Plain)));
    let request = SessionSpawnRequest::new("https://example.test/story/index.html")
        .with_body(
            "<html lang='en'><head><title>Story</title></head><body><nav>chrome</nav>\
                 <main><h1>Readable story</h1><p>This substantial paragraph keeps a \
                 <a href='../source'>source link</a> in the shared reader lane.</p></main></body></html>",
        )
        .with_viewport(640, 480);
    let mut session = registry
        .spawn(inker::routing::ENGINE_GENET_READER, &request)
        .expect("reader session");
    let scene = session.frame(640, 480);
    assert!(
        scene
            .ops
            .iter()
            .any(|op| matches!(op, netrender::SceneOp::GlyphRun(_)))
    );
    let report = session.inspect().expect("reader report");
    assert_eq!(report.title.as_deref(), Some("Readable story"));
    assert_eq!(report.links, ["https://example.test/source"]);
    let lineage = report.lineage.expect("fleece lineage");
    assert_eq!(lineage.tool, "fleece");
    assert_eq!(lineage.selector, "main");
    assert_eq!(lineage.block_count, 2);
}

#[test]
fn reader_requires_held_bytes_and_declines_non_articles() {
    let engine = ReaderSessionEngine::default();
    assert!(matches!(
        engine.spawn(&SessionSpawnRequest::new("https://example.test/")),
        Err(SessionError::Unsupported(_))
    ));
    assert!(matches!(
        engine.spawn(
            &SessionSpawnRequest::new("https://example.test/")
                .with_body("<nav><a href='/one'>one</a></nav>")
        ),
        Err(SessionError::SpawnFailed(_))
    ));
    assert!(matches!(
        engine.spawn(
            &SessionSpawnRequest::new("https://example.test/")
                .with_body("<main id='app'></main><script src='/app.js'></script>")
        ),
        Err(SessionError::Unsupported(message)) if message.contains("post-JS DOM")
    ));
}

#[test]
fn presented_headings_remain_in_reader_outline_order() {
    let blocks = vec![
        Block::Presented {
            presentation: Default::default(),
            block: Box::new(Block::Heading {
                level: 3,
                spans: vec![InlineSpan::Text("Wrapped first".into())],
            }),
        },
        Block::Quote {
            blocks: vec![Block::Presented {
                presentation: Default::default(),
                block: Box::new(Block::Heading {
                    level: 2,
                    spans: vec![InlineSpan::Text("Wrapped second".into())],
                }),
            }],
        },
        Block::Heading {
            level: 1,
            spans: vec![InlineSpan::Text("Plain third".into())],
        },
    ];

    let mut headings = Vec::new();
    collect_headings(&blocks, &mut headings);
    assert_eq!(
        headings,
        ["Wrapped first", "Wrapped second", "Plain third"],
        "presentation wrappers must not change reader-outline order"
    );
}

#[test]
fn reader_appearances_share_content_but_keep_viewport_and_scroll_independent() {
    let html = "<html><head><title>Shared reader</title></head><body><main>\
            <h1>Shared reader</h1><p>This deliberately long paragraph gives two reader \
            appearances enough vertical content to scroll independently while retaining \
            the same portable article source for each retained presentation.</p>\
            <p>Second paragraph with a <a href='/target'>target link</a>.</p></main></body></html>";
    let mut first = reader_session(html, 180, 90);
    let first = first
        .as_any()
        .downcast_mut::<ReaderDocumentSession>()
        .expect("reader session type");
    let mut second = first.new_appearance((420, 240));

    first.frame(180, 90);
    second.frame(420, 240);
    assert!(Arc::ptr_eq(
        &first.source_document(),
        &second.source_document()
    ));
    assert!(std::ptr::eq(first.document(), second.document()));
    assert_ne!(
        first.content_height(180, 90),
        second.content_height(420, 240),
        "each appearance lays out at its own viewport width"
    );

    let second_before = second.links();
    assert!(first.scroll_by(0.0, 80.0), "the narrow appearance scrolls");
    assert_ne!(first.links(), second_before, "first appearance moved");
    assert_eq!(
        second.links(),
        second_before,
        "second appearance retains its own viewport scroll"
    );
}

#[test]
fn reader_snapshot_waits_for_frame_and_groups_wrapped_links_across_reflow() {
    let label = "one two three four five six seven eight nine ten";
    let html = format!(
        "<html><head><title>Snapshot article</title></head><body><main>\
             <h1>Snapshot article</h1><p>This substantial reader paragraph introduces a \
             <a href='/go'>{label}</a> and keeps enough prose for Fleece to retain the article.\
             </p></main></body></html>"
    );
    let mut session = reader_session(&html, 130, 900);
    assert!(
        reader(session.as_ref()).accessibility_snapshot().is_none(),
        "sizing and construction do not publish a semantic snapshot"
    );

    let _ = session.frame(130, 900);
    let narrow = reader(session.as_ref())
        .accessibility_snapshot()
        .expect("completed frame publishes snapshot");
    assert_eq!(narrow.root_title.as_deref(), Some("Snapshot article"));
    let narrow_link = narrow
        .links
        .iter()
        .find(|link| link.url == "https://example.test/go")
        .expect("reader link");
    assert_eq!(narrow_link.label, label, "decorative arrow is excluded");
    assert!(
        narrow_link.rects.len() >= 2,
        "the narrow frame keeps one logical link across wrapped rectangles"
    );
    let identity = narrow_link.identity;

    let (x, y) = reader(session.as_ref())
        .accessibility_pointer_target(identity)
        .expect("visible semantic link resolves to a retained point");
    assert!((0.0..130.0).contains(&x) && (0.0..900.0).contains(&y));

    // A sizing query can rebuild retained geometry but cannot publish it
    // as the next a11y snapshot before the corresponding frame finishes.
    let _ = session.content_height(640, 300);
    assert!(
        reader(session.as_ref()).accessibility_snapshot().is_none(),
        "a size-triggered layout rebuild revokes the prior presentation"
    );
    let _ = session.frame(640, 300);
    let wide = reader(session.as_ref())
        .accessibility_snapshot()
        .expect("reflowed completed frame publishes snapshot");
    let wide_link = wide
        .links
        .iter()
        .find(|link| link.url == "https://example.test/go")
        .expect("reader link survives reflow");
    assert_eq!(
        wide_link.identity, identity,
        "geometry does not define identity"
    );
}

#[test]
fn reading_paints_code_and_links_after_maximizing_and_reflow() {
    let html = r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Reading from the graph</title></head><body><main><article><h1>Reading from the graph</h1><p>A useful reading space keeps the text close enough to follow and leaves room for a thought to settle. The document remains part of the graph: its sources, connections and saved places are available without crowding the page.</p><p>A Map and a Read are different projections of the same material. Moving between them changes the presentation, while the document keeps its identity. An Inspector can follow the current view or stay pinned to a source that is worth keeping nearby.</p><h2>Room to think</h2><p>Long lines make it easy to lose the next sentence. A measured column and a little space between lines give the eye a reliable place to return. On a narrow window the column follows the available space; the text stays readable without a second horizontal journey.</p><pre>document → reading projection
         → map projection
         → related sources</pre><p><a href="https://example.test/field-notes">Visit the related field notes</a></p><p>The surrounding controls use a smaller interface scale. Their labels remain clear, but their spacing gives the page priority. This local document is a deterministic layout sample; it does not fetch remote content.</p></article></main></body></html>"#;
    let mut session = ReaderSessionEngine::new(SmolwebTheme::System)
        .spawn(
            &SessionSpawnRequest::new("https://example.test/reading")
                .with_body(html)
                .with_viewport(549, 1382),
        )
        .unwrap();
    let reader = session
        .as_any()
        .downcast_mut::<ReaderDocumentSession>()
        .unwrap();
    for width in [549, 2200, 1280, 549] {
        let scene = reader
            .doc
            .try_frame(width, 1382)
            .expect("every visible band must translate");
        assert!(
            !scene.ops.is_empty(),
            "reading must remain painted after resize"
        );
    }
}

#[test]
fn reader_owned_virtual_focus_is_current_visible_and_appearance_local() {
    let html = format!("<html><body><main><h1>Focus article</h1><p>A substantial paragraph offers <a href='/next'>visible continuation</a> and enough prose to retain article content.</p>{}</main></body></html>", "<p>Substantial remaining article content extends below this viewport so scrolling can hide the first link.</p>".repeat(80));
    let mut first = reader_session(&html, 480, 300);
    let mut second = reader(first.as_ref()).new_appearance((480, 300));
    assert!(Arc::ptr_eq(
        &reader(first.as_ref()).source_document(),
        &second.source_document()
    ));
    first.frame(480, 300);
    second.frame(480, 300);
    let projection = first.accessibility_projection().unwrap();
    let link = projection
        .nodes()
        .iter()
        .find(|node| node.role == DocumentA11yRole::Link)
        .unwrap()
        .id;
    let request = inker::DocumentA11yActionRequest {
        revision: projection.revision(),
        target: link,
        action: DocumentA11yAction::Focus,
        data: None,
    };
    assert!(
        first.dispatch_accessibility_action(&request),
        "the advertised virtual Focus must reach the owned Reader session"
    );
    let focused = first.accessibility_projection().unwrap();
    assert!(focused.node(link).unwrap().state.focused);
    assert_ne!(focused.revision(), request.revision);
    assert!(
        !first.dispatch_accessibility_action(&request),
        "pre-Focus revision is stale"
    );
    assert!(
        second
            .accessibility_projection()
            .unwrap()
            .nodes()
            .iter()
            .all(|node| !node.state.focused),
        "focus is appearance local"
    );
    for action in [
        DocumentA11yAction::Click,
        DocumentA11yAction::SetValue,
        DocumentA11yAction::ScrollIntoView,
    ] {
        assert!(
            !first.dispatch_accessibility_action(&inker::DocumentA11yActionRequest {
                revision: focused.revision(),
                action,
                ..request.clone()
            })
        );
    }
    assert!(
        !first.dispatch_accessibility_action(&inker::DocumentA11yActionRequest {
            revision: focused.revision(),
            target: focused.root(),
            ..request.clone()
        })
    );
    first.scroll_to(2000.0);
    first.frame(480, 300);
    let hidden = first.accessibility_projection().unwrap();
    assert!(hidden.node(link).is_none());
    assert!(
        !first.dispatch_accessibility_action(&inker::DocumentA11yActionRequest {
            revision: hidden.revision(),
            ..request.clone()
        }),
        "current revision cannot revive an offscreen target"
    );
    first.scroll_to(0.0);
    first.frame(480, 300);
    assert!(
        first
            .accessibility_projection()
            .unwrap()
            .node(link)
            .unwrap()
            .state
            .focused,
        "scroll and temporary invisibility retain virtual focus custody"
    );
}

#[test]
fn reader_projection_is_partial_with_virtual_focus_only() {
    let html = "<html><head><title>Projection article</title></head><body><main>\
            <h1>Projection article</h1><p>A sufficiently long Reader paragraph has a \
            <a href='/next'>visible continuation</a> for the retained canvas.</p></main></body></html>";
    let mut session = reader_session(html, 480, 300);
    assert!(session.accessibility_projection().is_none());
    let _ = session.frame(480, 300);
    let first = session
        .accessibility_projection()
        .expect("completed Reader frame publishes a neutral projection");
    assert_eq!(first.support().capability(), inker::A11yCapability::Partial);
    assert!(
        first
            .support()
            .limitations()
            .iter()
            .any(|limitation| limitation.contains("activation remains unavailable"))
    );
    let root = first.node(first.root()).expect("Reader root");
    assert_eq!(root.role, DocumentA11yRole::Document);
    let link = first
        .nodes()
        .iter()
        .find(|node| node.role == DocumentA11yRole::Link)
        .expect("visible Reader link");
    assert_eq!(link.actions, [DocumentA11yAction::Focus]);
    assert!(link.bounds.is_some());
    let id = link.id;
    let revision = first.revision();
    let _ = session.frame(480, 300);
    let stable = session
        .accessibility_projection()
        .expect("same completed frame retains projection");
    assert_eq!(stable.revision(), revision);
    assert!(
        stable.node(id).is_some(),
        "link ID outlives a stable redraw"
    );
}

#[test]
fn reader_pointer_target_rechecks_current_scroll_and_content_hole() {
    let body: String = (0..40)
        .map(|index| format!("<p>Reader body line {index} has enough text for extraction.</p>"))
        .collect();
    let html = format!(
        "<html><head><title>Scrolled snapshot</title></head><body><main>\
             <h1>Scrolled snapshot</h1>{body}\
             <p><a href='/tail'>tail link</a></p></main></body></html>"
    );
    let mut session = reader_session(&html, 400, 120);
    let _ = session.frame(400, 120);
    assert!(
        reader(session.as_ref())
            .accessibility_snapshot()
            .expect("completed frame")
            .links
            .is_empty(),
        "the tail is not initially in the content hole"
    );

    session.scroll_to(f32::MAX);
    let snapshot = reader(session.as_ref())
        .accessibility_snapshot()
        .expect("scroll reads retained geometry");
    let tail = snapshot
        .links
        .iter()
        .find(|link| link.url == "https://example.test/tail")
        .expect("tail link becomes visible after scroll");
    let identity = tail.identity;
    let point = reader(session.as_ref())
        .accessibility_pointer_target(identity)
        .expect("current visible tail has a pointer target");
    assert!(point.0 >= 0.0 && point.0 < 400.0 && point.1 >= 0.0 && point.1 < 120.0);

    session.scroll_to(0.0);
    assert!(
        reader(session.as_ref())
            .accessibility_pointer_target(identity)
            .is_none(),
        "the old snapshot token is not used once scroll hides its retained rect"
    );
}

#[test]
fn article_lowering_preserves_table_header_and_figure_caption() {
    let dom = genet_static_dom::StaticDocument::parse(
        "<main><h1>Data report</h1><p>A substantial introduction makes this report readable.</p>\
             <table><tr><th>Name</th><th>Value</th></tr><tr><td>A</td><td>B</td></tr></table>\
             <figure><img src='/plot.png' alt='plot'><figcaption>Measured values</figcaption></figure></main>",
    );
    let article = fleece::extract_article(&dom).expect("article");
    let document = lower_article("https://example.test/report", &article);
    assert!(matches!(
        document.blocks.iter().find(|block| matches!(block, Block::Table { .. })),
        Some(Block::Table { header, rows, .. }) if header.len() == 2 && rows.len() == 1
    ));
    assert!(document.blocks.iter().any(|block| {
        matches!(block, Block::Image { url, .. } if url == "https://example.test/plot.png")
    }));
    assert!(document.blocks.iter().any(|block| {
        matches!(block, Block::Paragraph { spans } if inker::inline_text(spans) == "Measured values")
    }));
}
