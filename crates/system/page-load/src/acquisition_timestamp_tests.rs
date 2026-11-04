// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

#[test]
fn same_url_reload_gets_new_first_chunk_time_and_keeps_it_across_chunks() {
    let mut load = PageLoad::default();
    load.begin(1);
    assert_eq!(
        load.accept_progress(
            PageProgress {
                request: 1,
                url: "https://example.test/page".into(),
                response_url: "https://example.test/page".into(),
                content_type: Some("text/html".into()),
                bytes: b"same bytes".to_vec(),
            },
            10,
        ),
        Some(10)
    );
    assert_eq!(
        load.fetched("https://example.test/page")
            .unwrap()
            .acquired_at_ms,
        10
    );

    load.begin(2);
    assert_eq!(
        load.accept_progress(
            PageProgress {
                request: 2,
                url: "https://example.test/page".into(),
                response_url: "https://example.test/page".into(),
                content_type: Some("text/html".into()),
                bytes: b"same ".to_vec(),
            },
            30,
        ),
        Some(5)
    );
    assert_eq!(
        load.fetched("https://example.test/page")
            .unwrap()
            .acquired_at_ms,
        30
    );

    assert_eq!(
        load.accept_progress(
            PageProgress {
                request: 2,
                url: "https://example.test/page".into(),
                response_url: "https://example.test/page".into(),
                content_type: Some("text/html".into()),
                bytes: b"bytes".to_vec(),
            },
            40,
        ),
        Some(10)
    );
    assert_eq!(
        load.fetched("https://example.test/page").unwrap().bytes,
        b"same bytes"
    );
    assert_eq!(
        load.fetched("https://example.test/page")
            .unwrap()
            .acquired_at_ms,
        30
    );
}

#[test]
fn same_url_terminal_without_progress_does_not_borrow_old_source_metadata() {
    let url = "https://example.test/page";
    let mut load = PageLoad::default();
    load.note_fetched(
        url.into(),
        LoadedDocument {
            bytes: b"old bytes".to_vec(),
            content_type: Some("text/markdown".into()),
            body: "old bytes".into(),
            effective_url: Some("https://old.example/page".into()),
            acquired_at_ms: 10,
        },
        9,
    );
    load.begin(3);

    assert_eq!(
        load.accept_outcome(
            FetchOutcome {
                request: 3,
                url: url.into(),
                result: Ok(Fetched {
                    content_type: None,
                    content_disposition: None,
                    bytes: b"new bytes".to_vec(),
                    body: "new bytes".into(),
                }),
            },
            30,
        ),
        LoadAnswer::Document
    );
    let document = load
        .fetched(url)
        .expect("new terminal response is retained");
    assert_eq!(document.bytes, b"new bytes");
    assert_eq!(document.content_type, None);
    assert_eq!(document.effective_url, None);
    assert_eq!(document.acquired_at_ms, 30);
}
