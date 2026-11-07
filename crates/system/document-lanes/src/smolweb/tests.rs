/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

#[test]
fn response_mime_precedes_carrier_and_plain_text_is_not_sniffed() {
    let body = "# Literal heading\n<rss><channel><title>Still literal</title></channel></rss>";
    let plain = SmolwebDocument::parse_input(
        &EngineInput::new("scroll://source.test/page", body)
            .with_content_type("Text/Plain; charset=utf-8"),
        SmolwebTheme::Plain,
    );
    assert!(plain.document().title.is_none());
    assert_eq!(
        plain.document().provenance.source_kind.as_deref(),
        Some("nematic.text")
    );
    assert!(
        plain
            .document()
            .blocks
            .iter()
            .all(|block| matches!(block, Block::Paragraph { .. }))
    );
    let markdown = SmolwebDocument::parse_input(
        &EngineInput::new("gopher://source.test/page", "# Markdown heading\n")
            .with_content_type("text/markdown;charset=utf-8"),
        SmolwebTheme::Plain,
    );
    assert_eq!(
        markdown.document().provenance.source_kind.as_deref(),
        Some("nematic.markdown")
    );
    assert_eq!(
        markdown.document().title.as_deref(),
        Some("Markdown heading")
    );
    let feed = SmolwebDocument::parse_input(
        &EngineInput::new("gemini://source.test/feed", "<?xml version=\"1.0\"?><rss version=\"2.0\"><channel><title>Feed</title><link>https://source.test/</link><description>Saved</description></channel></rss>")
            .with_content_type("application/xml"),
        SmolwebTheme::Plain,
    );
    assert_eq!(
        feed.document().provenance.source_kind.as_deref(),
        Some("nematic.feed")
    );
    assert_eq!(feed.document().title.as_deref(), Some("Feed"));
}

#[test]
fn streamed_replacement_retains_response_mime_and_clears_stale_interpretation() {
    let url = "scroll://source.test/page";
    let input = EngineInput::new(url, "##### Prefix\n").with_content_type("text/scroll;lang=fr");
    let mut document = SmolwebDocument::parse_input(&input, SmolwebTheme::Plain);
    let mut appearance = document.new_appearance();
    appearance.replace_body(url, "##### Complete\n=> /record Cited [Citation]\n");
    assert_eq!(
        appearance.source_content_type.as_deref(),
        Some("text/scroll;lang=fr")
    );
    assert!(
        appearance
            .document()
            .blocks
            .iter()
            .any(|block| matches!(block, Block::Heading { level: 5, .. }))
    );
    assert_eq!(
        appearance.document().provenance.source_label.as_deref(),
        Some("scrolltext")
    );
    document
        .replace_input(&EngineInput::new(url, "# Literal now\n").with_content_type("text/plain"));
    assert!(document.document().title.is_none());
    document.replace_body("gemini://source.test/new", "# New address\n");
    assert_eq!(document.document().title.as_deref(), Some("New address"));
    assert!(document.source_content_type.is_none());
    document.replace_input(&EngineInput::new(url, "# Explicit unknown MIME\n"));
    assert_eq!(
        document.document().title.as_deref(),
        Some("Explicit unknown MIME")
    );
    assert!(document.source_content_type.is_none());
}

fn test_png() -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(2, 1, image::Rgba([20, 40, 60, 255]));
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .expect("encode PNG fixture");
    bytes
}

#[test]
fn gemtext_uses_engine_document_and_paints_text() {
    let mut doc = SmolwebDocument::parse(
        "gemini://x.test/",
        "# Hello\n\nWorld.\n",
        SmolwebTheme::Site,
    );
    assert_eq!(
        doc.document().provenance.source_kind.as_deref(),
        Some(nematic::ENGINE_GEMTEXT)
    );
    let scene = doc.frame(800, 600);
    assert!(
        scene
            .ops
            .iter()
            .any(|op| matches!(op, netrender::SceneOp::GlyphRun(_)))
    );
    assert!(matches!(
        scene.ops.first(),
        Some(netrender::SceneOp::Rect(_))
    ));
}

