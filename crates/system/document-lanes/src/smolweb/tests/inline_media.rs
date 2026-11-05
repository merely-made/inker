/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

#[test]
fn opted_in_gemtext_image_link_fetches_decodes_paints_and_stays_clickable() {
    let mut doc = SmolwebDocument::parse_with_inline_media(
        "gemini://x.test/posts/index.gmi",
        "=> media/picture.png A picture\n",
        SmolwebTheme::Plain,
        SmolwebInlineMediaPolicy::images(),
    );
    assert!(matches!(
        &doc.document().blocks[0],
        Block::Image { url, alt }
            if url == "gemini://x.test/posts/media/picture.png" && alt == "A picture"
    ));
    assert_eq!(
        doc.subresources(),
        ["gemini://x.test/posts/media/picture.png"]
    );
    assert!(doc.provide_subresource("gemini://x.test/posts/media/picture.png", &test_png()));
    assert!(doc.subresources().is_empty());

    let scene = doc.frame(400, 300);
    assert!(
        scene
            .ops
            .iter()
            .any(|operation| matches!(operation, netrender::SceneOp::Image(_)))
    );
    let (url, [x, y, width, height]) = doc.links().into_iter().next().expect("image link region");
    assert_eq!(url, "gemini://x.test/posts/media/picture.png");
    assert!(matches!(
        doc.click_at(x + width / 2.0, y + height / 2.0, 400, 300),
        Some(InteractionKind::Link { url })
            if url == "gemini://x.test/posts/media/picture.png"
    ));
}

#[test]
fn nested_image_resources_are_deduplicated_admitted_and_retained_by_replacement() {
    let mut doc = SmolwebDocument::parse_with_inline_media(
        "gemini://x.test/nested",
        "",
        SmolwebTheme::Plain,
        SmolwebInlineMediaPolicy::images(),
    );
    let image = |url: &str| Block::Image {
        url: url.into(),
        alt: String::new(),
    };
    let shared = "gemini://x.test/shared.png";
    let removed = "gemini://x.test/removed.png";
    let added = "gemini://x.test/added.png";
    let mut initial = (*doc.document).clone();
    initial.blocks = vec![
        Block::Quote {
            blocks: vec![image(shared)],
        },
        Block::List {
            ordered: false,
            items: vec![vec![image(shared), image(removed)]],
        },
        Block::Presented {
            presentation: Default::default(),
            block: Box::new(image(added)),
        },
    ];
    doc.replace_document(initial);

    assert_eq!(doc.subresources(), [shared, removed, added]);
    assert!(doc.provide_subresource(shared, &test_png()));
    assert!(doc.provide_subresource(removed, &test_png()));
    assert!(!doc.provide_subresource("gemini://x.test/unknown.png", &test_png()));

    let mut replacement = (*doc.document).clone();
    replacement.blocks = vec![Block::List {
        ordered: true,
        items: vec![vec![
            Block::Presented {
                presentation: Default::default(),
                block: Box::new(image(shared)),
            },
            image(shared),
            image(added),
        ]],
    }];
    doc.replace_document(replacement);

    assert!(doc.images.contains_key(shared));
    assert!(!doc.images.contains_key(removed));
    assert_eq!(doc.subresources(), [added]);
    assert!(doc.provide_subresource(added, &test_png()));
    assert!(doc.subresources().is_empty());
}
