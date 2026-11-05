/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::time::{Duration, Instant};

use genet_host_api::{ResourceFetchPolicy, ResourceFetcher, ResourceRequest};

use super::HttpResourceHost;
use crate::RemoteFetcher;

/// http(s) loading flows through the netfetcher engine end to end: an offline
/// mock server serves a body, and `RemoteFetcher` (with the `netfetch` branch)
/// fetches its bytes -- the same path `pelt --engine static https://…` takes,
/// proven without a live network.
#[test]
fn local_fetcher_gets_http_bytes_via_netfetcher() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("GET", "/page.html")
        .with_status(200)
        .with_header("content-type", "text/html; charset=utf-8")
        .with_body("<h1>From the network</h1>")
        .create();

    let url = format!("{}/page.html", server.url());
    let bytes = RemoteFetcher::shared()
        .fetch(&url)
        .expect("the http(s) document fetches over netfetcher");
    assert_eq!(
        bytes, b"<h1>From the network</h1>",
        "the fetched bytes are the served body"
    );
    mock.assert();
}

/// A non-2xx response is `None` (the caller surfaces a load error), not the error
/// body painted as a document.
#[test]
fn http_not_found_is_none() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("GET", "/missing")
        .with_status(404)
        .with_body("nope")
        .create();

    let url = format!("{}/missing", server.url());
    assert!(
        RemoteFetcher::shared().fetch(&url).is_none(),
        "a 404 is a failed load, not a document"
    );
    mock.assert();
}

#[test]
fn rich_request_preserves_post_body_and_error_response_metadata() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("POST", "/submit")
        .match_header("content-type", "application/octet-stream")
        .match_body(vec![0, 128, 255])
        .with_status(422)
        .with_header("content-type", "text/plain")
        .with_header("x-reason", "invalid")
        .with_body("rejected")
        .create();
    let mut request = ResourceRequest::plain_get(format!("{}/submit", server.url()));
    request.method = "POST".into();
    request
        .headers
        .push(("content-type".into(), "application/octet-stream".into()));
    request.body = Some(vec![0, 128, 255]);
    let response = RemoteFetcher::new(ResourceFetchPolicy::default())
        .fetch_request(&request)
        .expect("HTTP error is still a transport response");
    assert_eq!(response.status, 422);
    assert_eq!(response.bytes, b"rejected");
    assert_eq!(response.final_url, request.url);
    assert_eq!(response.content_type.as_deref(), Some("text/plain"));
    assert!(
        response
            .headers
            .iter()
            .any(|(name, value)| name == "x-reason" && value == "invalid")
    );
    mock.assert();
}

#[test]
fn unsupported_rich_policy_is_refused_before_transport() {
    let mut request = ResourceRequest::plain_get("http://127.0.0.1:9/unused");
    request.redirect = "silently-follow".into();
    assert!(
        RemoteFetcher::new(ResourceFetchPolicy::default())
            .fetch_request(&request)
            .is_none()
    );
}

#[test]
fn admission_wait_obeys_the_same_elapsed_deadline() {
    let policy = ResourceFetchPolicy {
        max_concurrent_fetches: 1,
        timeout: Duration::from_millis(20),
        ..ResourceFetchPolicy::default()
    };
    let host = HttpResourceHost::new(policy);
    *host.budget.available.lock().unwrap() = 0;
    let request = ResourceRequest::plain_get("http://127.0.0.1:9/unreached");
    let started = Instant::now();
    assert!(host.fetch_request(&request).is_none());
    assert!(started.elapsed() < Duration::from_secs(1));

    let zero = HttpResourceHost::new(ResourceFetchPolicy {
        max_concurrent_fetches: 0,
        ..policy
    });
    let started = Instant::now();
    assert!(zero.fetch_request(&request).is_none());
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn rich_response_keeps_redirect_final_identity() {
    let mut server = mockito::Server::new();
    let moved = server
        .mock("GET", "/moved")
        .with_status(302)
        .with_header("location", "/final")
        .create();
    let final_page = server
        .mock("GET", "/final")
        .with_status(200)
        .with_body("final")
        .create();
    let request = ResourceRequest::plain_get(format!("{}/moved", server.url()));
    let response = RemoteFetcher::new(ResourceFetchPolicy::default())
        .fetch_request(&request)
        .expect("redirected response");
    assert_eq!(response.final_url, format!("{}/final", server.url()));
    assert!(response.redirected);
    assert_eq!(response.bytes, b"final");
    moved.assert();
    final_page.assert();
}

#[test]
fn configured_fetcher_revalidates_a_shared_cached_response() {
    let mut server = mockito::Server::new();
    let initial = server
        .mock("GET", "/revision.css")
        .match_header("if-none-match", mockito::Matcher::Missing)
        .with_status(200)
        .with_header("cache-control", "max-age=0")
        .with_header("etag", "\"v1\"")
        .with_body("body { color: red; }")
        .expect(1)
        .create();
    let revalidated = server
        .mock("GET", "/revision.css")
        .match_header("if-none-match", "\"v1\"")
        .with_status(304)
        .expect(1)
        .create();
    let fetcher = RemoteFetcher::new(ResourceFetchPolicy::default());
    let url = format!("{}/revision.css", server.url());
    let first = fetcher.fetch_response(&url).expect("initial response");
    let second = fetcher.fetch_response(&url).expect("revalidated response");
    assert_eq!(first.bytes, second.bytes);
    initial.assert();
    revalidated.assert();
}

#[test]
fn configured_fetcher_enforces_redirect_and_body_limits() {
    let mut server = mockito::Server::new();
    let redirect = server
        .mock("GET", "/redirect")
        .with_status(302)
        .with_header("location", "/target")
        .expect(1)
        .create();
    let oversized = server
        .mock("GET", "/oversized")
        .with_status(200)
        .with_body("four")
        .expect(1)
        .create();
    let fetcher = RemoteFetcher::new(ResourceFetchPolicy {
        max_redirects: 0,
        max_response_bytes: 3,
        ..ResourceFetchPolicy::default()
    });
    assert!(
        fetcher
            .fetch(&format!("{}/redirect", server.url()))
            .is_none()
    );
    assert!(
        fetcher
            .fetch(&format!("{}/oversized", server.url()))
            .is_none()
    );
    redirect.assert();
    oversized.assert();
}
