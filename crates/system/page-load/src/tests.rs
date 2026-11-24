// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

const PAGE: &str = "gemini://example/live";

fn progress(request: FetchRequestId, bytes: &[u8]) -> PageProgress {
    PageProgress {
        request,
        url: PAGE.into(),
        response_url: PAGE.into(),
        content_type: Some("text/gemini".into()),
        bytes: bytes.to_vec(),
    }
}

fn outcome(request: FetchRequestId, result: Result<Fetched, FetchFailure>) -> FetchOutcome {
    FetchOutcome {
        request,
        url: PAGE.into(),
        result,
    }
}

#[test]
fn fetched_documents_are_address_scoped() {
    let document = LoadedDocument {
        bytes: b"# Capsule".to_vec(),
        content_type: Some("text/gemini".into()),
        body: "# Capsule".into(),
        effective_url: Some("gemini://example/one".into()),
        http_response: None,
        acquired_at_ms: 1,
    };
    let mut load = PageLoad::default();
    load.note_fetched("gemini://example/one".into(), document.clone(), 9);
    assert_eq!(load.fetched("gemini://example/one"), Some(&document));
    assert!(load.fetched("gemini://example/two").is_none());
}

#[test]
fn streamed_documents_accumulate_exact_bytes_before_decoding() {
    let mut load = PageLoad::default();
    load.begin(1);
    assert_eq!(
        load.accept_progress(progress(1, &[b'#', b' ', 0xc3]), 10),
        Some(3)
    );
    assert_eq!(
        load.accept_progress(progress(1, &[0xa9, b'\n']), 20),
        Some(5)
    );
    let document = load.fetched(PAGE).expect("streamed prefix is retained");
    assert_eq!(document.body, "# é\n");
    assert_eq!(document.acquired_at_ms, 10, "first admission time is kept");
    assert!(matches!(
        load.phase(),
        Some(LoadPhase::Streaming {
            received_bytes: 5,
            ..
        })
    ));
}

#[test]
fn a_superseded_request_cannot_surface() {
    let mut load = PageLoad::default();
    assert_eq!(load.begin(1), None);
    assert_eq!(
        load.begin(2),
        Some(1),
        "the older request is handed back to cancel"
    );
    assert_eq!(load.accept_progress(progress(1, b"old"), 1), None);
    assert_eq!(
        load.accept_outcome(outcome(1, Ok(Fetched::text(None, "old"))), 1),
        LoadAnswer::Stale
    );
    assert!(load.fetched(PAGE).is_none());
    assert_eq!(
        load.accept_outcome(outcome(2, Ok(Fetched::text(None, "new"))), 2),
        LoadAnswer::Document
    );
    assert_eq!(
        load.fetched(PAGE).map(|document| document.body.as_str()),
        Some("new")
    );
}

#[test]
fn stop_keeps_the_shown_prefix_and_retires_the_request() {
    let mut load = PageLoad::default();
    load.begin(7);
    load.accept_progress(progress(7, b"# Part"), 5);
    assert_eq!(load.stop_active(), Some(7));
    assert_eq!(load.stop_active(), None, "nothing is left to stop");
    assert_eq!(
        load.phase(),
        Some(&LoadPhase::Stopped { received_bytes: 6 })
    );
    assert_eq!(
        load.fetched(PAGE).map(|document| document.body.as_str()),
        Some("# Part")
    );
    assert_eq!(
        load.accept_outcome(outcome(7, Err(FetchFailure::Cancelled)), 6),
        LoadAnswer::Stale,
        "the transport's cancellation answer arrives after Stop retired the request"
    );
}

#[test]
fn a_settled_response_keeps_what_streaming_learned() {
    let mut load = PageLoad::default();
    load.begin(3);
    load.accept_progress(
        PageProgress {
            response_url: "gemini://example/moved".into(),
            ..progress(3, b"# Pa")
        },
        40,
    );
    let answer = load.accept_outcome(
        outcome(
            3,
            Ok(Fetched {
                content_type: None,
                content_disposition: None,
                bytes: b"# Page".to_vec(),
                body: "# Page".into(),
                http_response: None,
            }),
        ),
        90,
    );
    assert_eq!(answer, LoadAnswer::Document);
    let document = load.fetched(PAGE).expect("settled");
    assert_eq!(document.bytes, b"# Page");
    assert_eq!(
        document.effective_url.as_deref(),
        Some("gemini://example/moved")
    );
    assert_eq!(document.content_type.as_deref(), Some("text/gemini"));
    assert!(document.http_response.is_none());
    assert_eq!(document.acquired_at_ms, 40);
    assert_eq!(
        load.phase(),
        Some(&LoadPhase::Settled { received_bytes: 6 })
    );
}

