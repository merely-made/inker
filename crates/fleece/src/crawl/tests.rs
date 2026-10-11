// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;
use std::cell::{Cell, RefCell};
use std::sync::atomic::AtomicUsize;

fn policy() -> CrawlPolicy {
    CrawlPolicy {
        polite_delay: Duration::ZERO,
        ..CrawlPolicy::default()
    }
}

fn html(body: &str) -> Fetched {
    Fetched::text(Some("text/html".into()), body)
}

fn run(
    site: &[(&str, Fetched)],
    policy: CrawlPolicy,
) -> (CrawlSummary, Vec<CrawlRequest>, Vec<String>) {
    let requests = RefCell::new(Vec::new());
    let mut content = Vec::new();
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    let summary = runtime.block_on(run_crawl(
        "https://s.test/",
        policy,
        |request: CrawlRequest| {
            let response = site
                .iter()
                .find(|(url, _)| *url == request.url)
                .map(|(_, response)| response.clone())
                .ok_or(CrawlFetchError::NotFound { received_bytes: 0 });
            requests.borrow_mut().push(request);
            async move { response }
        },
        || false,
        |frontier, url, depth, fetched| {
            enqueue_fetched_html_links(frontier, url, depth, fetched);
            vec![url.to_string()]
        },
        |update| {
            if let CrawlUpdate::Content { content: gathered } = update {
                content.extend(gathered);
            }
        },
    ));
    (summary, requests.into_inner(), content)
}

#[test]
fn frontier_bounds_links_and_sitemap_by_depth_fanout_and_scope() {
    for scope in [
        HostScope::SameHost,
        HostScope::SameDomain,
        HostScope::AnyHost,
    ] {
        assert_eq!(HostScope::from_key(scope.as_key()), Some(scope));
    }
    assert_eq!(HostScope::from_key("unknown"), None);
    let mut frontier = Frontier::new(
        "https://s.test/",
        CrawlPolicy {
            max_fanout: 1,
            ..policy()
        },
    );
    assert_eq!(frontier.next(), Some(("https://s.test/".into(), 0)));
    assert_eq!(
        frontier.enqueue_seeds(&["https://s.test/a".into(), "https://s.test/b".into()]),
        1
    );
    assert_eq!(
        frontier.enqueue(&["https://other.test/".into(), "file:///x".into()], 0),
        0
    );
    assert_eq!(
        frontier.enqueue(&["https://s.test/a#fragment".into()], 0),
        0
    );
    assert_eq!(frontier.enqueue(&["https://s.test/deep".into()], 1), 0);
    assert_eq!(
        frontier.enqueue(&["https://s.test/overflow".into()], u32::MAX),
        0
    );
    assert_eq!(frontier.next(), Some(("https://s.test/a".into(), 1)));
    assert!(frontier.next().is_none());
    let mut zero_depth = Frontier::new(
        "https://s.test/",
        CrawlPolicy {
            max_depth: 0,
            ..policy()
        },
    );
    assert_eq!(zero_depth.enqueue_seeds(&["https://s.test/a".into()]), 0);
}

#[test]
fn frontier_never_queues_more_than_the_page_budget_and_rejects_non_http_seed() {
    let mut frontier = Frontier::new(
        "https://s.test/",
        CrawlPolicy {
            max_pages: 2,
            ..policy()
        },
    );
    assert_eq!(
        frontier.enqueue(&["https://s.test/a".into(), "https://s.test/b".into()], 0),
        1
    );
    assert!(frontier.next().is_some());
    assert!(frontier.next().is_some());
    assert!(frontier.next().is_none());
    assert!(Frontier::new("file:///local", policy()).next().is_none());
    assert!(
        Frontier::new("https://name:secret@s.test/", policy())
            .next()
            .is_none()
    );
}

#[test]
fn supplied_html_and_xhtml_use_extraction_without_fetching() {
    for mime in ["text/html; charset=utf-8", "application/xhtml+xml"] {
        let mut frontier = Frontier::new("https://s.test/a/", policy());
        frontier.next();
        let response = Fetched::text(
            Some(mime.into()),
            "<html><body><a href='../b#one'>B</a><a href='../b#two'>again</a></body></html>",
        );
        assert_eq!(
            enqueue_fetched_html_links(&mut frontier, "https://s.test/a/", 0, &response),
            1
        );
        assert_eq!(frontier.next(), Some(("https://s.test/b".into(), 1)));
    }
    let mut frontier = Frontier::new("https://s.test/", policy());
    frontier.next();
    let response = Fetched::text(Some("application/json".into()), "<a href='/x'>not HTML</a>");
    assert_eq!(
        enqueue_fetched_html_links(&mut frontier, "https://s.test/", 0, &response),
        0
    );
    assert!(frontier.next().is_none());
}

