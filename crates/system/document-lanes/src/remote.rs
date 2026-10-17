/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The remote half of a host's resource fetcher, [`RemoteFetcher`]: http(s)
//! document loading over the netfetcher engine (the `netfetch` feature), and smolweb
//! (gemini/gopher/nex/finger/spartan/guppy) over the errand transport (the `smolweb`
//! feature). A host composes it under `genet_documents::LocalFetcher::with_fallback`.
//!
//! pelt is genet's reference *host*, so -- like meerkat in the product -- it owns
//! networking and drives the sibling engines ([`netfetcher`] for the web, errand for
//! smolweb); genet's engine components stay byte-consuming and never link them.
//! `ResourceFetcher::fetch` is synchronous, so the engines' async `fetch` is bridged
//! onto it through a small tokio runtime, block-on per request -- the document load is
//! a one-shot at open time, not a per-frame cost. The same wiring genet-wpt's
//! `fetch()` uses.

use std::sync::OnceLock;
#[cfg(feature = "netfetch")]
use std::sync::{Arc, Condvar, Mutex};
#[cfg(feature = "netfetch")]
use std::time::Instant;

#[cfg(feature = "netfetch")]
use bytes::BytesMut;
#[cfg(any(feature = "netfetch", feature = "smolweb"))]
use tokio::runtime::Runtime;

use genet_host_api::{
    ResourceFetchPolicy, ResourceFetchResponse, ResourceFetcher, ResourceRequest, ResourceResponse,
};

#[cfg(any(feature = "netfetch", feature = "smolweb"))]
/// The shared tokio runtime the blocking bridge drives. Built once on first use: a
/// multithread runtime lets the host policy admit several independent document
/// sessions without creating a private runtime per resource. `enable_all`
/// lights the IO + time drivers netfetcher's transport needs.
fn runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("pelt netfetch tokio runtime")
    })
}

/// One shared remote-fetch host. Its context owns HTTP cache revalidation and
/// redirect policy; its permit pool bounds all simultaneous resource fetches.
/// It intentionally lives at the port boundary, not in a style engine.
#[cfg(feature = "netfetch")]
pub(crate) struct HttpResourceHost {
    context: netfetcher::FetchContext,
    policy: ResourceFetchPolicy,
    available: Mutex<usize>,
    changed: Condvar,
}

#[cfg(feature = "netfetch")]
impl HttpResourceHost {
    pub(crate) fn new(policy: ResourceFetchPolicy) -> Self {
        let mut context =
            netfetcher::FetchContext::permissive().with_redirect_limit(policy.max_redirects);
        context.cache = Arc::new(netfetcher::InMemoryHttpCache::new());
        Self {
            context,
            policy,
            available: Mutex::new(policy.max_concurrent_fetches),
            changed: Condvar::new(),
        }
    }

    fn acquire_until(&self, deadline: Instant) -> Option<HttpFetchPermit<'_>> {
        let mut available = self.available.lock().expect("HTTP fetch permit lock");
        while *available == 0 {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return None;
            }
            let (waiting, _) = self
                .changed
                .wait_timeout(available, remaining)
                .expect("HTTP fetch permit wait");
            available = waiting;
        }
        *available -= 1;
        Some(HttpFetchPermit { host: self })
    }

    pub(crate) fn fetch_request(&self, request: &ResourceRequest) -> Option<ResourceFetchResponse> {
        if self.policy.max_concurrent_fetches == 0 {
            return None;
        }
        let deadline = Instant::now().checked_add(self.policy.timeout)?;
        let wire = netfetcher::Request::from_host(request)?;
        let _permit = self.acquire_until(deadline)?;
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return None;
        }
        let policy = self.policy;
        runtime().block_on(async move {
            tokio::time::timeout(remaining, async move {
                let response = netfetcher::fetch(wire, &self.context).await;
                if response.is_network_error() {
                    return None;
                }
                let final_url = response
                    .url_list
                    .last()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| request.url.clone());
                let content_type = response
                    .headers
                    .iter()
                    .rev()
                    .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
                    .map(|(_, value)| value.clone());
                let status = response.status;
                let headers = response.headers.clone();
                let response_type = match response.response_type {
                    netfetcher::ResponseType::Basic => "basic",
                    netfetcher::ResponseType::Cors => "cors",
                    netfetcher::ResponseType::Opaque => "opaque",
                    netfetcher::ResponseType::OpaqueRedirect => "opaqueredirect",
                    netfetcher::ResponseType::Error => "error",
                }
                .to_owned();
                let redirected = response.url_list.len() > 1;
                let mut body = response.body;
                let mut bytes = BytesMut::new();
                while let Some(chunk) = body.next_chunk().await {
                    let chunk = chunk.ok()?;
                    if bytes.len().saturating_add(chunk.len()) > policy.max_response_bytes {
                        return None;
                    }
                    bytes.extend_from_slice(&chunk);
                }
                Some(ResourceFetchResponse {
                    final_url,
                    content_type,
                    bytes: bytes.to_vec(),
                    status,
                    // netfetcher does not expose the transport reason phrase.
                    status_text: String::new(),
                    headers,
                    response_type,
                    redirected,
                })
            })
            .await
            .ok()
            .flatten()
        })
    }
}

#[cfg(feature = "netfetch")]
struct HttpFetchPermit<'a> {
    host: &'a HttpResourceHost,
}