#[test]
fn terminal_http_response_replaces_the_streamed_prefix() {
    let url = "https://example.test/requested";
    let final_url = "https://example.test/final/page";
    let headers = [
        ("content-security-policy".into(), "frame-src 'self'".into()),
        ("referrer-policy".into(), "origin".into()),
        ("content-security-policy".into(), "script-src 'self'".into()),
    ];
    let mut load = PageLoad::default();
    load.begin(11);
    assert_eq!(
        load.accept_progress(
            PageProgress {
                request: 11,
                url: url.into(),
                response_url: "https://example.test/prefix".into(),
                content_type: Some("text/html".into()),
                bytes: b"<html>".to_vec(),
            },
            40,
        ),
        Some(6)
    );
    let prefix = load.fetched(url).expect("prefix is retained");
    assert_eq!(
        prefix.effective_url.as_deref(),
        Some("https://example.test/prefix")
    );
    assert!(prefix.http_response.is_none());

    assert_eq!(
        load.accept_outcome(
            FetchOutcome {
                request: 11,
                url: url.into(),
                result: Ok(Fetched {
                    http_response: Some(HttpDocumentResponse::new(final_url, headers.clone())),
                    ..Fetched::text(None, "<html></html>")
                }),
            },
            90,
        ),
        LoadAnswer::Document
    );
    let document = load.fetched(url).expect("terminal response is retained");
    assert!(load.fetched(final_url).is_none());
    assert_eq!(document.bytes, b"<html></html>");
    assert_eq!(document.content_type.as_deref(), Some("text/html"));
    assert_eq!(document.effective_url.as_deref(), Some(final_url));
    assert_eq!(document.acquired_at_ms, 40);
    let http_response = document
        .http_response
        .as_ref()
        .expect("HTTP response is retained");
    assert_eq!(http_response.final_url(), final_url);
    assert_eq!(http_response.policy_headers(), headers.as_slice());
}

#[test]
fn a_stale_http_response_cannot_replace_the_current_policy() {
    let url = "https://example.test/requested";
    let mut load = PageLoad::default();
    load.begin(21);
    load.begin(22);
    let answer = |request, final_url, policy: &str| FetchOutcome {
        request,
        url: url.into(),
        result: Ok(Fetched {
            http_response: Some(HttpDocumentResponse::new(
                final_url,
                [("referrer-policy".into(), policy.into())],
            )),
            ..Fetched::text(Some("text/html".into()), "<html></html>")
        }),
    };
    assert_eq!(
        load.accept_outcome(answer(22, "https://example.test/current", "origin"), 80),
        LoadAnswer::Document
    );
    assert_eq!(
        load.accept_outcome(answer(21, "https://example.test/stale", "unsafe-url"), 90),
        LoadAnswer::Stale
    );
    let document = load.fetched(url).expect("current response is retained");
    assert_eq!(
        document.effective_url.as_deref(),
        Some("https://example.test/current")
    );
    let http_response = document
        .http_response
        .as_ref()
        .expect("HTTP response is retained");
    assert_eq!(
        http_response.policy_headers(),
        [("referrer-policy".to_owned(), "origin".to_owned())]
    );
}

#[test]
fn downloads_are_handed_back_not_retained() {
    let mut load = PageLoad::default();
    load.begin(4);
    let binary = Fetched {
        content_type: Some("application/octet-stream".into()),
        content_disposition: None,
        bytes: vec![0, 1],
        body: "\0\u{1}".into(),
        http_response: None,
    };
    assert_eq!(
        load.accept_outcome(outcome(4, Ok(binary.clone())), 1),
        LoadAnswer::Download(binary)
    );
    assert!(load.fetched(PAGE).is_none());
}

#[test]
fn conversations_stay_typed_and_keep_the_last_document() {
    let mut load = PageLoad::default();
    load.note_fetched(
        PAGE.into(),
        LoadedDocument {
            bytes: b"# Old".to_vec(),
            content_type: None,
            body: "# Old".into(),
            effective_url: None,
            http_response: None,
            acquired_at_ms: 1,
        },
        5,
    );
    load.begin(5);
    let input = FetchFailure::InputRequired {
        url: PAGE.into(),
        prompt: "Name?".into(),
        sensitive: false,
    };
    assert_eq!(
        load.accept_outcome(outcome(5, Err(input.clone())), 2),
        LoadAnswer::Failed(input)
    );
    assert!(!load.in_progress());
    assert_eq!(load.phase(), None);
    assert!(load.fetched(PAGE).is_some());
}

#[test]
fn binary_progress_does_not_stream() {
    let mut load = PageLoad::default();
    load.begin(6);
    let image = PageProgress {
        content_type: Some("image/png".into()),
        ..progress(6, &[0x89, b'P'])
    };
    assert_eq!(load.accept_progress(image, 1), None);
    assert_eq!(load.phase(), Some(&LoadPhase::Requested));
}

#[test]
fn attachment_and_unrenderable_media_download_but_text_stays_a_document() {
    assert!(is_download_response(&Fetched {
        content_type: Some("text/plain".into()),
        content_disposition: Some("attachment; filename=notes.txt".into()),
        bytes: b"notes".to_vec(),
        body: "notes".into(),
        http_response: None,
    }));
    assert!(is_download_response(&Fetched {
        content_type: Some("application/octet-stream".into()),
        content_disposition: None,
        bytes: vec![0, 1],
        body: "\0\u{1}".into(),
        http_response: None,
    }));
    assert!(!is_download_response(&Fetched::text(
        Some("text/gemini; charset=utf-8".into()),
        "# Page",
    )));
    assert!(!is_download_response(&Fetched::text(
        Some("application/gopher-menu".into()),
        "iInfo\t\t\t",
    )));
}

#[test]
fn request_ids_are_unique() {
    let first = next_fetch_request_id();
    assert!(next_fetch_request_id() > first);
}
