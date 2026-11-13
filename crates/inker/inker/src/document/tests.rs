use super::*;

fn doc(blocks: Vec<Block>) -> EngineDocument {
    EngineDocument {
        address: "doc:1".into(),
        title: None,
        content_type: "text/plain".into(),
        lang: None,
        provenance: DocumentProvenance::default(),
        trust: DocumentTrustState::Unknown,
        diagnostics: Vec::new(),
        navigation: Default::default(),
        blocks,
    }
}

#[test]
fn provenance_for_engine_records_kind_and_uri() {
    let p = DocumentProvenance::for_engine("nematic.markdown", "file:///x.md");
    assert_eq!(p.source_kind.as_deref(), Some("nematic.markdown"));
    assert_eq!(p.canonical_uri.as_deref(), Some("file:///x.md"));
    assert!(p.fetched_at.is_none());
}

#[test]
fn trust_state_default_is_unknown() {
    let t: DocumentTrustState = Default::default();
    assert_eq!(t, DocumentTrustState::Unknown);
}

#[test]
fn outgoing_links_walks_feed_entry_urls() {
    let document = doc(vec![
        Block::FeedHeader {
            title: "Feed".into(),
            subtitle: None,
            summary: None,
            source_url: Some("https://feed.test/".into()),
        },
        Block::FeedEntry {
            title: "Entry".into(),
            date: None,
            summary: None,
            article_url: Some("https://feed.test/post-1".into()),
            source_url: Some("https://feed.test/".into()),
            published: None,
            updated: None,
            guid: None,
            enclosures: Vec::new(),
            content_address: None,
        },
    ]);
    assert_eq!(
        document.outgoing_links(),
        vec![
            "https://feed.test/",
            "https://feed.test/post-1",
            "https://feed.test/",
        ]
    );
}

#[test]
fn outgoing_links_skips_image_sources() {
    // Links are destinations; an image source is a subresource of this
    // document, fetched through `DocumentSession::subresources`. The
    // anchor beside the image is the only outgoing link here.
    let document = doc(vec![
        Block::Image {
            url: "https://img.test/diagram.png".into(),
            alt: "diagram".into(),
        },
        Block::Paragraph {
            spans: vec![InlineSpan::Link {
                url: "https://dest.test/".into(),
                spans: vec![InlineSpan::Text("go".into())],
                title: None,
                predicate: None,
            }],
        },
    ]);
    assert_eq!(document.outgoing_links(), vec!["https://dest.test/"]);
}

#[test]
fn in_page_links_are_label_text_not_outgoing_links() {
    let document = doc(vec![Block::Paragraph {
        spans: vec![InlineSpan::InPage {
            target: InPageTarget {
                fragment: Some("top".into()),
                block: Some(0),
            },
            spans: vec![InlineSpan::Text("Top".into())],
        }],
    }]);
    assert!(document.outgoing_links().is_empty());
    let Block::Paragraph { spans } = &document.blocks[0] else {
        unreachable!()
    };
    assert_eq!(inline_text(spans), "Top");
}