#[cfg(feature = "netfetch")]
impl Drop for HttpFetchPermit<'_> {
    fn drop(&mut self) {
        if let Ok(mut available) = self.host.available.lock() {
            *available += 1;
            self.host.changed.notify_one();
        }
    }
}

/// The remote half of a host's resource fetcher: http(s) over netfetcher
/// (feature `netfetch`) and the smolweb schemes over errand (feature
/// `smolweb`). Every other scheme is `None`; a host composes it under
/// `genet_documents::LocalFetcher::with_fallback`. Clones share one HTTP
/// cache, redirect cap and concurrency budget.
#[derive(Clone)]
pub struct RemoteFetcher {
    #[cfg(feature = "netfetch")]
    http: Arc<HttpResourceHost>,
}

impl RemoteFetcher {
    /// A fetcher with its own HTTP cache, redirect cap and concurrency budget.
    pub fn new(policy: ResourceFetchPolicy) -> Self {
        #[cfg(not(feature = "netfetch"))]
        let _ = policy;
        Self {
            #[cfg(feature = "netfetch")]
            http: Arc::new(HttpResourceHost::new(policy)),
        }
    }

    /// One process-shared fetcher under the default policy, so document loads
    /// and every shared-resource resolver pass reuse the same cache, redirect
    /// cap and concurrency budget.
    pub fn shared() -> Self {
        static SHARED: OnceLock<RemoteFetcher> = OnceLock::new();
        SHARED
            .get_or_init(|| Self::new(ResourceFetchPolicy::default()))
            .clone()
    }
}

impl ResourceFetcher for RemoteFetcher {
    fn fetch(&self, url: &str) -> Option<Vec<u8>> {
        self.fetch_response(url).map(|response| response.bytes)
    }

    fn fetch_response(&self, url: &str) -> Option<ResourceResponse> {
        #[cfg(feature = "smolweb")]
        if url
            .split_once("://")
            .and_then(|(scheme, _)| errand::Scheme::parse(scheme))
            .is_some()
        {
            return smolweb_get_bytes(url).map(|bytes| ResourceResponse::new(url, bytes));
        }
        let response = self.fetch_request(&ResourceRequest::plain_get(url))?;
        (200..300)
            .contains(&response.status)
            .then(|| response.into_legacy_response())
    }

    fn fetch_request(&self, request: &ResourceRequest) -> Option<ResourceFetchResponse> {
        #[cfg(feature = "netfetch")]
        if request.url.starts_with("http://") || request.url.starts_with("https://") {
            return self.http.fetch_request(request);
        }
        let _ = request;
        None
    }
}

/// Blocking smolweb GET of `url` over the errand transport, returning the response
/// body on a success status, or `None` on a non-success status (input / redirect /
/// failure / cert-required) or a transport error. The smolweb branch of
/// [`RemoteFetcher`]; mirrors `http_get_bytes`,
/// bridging errand's async `fetch` onto the sync `ResourceFetcher` through the shared
/// runtime. The caller surfaces the `None` as a clean load error rather than painting
/// a protocol error line as a document, matching the http path's non-2xx handling.
#[cfg(feature = "smolweb")]
fn smolweb_get_bytes(url: &str) -> Option<Vec<u8>> {
    install_tofu();
    runtime().block_on(async move {
        match errand::fetch(url).await {
            Ok(resp) if resp.status == errand::Status::Success => Some(resp.body),
            _ => None,
        }
    })
}

/// Install an [`errand::InMemoryTofu`] once for the process, so gemini certificate
/// pins persist across requests in a session: a first contact is trusted-on-first-use
/// and a later mismatch (a possible MITM or a key rotation) surfaces as a failed load
/// rather than a silent re-pin. Without this errand defaults to accept-any
/// (`PermissiveTofu`); the reference shell opts into real pinning. A durable on-disk
/// store is a later rung.
#[cfg(feature = "smolweb")]
fn install_tofu() {
    use std::sync::OnceLock;
    static INSTALLED: OnceLock<()> = OnceLock::new();
    INSTALLED.get_or_init(|| {
        errand::set_trust_store(std::sync::Arc::new(errand::InMemoryTofu::new()));
    });
}

#[cfg(all(test, feature = "netfetch"))]
mod tests {
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
        *host.available.lock().unwrap() = 0;
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
}

#[cfg(all(test, feature = "smolweb"))]
mod smolweb_tests {
    use genet_host_api::ResourceFetcher;

    use crate::RemoteFetcher;

    /// A smolweb scheme is recognized and routed to the errand transport, and a host
    /// that cannot resolve fails to a clean `None` (a failed load, not a panic or an
    /// error document) -- the same contract the http path holds for a non-2xx. Uses a
    /// `.invalid` host (RFC 6761 guarantees NXDOMAIN, answered locally) so the test
    /// needs no live capsule, and exercises the one-time TOFU install on the way.
    #[test]
    fn smolweb_scheme_routes_and_unresolvable_host_is_none() {
        assert!(
            RemoteFetcher::shared()
                .fetch("gemini://capsule.invalid/")
                .is_none(),
            "an unresolvable gemini host is a failed load, not a document"
        );
    }

    /// A non-smolweb, non-http unknown scheme is not routed to errand; it falls
    /// through to the filesystem attempt and fails to `None`.
    #[test]
    fn unknown_scheme_is_not_routed_to_errand() {
        assert!(
            RemoteFetcher::shared().fetch("wat://nope/").is_none(),
            "a non-smolweb scheme is not an errand fetch nor a readable path"
        );
    }
}