#[test]
fn generic_content_is_gathered_with_depth_and_cycle_limits() {
    let (summary, requests, content) = run(
        &[
            (
                "https://s.test/",
                html("<a href='/a'>A</a><a href='/b'>B</a>"),
            ),
            (
                "https://s.test/a",
                html("<a href='/'>cycle</a><a href='/c'>deep</a>"),
            ),
            ("https://s.test/b", html("<p>leaf</p>")),
            ("https://s.test/c", html("<p>deep</p>")),
        ],
        policy(),
    );
    assert_eq!(summary.fetched, 3);
    assert_eq!(summary.requests, 4); // robots + three pages
    assert_eq!(
        content,
        ["https://s.test/", "https://s.test/a", "https://s.test/b"]
    );
    assert_eq!(requests[0].kind, CrawlRequestKind::Robots);
    assert!(
        requests
            .iter()
            .all(|request| request.max_bytes == policy().max_response_bytes)
    );
}

#[test]
fn sitemap_and_robots_count_against_request_budget() {
    let (summary, requests, content) = run(
        &[
            (
                "https://s.test/sitemap.xml",
                html("<urlset><url><loc>https://s.test/a</loc></url></urlset>"),
            ),
            ("https://s.test/", html("<p>seed</p>")),
            ("https://s.test/a", html("<p>leaf</p>")),
        ],
        CrawlPolicy {
            seed_sitemap: true,
            max_requests: 3,
            ..policy()
        },
    );
    assert_eq!(summary.stop, CrawlStop::RequestBudget);
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests
            .iter()
            .map(|request| request.kind)
            .collect::<Vec<_>>(),
        [
            CrawlRequestKind::Robots,
            CrawlRequestKind::Sitemap,
            CrawlRequestKind::Page
        ]
    );
    assert_eq!(content, ["https://s.test/"]);
    let (summary, requests, _) = run(
        &[("https://s.test/", html("seed"))],
        CrawlPolicy {
            max_requests: 1,
            ..policy()
        },
    );
    assert_eq!(summary.stop, CrawlStop::RequestBudget);
    assert_eq!(requests.len(), 1);
    assert_eq!(summary.fetched, 0);
}

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
fn robots_blocks_pages_sitemap_and_percent_escape_bypasses() {
    let (summary, requests, content) = run(
        &[
            (
                "https://s.test/robots.txt",
                html("User-agent: *\nDisallow: /private\nDisallow: /sitemap.xml"),
            ),
            (
                "https://s.test/",
                html("<a href='/ok'>OK</a><a href='/%70rivate/secret'>private</a>"),
            ),
            ("https://s.test/ok", html("ok")),
        ],
        CrawlPolicy {
            seed_sitemap: true,
            ..policy()
        },
    );
    assert_eq!(summary.robots_skipped, 1);
    assert_eq!(summary.fetched, 2);
    assert_eq!(requests.len(), 3);
    assert!(
        !requests
            .iter()
            .any(|request| request.kind == CrawlRequestKind::Sitemap)
    );
    assert_eq!(content, ["https://s.test/", "https://s.test/ok"]);
}

#[test]
fn unreachable_robots_fails_closed_and_accounts_partial_bytes() {
    let mut calls = 0;
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    let summary = runtime.block_on(run_crawl(
        "https://s.test/",
        policy(),
        |_| {
            calls += 1;
            async {
                Err(CrawlFetchError::Failed {
                    message: "offline".into(),
                    received_bytes: 4,
                })
            }
        },
        || false,
        |_, _, _, _| Vec::<()>::new(),
        |_| {},
    ));
    assert_eq!(calls, 1);
    assert_eq!(summary.received_bytes, 4);
    assert_eq!(summary.failed_requests, 1);
    assert_eq!(summary.robots_skipped, 1);
    assert_eq!(summary.fetched, 0);
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
fn robots_are_cached_per_origin_including_port() {
    let (summary, requests, content) = run(
        &[
            (
                "https://s.test/",
                html("<a href='https://s.test:8443/private'>other port</a>"),
            ),
            (
                "https://s.test:8443/robots.txt",
                html("User-agent: *\nDisallow: /private"),
            ),
        ],
        policy(),
    );
    assert_eq!(summary.fetched, 1);
    assert_eq!(summary.robots_skipped, 1);
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.kind == CrawlRequestKind::Robots)
            .count(),
        2
    );
    assert_eq!(content, ["https://s.test/"]);
}

