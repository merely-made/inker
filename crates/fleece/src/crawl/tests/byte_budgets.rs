// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

#[test]
fn response_and_total_bytes_bound_adapter_and_reject_oversized_results() {
    let (summary, requests, content) = run(
        &[("https://s.test/", html("123456"))],
        CrawlPolicy {
            max_response_bytes: 5,
            ..policy()
        },
    );
    assert!(matches!(
        summary.stop,
        CrawlStop::ResponseByteLimit {
            cap: 5,
            received: 6,
            ..
        }
    ));
    assert_eq!(requests[1].max_bytes, 5);
    assert!(content.is_empty());
    let (summary, requests, _) = run(
        &[
            ("https://s.test/robots.txt", html("1234")),
            ("https://s.test/", html("1234567")),
        ],
        CrawlPolicy {
            max_response_bytes: 20,
            max_total_bytes: 10,
            ..policy()
        },
    );
    assert_eq!(requests[1].max_bytes, 6);
    assert_eq!(summary.stop, CrawlStop::TotalByteBudget);
    assert_eq!(summary.received_bytes, 11); // reports the deliberately broken adapter
}

#[test]
fn failed_page_response_bytes_reduce_subsequent_request_allowance() {
    let requests = RefCell::new(Vec::new());
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    let summary = runtime.block_on(run_crawl(
        "https://s.test/",
        CrawlPolicy {
            max_total_bytes: 64,
            max_response_bytes: 64,
            ..policy()
        },
        |request: CrawlRequest| {
            let result = match request.kind {
                CrawlRequestKind::Robots => Ok(html("")),
                _ if request.url.ends_with("/a") => Err(CrawlFetchError::Failed {
                    message: "reset".into(),
                    received_bytes: 8,
                }),
                _ if request.url.ends_with("/b") => Ok(html("leaf")),
                _ => Ok(html("<a href='/a'>A</a><a href='/b'>B</a>")),
            };
            requests.borrow_mut().push(request);
            async move { result }
        },
        || false,
        |frontier, url, depth, fetched| {
            enqueue_fetched_html_links(frontier, url, depth, fetched);
            Vec::<()>::new()
        },
        |_| {},
    ));
    let requests = requests.into_inner();
    assert_eq!(summary.failed_requests, 1);
    assert_eq!(requests[3].max_bytes, requests[2].max_bytes - 8);
    assert_eq!(summary.received_bytes, 48);
}

#[test]
fn single_hop_adapter_preserves_absence_failure_status_and_partial_byte_evidence() {
    let response = |status| fetch::crawler::CrawlerResponse {
        status,
        content_type: Some("text/plain".into()),
        content_disposition: Some("inline".into()),
        bytes: b"body".to_vec(),
    };
    let success = lower_crawler_result(Ok(response(200))).unwrap();
    assert_eq!(success.body, "body");
    assert_eq!(success.bytes, b"body");
    assert_eq!(success.content_type.as_deref(), Some("text/plain"));
    assert_eq!(success.content_disposition.as_deref(), Some("inline"));
    for status in [404, 410] {
        assert_eq!(
            lower_crawler_result(Ok(response(status))),
            Err(CrawlFetchError::NotFound { received_bytes: 4 })
        );
    }
    for status in [302, 401, 403, 500] {
        assert!(matches!(
            lower_crawler_result(Ok(response(status))),
            Err(CrawlFetchError::Failed {
                received_bytes: 4,
                ..
            })
        ));
    }
    assert_eq!(
        lower_crawler_result(Err(fetch::crawler::CrawlerFetchError::ResponseLimit {
            max_bytes: 5,
            received_bytes: 7,
        })),
        Err(CrawlFetchError::ResponseLimit { received_bytes: 7 })
    );
    assert_eq!(
        lower_crawler_result(Err(fetch::crawler::CrawlerFetchError::Network {
            message: "reset".into(),
            received_bytes: 3,
        }))
        .unwrap_err()
        .received_bytes(),
        3
    );
}

#[test]
fn ready_failed_fetch_preserves_complete_bytes_and_terminal_cancellation() {
    for cancel_kind in [CrawlRequestKind::Robots, CrawlRequestKind::Page] {
        let cancelled = Cell::new(false);
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let summary = runtime.block_on(run_crawl(
            "https://s.test/",
            policy(),
            |request: CrawlRequest| {
                let cancelled = &cancelled;
                async move {
                    if request.kind == cancel_kind {
                        cancelled.set(true);
                        Err(CrawlFetchError::Failed {
                            message: "request stopped".into(),
                            received_bytes: 3,
                        })
                    } else {
                        Ok(html(""))
                    }
                }
            },
            || cancelled.get(),
            |_, _, _, _| Vec::<()>::new(),
            |_| {},
        ));
        assert_eq!(summary.stop, CrawlStop::Cancelled);
        assert_eq!(summary.received_bytes, 3);
        assert!(summary.received_bytes_complete);
        assert_eq!(summary.failed_requests, 1);
        assert_eq!(summary.fetched, 0);
        assert_eq!(
            summary.requests,
            if cancel_kind == CrawlRequestKind::Robots {
                1
            } else {
                2
            }
        );
    }
}
