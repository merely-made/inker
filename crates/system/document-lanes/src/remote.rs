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
#[cfg(any(feature = "netfetch", feature = "smolweb"))]
use std::sync::{Arc, Condvar, Mutex};
#[cfg(any(feature = "netfetch", feature = "smolweb"))]
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
    budget: Arc<RemoteFetchBudget>,
}

#[cfg(any(feature = "netfetch", feature = "smolweb"))]
struct RemoteFetchBudget {
    policy: ResourceFetchPolicy,
    available: Mutex<usize>,
    changed: Condvar,
}

#[cfg(feature = "netfetch")]
impl HttpResourceHost {
    #[cfg(test)]
    pub(crate) fn new(policy: ResourceFetchPolicy) -> Self {
        Self::with_budget(Arc::new(RemoteFetchBudget::new(policy)))
    }

    fn with_budget(budget: Arc<RemoteFetchBudget>) -> Self {
        let mut context =
            netfetcher::FetchContext::permissive().with_redirect_limit(budget.policy.max_redirects);
        context.cache = Arc::new(netfetcher::InMemoryHttpCache::new());
        Self { context, budget }
    }
}

#[cfg(any(feature = "netfetch", feature = "smolweb"))]
impl RemoteFetchBudget {
    fn new(policy: ResourceFetchPolicy) -> Self {
        Self {
            policy,
            available: Mutex::new(policy.max_concurrent_fetches),
            changed: Condvar::new(),
        }
    }

    fn acquire_until(&self, deadline: Instant) -> Option<RemoteFetchPermit<'_>> {
        if self.policy.max_concurrent_fetches == 0
            || self.policy.max_response_bytes == 0
            || self.policy.timeout.is_zero()
        {
            return None;
        }
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
        Some(RemoteFetchPermit { host: self })
    }
}

#[cfg(feature = "netfetch")]
impl HttpResourceHost {
    pub(crate) fn fetch_request(&self, request: &ResourceRequest) -> Option<ResourceFetchResponse> {
        let deadline = Instant::now().checked_add(self.budget.policy.timeout)?;
        let wire = netfetcher::Request::from_host(request)?;
        let _permit = self.budget.acquire_until(deadline)?;
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return None;
        }
        let policy = self.budget.policy;
        runtime().block_on(async move {
            if Instant::now() >= deadline {
                return None;
            }
            tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), async move {
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

#[cfg(any(feature = "netfetch", feature = "smolweb"))]
struct RemoteFetchPermit<'a> {
    host: &'a RemoteFetchBudget,
}

#[cfg(any(feature = "netfetch", feature = "smolweb"))]
impl Drop for RemoteFetchPermit<'_> {
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
    #[cfg(any(feature = "netfetch", feature = "smolweb"))]
    budget: Arc<RemoteFetchBudget>,
    #[cfg(feature = "netfetch")]
    http: Arc<HttpResourceHost>,
    #[cfg(feature = "smolweb")]
    smolweb_request_bytes: usize,
    #[cfg(feature = "smolweb")]
    smolweb_header_line_bytes: usize,
}

impl RemoteFetcher {
    /// A fetcher with its own HTTP cache, redirect cap and concurrency budget.
    pub fn new(policy: ResourceFetchPolicy) -> Self {
        #[cfg(not(any(feature = "netfetch", feature = "smolweb")))]
        let _ = policy;
        #[cfg(any(feature = "netfetch", feature = "smolweb"))]
        let budget = Arc::new(RemoteFetchBudget::new(policy));
        Self {
            #[cfg(feature = "netfetch")]
            http: Arc::new(HttpResourceHost::with_budget(Arc::clone(&budget))),
            #[cfg(any(feature = "netfetch", feature = "smolweb"))]
            budget,
            #[cfg(feature = "smolweb")]
            smolweb_request_bytes: errand::FetchOptions::default().max_request_bytes,
            #[cfg(feature = "smolweb")]
            smolweb_header_line_bytes: errand::FetchOptions::default().max_header_line_bytes,
        }
    }

    /// Configure native request and response-line allowances. Body and total
    /// deadline remain those of the host's ResourceFetchPolicy. Native protocol
    /// ceilings still apply, and smolweb redirects remain unfollowed.
    #[cfg(feature = "smolweb")]
    pub fn with_smolweb_request_limits(
        mut self,
        request_bytes: usize,
        header_line_bytes: usize,
    ) -> Self {
        self.smolweb_request_bytes = request_bytes;
        self.smolweb_header_line_bytes = header_line_bytes;
        self
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
    fn max_concurrent_fetches(&self) -> usize {
        #[cfg(any(feature = "netfetch", feature = "smolweb"))]
        {
            self.budget.policy.max_concurrent_fetches
        }
        #[cfg(not(any(feature = "netfetch", feature = "smolweb")))]
        {
            0
        }
    }

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
            return smolweb_get_response(
                url,
                &self.budget,
                self.smolweb_request_bytes,
                self.smolweb_header_line_bytes,
            );
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
fn smolweb_get_response(
    url: &str,
    budget: &RemoteFetchBudget,
    request_bytes: usize,
    header_line_bytes: usize,
) -> Option<ResourceResponse> {
    let parsed = url::Url::parse(url).ok()?;
    // The shared-site plan (2026-10-20_shared_site_model_plan.md) requires
    // explicit Submit for Titan and Spartan query/data exchanges. This is
    // the same host read-admission rule as mere-fetch's redirect-hop guard;
    // keep this tiny predicate local rather than adding a fetch dependency.
    // Refuse before either trust setup or the general-purpose transport.
    if parsed.scheme() == "titan" || (parsed.scheme() == "spartan" && parsed.query().is_some()) {
        return None;
    }
    let deadline = Instant::now().checked_add(budget.policy.timeout)?;
    let _permit = budget.acquire_until(deadline)?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return None;
    }
    install_tofu();
    runtime().block_on(async move {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return None;
        }
        let options = errand::FetchOptions {
            timeout: remaining,
            max_request_bytes: request_bytes,
            max_header_line_bytes: header_line_bytes,
            max_body_bytes: budget.policy.max_response_bytes,
        };
        match tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            errand::fetch_with_options(url, &options),
        )
        .await
        .ok()?
        {
            Ok(resp) if resp.status == errand::Status::Success => {
                // These are host representation tags, not wire MIME fields.
                // Match mere-fetch's existing carrier policy so a synthetic
                // text/plain Finger reply keeps its literal native lane.
                let content_type = match errand::Scheme::parse(parsed.scheme()) {
                    Some(errand::Scheme::Finger) => Some("text/x-finger".to_string()),
                    Some(errand::Scheme::Nex) => Some("application/x-nex".to_string()),
                    Some(errand::Scheme::Guppy) => Some("application/x-guppy".to_string()),
                    Some(errand::Scheme::Titan) => Some("application/x-titan".to_string()),
                    _ => resp.mime().map(|_| resp.meta.clone()),
                };
                Some(ResourceResponse {
                    final_url: url.to_owned(),
                    content_type,
                    bytes: resp.body,
                })
            },
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
#[path = "remote/tests.rs"]
mod tests;

#[cfg(all(test, feature = "smolweb", not(target_arch = "wasm32")))]
#[path = "remote_client_limits_tests.rs"]
mod client_limits_tests;

#[cfg(all(test, feature = "smolweb"))]
#[path = "remote/smolweb_tests.rs"]
mod smolweb_tests;