#[test]
fn cancelled_before_start_sends_no_requests_and_invalid_seed_has_typed_receipt() {
    let calls = Cell::new(0);
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    let summary = runtime.block_on(run_crawl(
        "https://s.test/",
        CrawlPolicy {
            seed_sitemap: true,
            ..policy()
        },
        |_| {
            calls.set(calls.get() + 1);
            async { Ok(html("")) }
        },
        || true,
        |_, _, _, _| Vec::<()>::new(),
        |_| {},
    ));
    assert_eq!(summary.stop, CrawlStop::Cancelled);
    assert_eq!(calls.get(), 0);
    assert!(summary.received_bytes_complete);
    let summary = runtime.block_on(run_crawl(
        "engine:local-node",
        policy(),
        |_| async { Ok(html("")) },
        || false,
        |_, _, _, _| Vec::<()>::new(),
        |_| {},
    ));
    assert_eq!(summary.stop, CrawlStop::InvalidSeed);
    assert_eq!(summary.requests, 0);
}

#[test]
fn in_flight_request_is_dropped_on_cancellation_and_timeout() {
    let cancelled = Cell::new(false);
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    let summary = runtime.block_on(run_crawl(
        "https://s.test/",
        policy(),
        |_| {
            poll_fn(|_| {
                cancelled.set(true);
                Poll::Pending::<Result<Fetched, CrawlFetchError>>
            })
        },
        || cancelled.get(),
        |_, _, _, _| Vec::<()>::new(),
        |_| {},
    ));
    assert_eq!(summary.stop, CrawlStop::Cancelled);
    assert_eq!(summary.requests, 1);
    assert!(!summary.received_bytes_complete);
    let summary = runtime.block_on(run_crawl(
        "https://s.test/",
        CrawlPolicy {
            request_timeout: Duration::ZERO,
            ..policy()
        },
        |_| std::future::pending::<Result<Fetched, CrawlFetchError>>(),
        || false,
        |_, _, _, _| Vec::<()>::new(),
        |_| {},
    ));
    assert!(matches!(summary.stop, CrawlStop::RequestTimeout { .. }));
    assert_eq!(summary.requests, 1);
    assert!(!summary.received_bytes_complete);
}

#[test]
fn polite_delay_applies_to_robots_sitemap_and_page_requests() {
    let requests = RefCell::new(Vec::new());
    let delay = Duration::from_millis(10);
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    runtime.block_on(run_crawl(
        "https://s.test/",
        CrawlPolicy {
            seed_sitemap: true,
            polite_delay: delay,
            ..policy()
        },
        |request| {
            requests.borrow_mut().push((request, Instant::now()));
            async { Ok(html("")) }
        },
        || false,
        |_, _, _, _| Vec::<()>::new(),
        |_| {},
    ));
    let requests = requests.into_inner();
    assert_eq!(requests.len(), 3);
    for pair in requests.windows(2) {
        assert!(pair[1].1.duration_since(pair[0].1) >= delay);
    }
}

#[test]
fn zero_request_byte_and_page_budgets_do_not_invoke_fetcher() {
    for (policy, expected) in [
        (
            CrawlPolicy {
                max_requests: 0,
                ..policy()
            },
            CrawlStop::RequestBudget,
        ),
        (
            CrawlPolicy {
                max_total_bytes: 0,
                ..policy()
            },
            CrawlStop::TotalByteBudget,
        ),
        (
            CrawlPolicy {
                max_pages: 0,
                ..policy()
            },
            CrawlStop::PageBudget,
        ),
    ] {
        let (summary, requests, content) = run(&[], policy);
        assert_eq!(summary.stop, expected);
        assert!(requests.is_empty());
        assert!(content.is_empty());
    }
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
fn creating_a_gathering_actor_does_not_start_network_acquisition() {
    let calls = Arc::new(AtomicUsize::new(0));
    let fetch_calls = calls.clone();
    let fetch: CrawlFetch = Arc::new(move |_| {
        fetch_calls.fetch_add(1, Ordering::Relaxed);
        Box::pin(async { Ok(html("")) })
    });
    let (handle, _) = spawn_crawl(
        Arc::new(|| {}),
        fetch,
        Arc::new(|_, _, _, _| Vec::<()>::new()),
    );
    handle.join();
    assert_eq!(calls.load(Ordering::Relaxed), 0);
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