#[test]
fn replacing_a_streamed_body_rebuilds_structure_and_invalidates_layout() {
    let mut doc = SmolwebDocument::parse_with_inline_media(
        "gemini://x.test/live",
        "# Prefix\n=> first.png First\n",
        SmolwebTheme::Plain,
        SmolwebInlineMediaPolicy::images(),
    );
    let _ = doc.frame(400, 300);
    assert!(doc.layout.is_some());

    doc.replace_body(
        "gemini://x.test/live",
        "# Complete\n=> second.png Second\nTail\n",
    );

    assert_eq!(doc.document().title.as_deref(), Some("Complete"));
    assert_eq!(doc.subresources(), ["gemini://x.test/second.png"]);
    assert!(doc.layout.is_none());
    assert!(
        doc.frame(400, 300)
            .ops
            .iter()
            .any(|operation| matches!(operation, netrender::SceneOp::GlyphRun(_)))
    );
}

#[test]
fn streamed_replacement_scene_retains_prefix_runs_and_adds_tail_runs() {
    let url = "gemini://x.test/streaming.gmi";
    let prefix = "# Streaming prefix visible\nThis arrived before connection close.\n";
    let complete = concat!(
        "# Streaming prefix visible\n",
        "This arrived before connection close.\n",
        "## Streaming tail arrived\n",
        "The terminal body is complete.\n",
    );
    let mut doc = SmolwebDocument::parse(url, prefix, SmolwebTheme::Plain);
    let prefix_scene = doc.frame(614, 600);
    let prefix_runs: Vec<_> = prefix_scene
        .iter_glyph_runs()
        .map(|run| {
            (
                run.font_size.to_bits(),
                run.glyphs
                    .iter()
                    .map(|glyph| (glyph.id, glyph.x.to_bits(), glyph.y.to_bits()))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();

    doc.replace_body(url, complete);
    let complete_scene = doc.frame(614, 600);
    let complete_runs: Vec<_> = complete_scene
        .iter_glyph_runs()
        .map(|run| {
            (
                run.font_size.to_bits(),
                run.glyphs
                    .iter()
                    .map(|glyph| (glyph.id, glyph.x.to_bits(), glyph.y.to_bits()))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();

    assert!(complete_runs.len() > prefix_runs.len());
    assert!(
        prefix_runs
            .iter()
            .all(|prefix_run| complete_runs.contains(prefix_run)),
        "the full replacement scene must still carry every prefix glyph run"
    );
}

#[test]
fn long_document_scrolls_and_windows() {
    let body: String = (0..200).map(|i| format!("Line {i}\n\n")).collect();
    let mut doc = SmolwebDocument::parse("gemini://x.test/", &body, SmolwebTheme::Plain);
    assert!(doc.content_height(400, 300) > 300);
    assert!(!doc.scroll_by(0.0, -50.0));
    assert!(doc.scroll_by(0.0, 240.0));
    assert_eq!(doc.frame(400, 300).viewport_height, 300);
}

#[test]
fn links_and_clicks_share_viewport_coordinates() {
    let mut body: String = (0..4).map(|i| format!("Line {i}\n\n")).collect();
    body.push_str("=> gemini://x.test/page A link\n");
    body.extend((0..30).map(|i| format!("Tail {i}\n\n")));
    let mut doc = SmolwebDocument::parse("gemini://x.test/", &body, SmolwebTheme::Plain);
    assert!(doc.links().is_empty());
    let _ = doc.frame(400, 300);
    let (url, [_, initial_y, _, _]) = doc.links().into_iter().next().expect("link region");
    assert_eq!(url, "gemini://x.test/page");
    assert!(doc.scroll_by(0.0, 40.0));
    let (url, [x, y, width, height]) = doc
        .links()
        .into_iter()
        .next()
        .expect("scrolled link region");
    assert_eq!(url, "gemini://x.test/page");
    assert!(y < initial_y, "scrolling must move the host hit table");
    assert!(width > 0.0 && height > 0.0);
    assert!(matches!(
        doc.click_at(x + width / 2.0, y + height / 2.0, 400, 300),
        Some(InteractionKind::Link { url }) if url == "gemini://x.test/page"
    ));
    doc.scroll_to(f32::MAX);
    assert!(
        doc.links().is_empty(),
        "viewport-space links omit regions scrolled fully out of view"
    );
}

#[test]
fn wide_table_links_clip_scroll_hit_test_and_resize_in_viewport_coordinates() {
    let document = EngineDocument {
        address: "gemini://x.test/wide".into(),
        title: None,
        content_type: "text/gemini".into(),
        lang: None,
        provenance: inker::DocumentProvenance::default(),
        trust: inker::DocumentTrustState::Unknown,
        diagnostics: Vec::new(),
        navigation: Default::default(),
        blocks: vec![Block::Table {
            alignments: Vec::new(),
            header: Vec::new(),
            rows: vec![vec![
                vec![InlineSpan::Text(
                    "first-column-is-intentionally-unbreakable".into(),
                )],
                vec![InlineSpan::Link {
                    url: "gemini://x.test/wide/target".into(),
                    title: None,
                    spans: vec![InlineSpan::Text("target".into())],
                    predicate: None,
                }],
            ]],
        }],
    };
    let mut doc = SmolwebDocument::from_document(
        document,
        DocumentStyleSheet::default(),
        [1.0, 1.0, 1.0, 1.0],
    );
    let _ = doc.frame(120, 100);
    assert!(
        doc.max_scroll_x() > 0.0,
        "the wide table must expose a horizontal range"
    );
    assert!(
        doc.links().is_empty(),
        "offscreen link rectangles are not exposed as viewport targets"
    );

    assert!(doc.scroll_by(f32::MAX, 0.0));
    let (url, [x, y, width, height]) = doc.links().into_iter().next().expect("scrolled target");
    assert_eq!(url, "gemini://x.test/wide/target");
    assert!(x >= 0.0 && x + width <= 120.0);
    assert!(y >= 0.0 && y + height <= 100.0);
    assert!(matches!(
        doc.click_at(x + width * 0.5, y + height * 0.5, 120, 100),
        Some(InteractionKind::Link { url }) if url == "gemini://x.test/wide/target"
    ));

    let _ = doc.frame(240, 100);
    assert!(doc.scroll_x() <= doc.max_scroll_x());
    for (_, [x, y, width, height]) in doc.links() {
        assert!(x >= 0.0 && x + width <= 240.0);
        assert!(y >= 0.0 && y + height <= 100.0);
    }
}

#[test]
fn spartan_prompt_click_remains_a_submission() {
    let mut doc = SmolwebDocument::parse(
        "spartan://x.test/guestbook",
        "=: /guestbook/sign Sign it\n",
        SmolwebTheme::Plain,
    );
    let _ = doc.frame(400, 300);
    let region = doc
        .layout
        .as_ref()
        .expect("layout")
        .packet
        .interactions
        .iter()
        .find(|region| matches!(region.kind, InteractionKind::Submit { .. }))
        .expect("prompt region");
    let x0 = region.bounds.origin.x;
    let y0 = region.bounds.origin.y;
    let x1 = x0 + region.bounds.size.width;
    let y1 = y0 + region.bounds.size.height;
    assert!(matches!(
        doc.click_at((x0 + x1) / 2.0, (y0 + y1) / 2.0, 400, 300),
        Some(InteractionKind::Submit { target }) if target == "/guestbook/sign"
    ));
}

#[test]
fn schemes_select_their_nematic_engines() {
    let gopher = SmolwebDocument::parse(
        "gopher://x.test/",
        "1Files\t/files\tx.test\t70\r\n",
        SmolwebTheme::Plain,
    );
    assert_eq!(
        gopher.document().provenance.source_kind.as_deref(),
        Some(nematic::ENGINE_GOPHER)
    );

    let feed = SmolwebDocument::parse(
        "gemini://x.test/feed",
        "<?xml version=\"1.0\"?><rss version=\"2.0\"><channel><title>Log</title></channel></rss>",
        SmolwebTheme::Dark,
    );
    assert_eq!(
        feed.document().provenance.source_kind.as_deref(),
        Some(nematic::ENGINE_FEED)
    );
}

#[test]
fn a_site_themed_appearance_follows_the_host_mode_and_keeps_its_fonts() {
    let url = "gemini://geminiprotocol.net/";
    let mut doc = SmolwebDocument::parse(url, "# Gemini\n=> /docs Docs\n", SmolwebTheme::Site);
    doc.set_font_family("Atkinson");
    doc.set_reading_preferences(crate::ReadingPreferences {
        text_size: 24,
        column_em: 28,
    })
    .unwrap();
    let light = (doc.style_sheet().colors, doc.background);
    assert!(!doc.set_color_mode(ModeProfile::LIGHT), "already light");
    assert!(doc.set_color_mode(ModeProfile::DARK));
    let (colors, background) = theme_colors(&SmolwebTheme::Site, url, ModeProfile::DARK);
    assert_eq!(
        (doc.style_sheet().colors, doc.background),
        (colors, background)
    );
    assert_ne!(doc.background, light.1);
    assert!(
        background[..3].iter().all(|channel| *channel < 0.3),
        "dark ground {background:?}"
    );
    assert!(doc.style_sheet().body_font_family.starts_with("Atkinson"));
    assert_eq!(doc.reading_preferences().text_size, 24);
    let twin = doc.new_appearance();
    assert_eq!(
        (twin.color_mode(), twin.background),
        (ModeProfile::DARK, background)
    );
    assert!(doc.set_color_mode(ModeProfile::LIGHT));
    assert_eq!((doc.style_sheet().colors, doc.background), light);

    // An appearance built from an explicit style sheet keeps its colors.
    let (style, background) = style_for_theme(&SmolwebTheme::Plain, url, "text/gemini");
    let mut styled =
        SmolwebDocument::from_document(doc.document().clone(), style.clone(), background);
    assert!(!styled.set_color_mode(ModeProfile::DARK));
    assert_eq!(
        (styled.style_sheet().colors, styled.background),
        (style.colors, background)
    );
}

/// A gopher menu reaches the host as one typed menu (smolweb fidelity plan
/// WS4, R2): its rows keep their kinds, a directory row is a viewport link
/// target, and the search row submits rather than navigates.
#[test]
fn a_gopher_menu_reaches_the_host_as_one_typed_menu() {
    let body = concat!(
        "iWelcome\tfake\t(NULL)\t0\r\n",
        "1Phlog\t/phlog\tx.test\t70\r\n",
        "7Search\t/find\tx.test\t70\r\n",
    );
    let mut doc = SmolwebDocument::parse("gopher://x.test/", body, SmolwebTheme::Plain);
    let [Block::Menu { rows }] = doc.document().blocks.as_slice() else {
        panic!("one menu: {:?}", doc.document().blocks);
    };
    let kinds: Vec<_> = rows.iter().map(|row| row.kind).collect();
    assert_eq!(
        kinds,
        [
            inker::MenuItemKind::Info,
            inker::MenuItemKind::Directory,
            inker::MenuItemKind::Search,
        ]
    );
    let _ = doc.frame(640, 480);
    let links: Vec<_> = doc.links().into_iter().map(|(url, _)| url).collect();
    assert_eq!(
        links,
        ["gopher://x.test/1/phlog"],
        "the search row is not a link"
    );
}

/// An entry's own address fetches the feed without its fragment and renders
/// the entry alone (smolweb fidelity plan WS4, R4).
#[test]
fn an_entry_address_loads_the_feed_and_renders_the_entry() {
    const FEED: &str = r#"<?xml version="1.0"?>
<rss version="2.0" xmlns:content="http://purl.org/rss/1.0/modules/content/">
  <channel><title>Log</title>
    <item>
      <title>Post</title>
      <link>https://x.test/post</link>
      <guid>post-1</guid>
      <description>Teaser.</description>
      <content:encoded><![CDATA[<p>The whole post.</p>]]></content:encoded>
    </item>
  </channel>
</rss>"#;
    struct Recording(std::sync::Mutex<Vec<String>>);
    impl ResourceFetcher for Recording {
        fn fetch_request(
            &self,
            _request: &genet_host_api::ResourceRequest,
        ) -> Option<genet_host_api::ResourceFetchResponse> {
            None
        }
        fn fetch(&self, url: &str) -> Option<Vec<u8>> {
            self.0.lock().unwrap().push(url.to_string());
            Some(FEED.as_bytes().to_vec())
        }
    }

    let feed = SmolwebDocument::parse("gemini://x.test/feed.xml", FEED, SmolwebTheme::Plain);
    let address = feed
        .document()
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::FeedEntry {
                content_address: Some(address),
                ..
            } => Some(address.clone()),
            _ => None,
        })
        .expect("the entry carries its own address");

    let fetcher = Recording(Default::default());
    let article = SmolwebDocument::load(&fetcher, &address, SmolwebTheme::Plain).unwrap();
    assert_eq!(
        *fetcher.0.lock().unwrap(),
        ["gemini://x.test/feed.xml"],
        "the fragment is not sent"
    );
    let document = article.document();
    assert_eq!(document.title.as_deref(), Some("Post"));
    assert_eq!(
        document.provenance.canonical_uri.as_deref(),
        Some("https://x.test/post")
    );
    assert!(inline_text_of(document).contains("The whole post."));
}

fn inline_text_of(document: &EngineDocument) -> String {
    document
        .walk_inline_spans()
        .into_iter()
        .map(|span| inline_text(std::slice::from_ref(span)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A fixed-width menu format keeps one typeface across its column grid
/// (see `FIXED_WIDTH_MENU_TYPES`).
#[test]
fn a_fixed_width_menu_sets_its_body_font_to_the_monospace_one() {
    for content_type in ["application/gopher-menu", "application/x-nex-listing"] {
        let (style, _) = style_for_theme(&SmolwebTheme::Dark, "gopher://x.test/", content_type);
        assert_eq!(
            style.body_font_family, style.mono_font_family,
            "{content_type} must not mix typefaces inside one column grid"
        );
    }
    // A prose format keeps the body serif: gemtext is not column-aligned.
    for content_type in ["text/gemini", "text/plain"] {
        let (style, _) = style_for_theme(&SmolwebTheme::Dark, "gemini://x.test/", content_type);
        assert_eq!(
            style.body_font_family, "serif, sans-serif, monospace",
            "{content_type} keeps prose and glyph fallback roles"
        );
    }
}

#[test]
fn app_rgb_palette_maps_into_document_style() {
    let theme = SmolwebTheme::App(SmolwebPalette {
        bg: "rgb(16, 32, 48)".into(),
        fg: "rgb(250, 250, 250)".into(),
        link: "rgb(51, 204, 255)".into(),
        quote: "rgb(153, 170, 187)".into(),
        pre_bg: "rgb(10, 22, 34)".into(),
    });
    let (style, background) = style_for_theme(&theme, "gemini://x.test/", "text/gemini");
    assert_eq!(background, [16.0 / 255.0, 32.0 / 255.0, 48.0 / 255.0, 1.0]);
    assert_eq!(
        style.colors.link_text,
        [51.0 / 255.0, 204.0 / 255.0, 1.0, 1.0]
    );
}

#[path = "tests/inline_media.rs"]
mod inline_media;
