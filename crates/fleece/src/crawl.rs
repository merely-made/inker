// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! An explicitly started HTTP source adapter for Fleece gathering.
//!
//! General engine/graph traversal lives in [`crate::traversal`]. This module
//! adds web-specific admission, robots and politeness to a supplied fetch seam.
//! It never observes ordinary browsing or starts itself on navigation.

use std::collections::{HashMap, HashSet, VecDeque};
use std::future::{Future, poll_fn};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::task::Poll;
use std::time::{Duration, Instant};

use armillary::{ActorHandle, Emitter, Wake, spawn};
use fetch::Fetched;
use tokio::runtime::Builder;

mod frontier;
pub use frontier::*;
use frontier::{host_of, is_http, origin_of, path_of, robots_url, sitemap_url};
mod robots;
pub use robots::CRAWLER_UA;
use robots::RobotsRules;
#[cfg(feature = "crawl-graph")]
pub mod graph;
mod sitemap;

/// Harvest links from an already supplied HTML response. Extraction returns raw
/// links; this adapter resolves them and the frontier applies admission policy.
/// Other content types are left to their own source adapters.
pub fn enqueue_fetched_html_links(
    frontier: &mut Frontier,
    page_url: &str,
    depth: u32,
    fetched: &Fetched,
) -> usize {
    let Some(syntax) = html_syntax(fetched.content_type.as_deref()) else {
        return 0;
    };
    let Ok(base) = url::Url::parse(page_url) else {
        return 0;
    };
    let document = match syntax {
        HtmlSyntax::Html => genet_static_dom::StaticDocument::parse(&fetched.body),
        HtmlSyntax::Xhtml => genet_static_dom::StaticDocument::parse_xml(&fetched.body),
    };
    let links = crate::extract_links(&document)
        .into_iter()
        .filter_map(|link| base.join(&link.href).ok().map(|url| url.to_string()))
        .collect::<Vec<_>>();
    frontier.enqueue(&links, depth)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HtmlSyntax {
    Html,
    Xhtml,
}

fn html_syntax(content_type: Option<&str>) -> Option<HtmlSyntax> {
    let media_type = content_type?.split(';').next()?.trim();
    if media_type.eq_ignore_ascii_case("text/html") {
        Some(HtmlSyntax::Html)
    } else if media_type.eq_ignore_ascii_case("application/xhtml+xml") {
        Some(HtmlSyntax::Xhtml)
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrawlRequestKind {
    Robots,
    Sitemap,
    Page,
}

/// Exactly one transport attempt. The adapter must enforce `max_bytes` while
/// reading, identify with [`CRAWLER_UA`], and perform no hidden redirects/retries.
/// Requests for a redirect target require fresh admission and budget accounting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrawlRequest {
    pub url: String,
    pub max_bytes: usize,
    pub kind: CrawlRequestKind,
}

/// Raw bytes already received on a failed operation still consume the byte
/// budget. An absent robots file is distinguished from an unreachable service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CrawlFetchError {
    NotFound {
        received_bytes: usize,
    },
    Failed {
        message: String,
        received_bytes: usize,
    },
    ResponseLimit {
        received_bytes: usize,
    },
}

impl CrawlFetchError {
    pub fn received_bytes(&self) -> usize {
        match self {
            Self::NotFound { received_bytes }
            | Self::Failed { received_bytes, .. }
            | Self::ResponseLimit { received_bytes } => *received_bytes,
        }
    }
}

pub type CrawlFetchFuture = Pin<Box<dyn Future<Output = Result<Fetched, CrawlFetchError>> + Send>>;
pub type CrawlFetch = Arc<dyn Fn(CrawlRequest) -> CrawlFetchFuture + Send + Sync>;

/// Mere's concrete single-hop HTTP adapter for the supplied request seam. It
/// reads within the response cap without ambient browser cookies, redirects or
/// retries. Hosts with their own transports can supply their own adapter.
pub async fn fetch_crawler_request(request: CrawlRequest) -> Result<Fetched, CrawlFetchError> {
    let result = fetch::crawler::fetch_crawler_capped(&request.url, request.max_bytes).await;
    lower_crawler_result(result)
}

fn lower_crawler_result(
    result: Result<fetch::crawler::CrawlerResponse, fetch::crawler::CrawlerFetchError>,
) -> Result<Fetched, CrawlFetchError> {
    let response = result.map_err(|error| match &error {
        fetch::crawler::CrawlerFetchError::ResponseLimit { received_bytes, .. } => {
            CrawlFetchError::ResponseLimit {
                received_bytes: *received_bytes,
            }
        },
        _ => CrawlFetchError::Failed {
            received_bytes: error.received_bytes(),
            message: error.to_string(),
        },
    })?;
    match response.status {
        200..=299 => Ok(Fetched {
            content_type: response.content_type,
            content_disposition: response.content_disposition,
            body: String::from_utf8_lossy(&response.bytes).into_owned(),
            bytes: response.bytes,
        }),
        404 | 410 => Err(CrawlFetchError::NotFound {
            received_bytes: response.bytes.len(),
        }),
        status => Err(CrawlFetchError::Failed {
            message: format!("HTTP {status}"),
            received_bytes: response.bytes.len(),
        }),
    }
}

/// A supplied response processor can emit any caller-owned gathered-content
/// type. Graph IDs and graph mutation authority live in the optional adapter.
pub type CrawlProcess<T> = Arc<dyn Fn(&mut Frontier, &str, u32, &Fetched) -> Vec<T> + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CrawlStop {
    FrontierExhausted,
    PageBudget,
    RequestBudget,
    TotalByteBudget,
    ResponseByteLimit {
        url: String,
        cap: usize,
        received: usize,
    },
    Cancelled,
    RequestTimeout {
        url: String,
    },
    InvalidSeed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrawlSummary {
    pub fetched: usize,
    pub examined: usize,
    pub requests: usize,
    pub received_bytes: usize,
    /// False when an in-flight operation was dropped before returning byte
    /// evidence. In that case `received_bytes` is a known lower bound.
    pub received_bytes_complete: bool,
    pub failed_requests: usize,
    pub robots_skipped: usize,
    pub stop: CrawlStop,
}

impl Default for CrawlSummary {
    fn default() -> Self {
        Self {
            fetched: 0,
            examined: 0,
            requests: 0,
            received_bytes: 0,
            received_bytes_complete: true,
            failed_requests: 0,
            robots_skipped: 0,
            stop: CrawlStop::FrontierExhausted,
        }
    }
}

pub enum CrawlUpdate<T> {
    Content { content: Vec<T> },
    Progress { fetched: usize, last_url: String },
    Done { summary: CrawlSummary },
}

/// Each command has its own cancellation token and receipt correlation. Starting
/// a replacement crawl can stop the old one without misrouting its late output.
pub enum CrawlCommand {
    Start {
        run_id: u64,
        seed: String,
        policy: CrawlPolicy,
        cancel: Arc<AtomicBool>,
    },
}

pub struct CrawlEvent<T> {
    pub run_id: u64,
    pub update: CrawlUpdate<T>,
}

pub fn spawn_crawl<T: Send + 'static>(
    wake: Wake,
    fetch: CrawlFetch,
    process_page: CrawlProcess<T>,
) -> (ActorHandle<CrawlCommand>, Receiver<CrawlEvent<T>>) {
    spawn(wake, move |commands, out: Emitter<CrawlEvent<T>>| {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build the crawl runtime");
        while let Ok(command) = commands.recv() {
            match command {
                CrawlCommand::Start {
                    run_id,
                    seed,
                    policy,
                    cancel,
                } => {
                    runtime.block_on(run_crawl(
                        &seed,
                        policy,
                        |request| (fetch)(request),
                        || cancel.load(Ordering::Relaxed),
                        |frontier, url, depth, fetched| {
                            (process_page)(frontier, url, depth, fetched)
                        },
                        |update| out.emit(CrawlEvent { run_id, update }),
                    ));
                },
            }
        }
    })
}

/// Drive one finite web acquisition. Robots, optional sitemap, page failures and
/// their response bytes all consume the same limits. A robots transport failure
/// denies that origin; an explicitly absent file permits it. The adapter returns
/// raw bytes as the accounting authority, independently of decoded text.
pub async fn run_crawl<F, Fut, P, T>(
    seed: &str,
    policy: CrawlPolicy,
    mut fetch: F,
    mut cancelled: impl FnMut() -> bool,
    mut process_page: P,
    mut emit: impl FnMut(CrawlUpdate<T>),
) -> CrawlSummary
where
    F: FnMut(CrawlRequest) -> Fut,
    Fut: Future<Output = Result<Fetched, CrawlFetchError>>,
    P: FnMut(&mut Frontier, &str, u32, &Fetched) -> Vec<T>,
{
    let mut summary = CrawlSummary::default();
    if !is_http(seed) {
        summary.stop = CrawlStop::InvalidSeed;
        emit(CrawlUpdate::Done {
            summary: summary.clone(),
        });
        return summary;
    }
    let mut frontier = Frontier::new(seed, policy);
    let mut last_host_fetch = HashMap::new();
    // None records a failed, unreachable robots file and denies that origin.
    let mut robots: HashMap<String, Option<RobotsRules>> = HashMap::new();
    let mut sitemap_checked = false;

    'crawl: while let Some((url, depth)) = frontier.next() {
        summary.examined = frontier.fetched();
        if cancelled() {
            summary.stop = CrawlStop::Cancelled;
            break;
        }
        let origin = origin_of(&url).unwrap_or_default();
        if !robots.contains_key(&origin) {
            let result = bounded_request(
                robots_url(&url).expect("valid HTTP origin"),
                CrawlRequestKind::Robots,
                policy,
                &mut summary,
                &mut last_host_fetch,
                &mut fetch,
                &mut cancelled,
            )
            .await;
            let rules = match result {
                Ok(Ok(response)) => Some(RobotsRules::parse(&response.body)),
                Ok(Err(CrawlFetchError::NotFound { .. })) => Some(RobotsRules::parse("")),
                Ok(Err(_)) => None,
                Err(stop) => {
                    summary.stop = stop;
                    break;
                },
            };
            robots.insert(origin.clone(), rules);
        }
        let rules = &robots[&origin];
        // Sitemap acquisition obeys the same origin policy and request limits.
        if policy.seed_sitemap && !sitemap_checked {
            sitemap_checked = true;
            if let Some(sitemap) = sitemap_url(seed) {
                if rules
                    .as_ref()
                    .is_some_and(|rules| rules.allows(&path_of(&sitemap)))
                {
                    match bounded_request(
                        sitemap,
                        CrawlRequestKind::Sitemap,
                        policy,
                        &mut summary,
                        &mut last_host_fetch,
                        &mut fetch,
                        &mut cancelled,
                    )
                    .await
                    {
                        Ok(Ok(response)) => {
                            frontier.enqueue_seeds(&sitemap::parse_sitemap(&response.body));
                        },
                        Ok(Err(_)) => {},
                        Err(stop) => {
                            summary.stop = stop;
                            break 'crawl;
                        },
                    }
                }
            }
        }
        if !rules
            .as_ref()
            .is_some_and(|rules| rules.allows(&path_of(&url)))
        {
            summary.robots_skipped += 1;
            continue;
        }
        match bounded_request(
            url.clone(),
            CrawlRequestKind::Page,
            policy,
            &mut summary,
            &mut last_host_fetch,
            &mut fetch,
            &mut cancelled,
        )
        .await
        {
            Ok(Ok(response)) => {
                if cancelled() {
                    summary.stop = CrawlStop::Cancelled;
                    break;
                }
                summary.fetched += 1;
                let content = process_page(&mut frontier, &url, depth, &response);
                if !content.is_empty() {
                    emit(CrawlUpdate::Content { content });
                }
                emit(CrawlUpdate::Progress {
                    fetched: summary.fetched,
                    last_url: url,
                });
            },
            Ok(Err(error)) => tracing::warn!(%url, ?error, "crawl request failed; skipping"),
            Err(stop) => {
                summary.stop = stop;
                break;
            },
        }
    }
    if summary.stop == CrawlStop::FrontierExhausted && frontier.fetched() >= policy.max_pages {
        summary.stop = CrawlStop::PageBudget;
    }
    emit(CrawlUpdate::Done {
        summary: summary.clone(),
    });
    summary
}

async fn bounded_request<F, Fut>(
    url: String,
    kind: CrawlRequestKind,
    policy: CrawlPolicy,
    summary: &mut CrawlSummary,
    last: &mut HashMap<String, Instant>,
    fetch: &mut F,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<Result<Fetched, CrawlFetchError>, CrawlStop>
where
    F: FnMut(CrawlRequest) -> Fut,
    Fut: Future<Output = Result<Fetched, CrawlFetchError>>,
{
    if cancelled() {
        return Err(CrawlStop::Cancelled);
    }
    if summary.requests >= policy.max_requests {
        return Err(CrawlStop::RequestBudget);
    }
    let remaining = policy
        .max_total_bytes
        .saturating_sub(summary.received_bytes);
    if remaining == 0 {
        return Err(CrawlStop::TotalByteBudget);
    }
    let cap = policy.max_response_bytes.min(remaining);
    if cap == 0 {
        return Err(CrawlStop::ResponseByteLimit {
            url,
            cap,
            received: 0,
        });
    }
    polite_wait(&url, policy.polite_delay, last, cancelled).await?;
    if cancelled() {
        return Err(CrawlStop::Cancelled);
    }
    summary.requests += 1;
    let request = CrawlRequest {
        url: url.clone(),
        max_bytes: cap,
        kind,
    };
    let future = fetch(request);
    let future = std::pin::pin!(future);
    let result = match await_request(future, policy.request_timeout, cancelled).await {
        Ok(result) => result,
        Err(stop) => {
            summary.received_bytes_complete = false;
            return Err(match stop {
                AwaitStop::Cancelled => CrawlStop::Cancelled,
                AwaitStop::Timeout => CrawlStop::RequestTimeout { url },
            });
        },
    };
    let received = match &result {
        Ok(response) => response.bytes.len(),
        Err(error) => {
            summary.failed_requests += 1;
            error.received_bytes()
        },
    };
    summary.received_bytes = summary.received_bytes.saturating_add(received);
    if received > cap || matches!(&result, Err(CrawlFetchError::ResponseLimit { .. })) {
        return Err(if cap < policy.max_response_bytes {
            CrawlStop::TotalByteBudget
        } else {
            CrawlStop::ResponseByteLimit { url, cap, received }
        });
    }
    // A completed operation may itself observe cancellation, including a ready
    // failed response at the end of the frontier. Its bytes remain accounted and
    // complete; cancellation still owns the terminal stop reason.
    if cancelled() {
        return Err(CrawlStop::Cancelled);
    }
    Ok(result)
}

enum AwaitStop {
    Cancelled,
    Timeout,
}

/// Poll cancellation during I/O without requiring the adapter's future to know
/// about a Fleece actor. Dropping that future is the adapter's abort boundary.
async fn await_request<Fut: Future>(
    mut future: Pin<&mut Fut>,
    timeout: Duration,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<Fut::Output, AwaitStop> {
    let started = Instant::now();
    let timer = tokio::time::sleep(Duration::from_millis(25));
    let mut timer = std::pin::pin!(timer);
    poll_fn(|cx| {
        if cancelled() {
            return Poll::Ready(Err(AwaitStop::Cancelled));
        }
        if started.elapsed() >= timeout {
            return Poll::Ready(Err(AwaitStop::Timeout));
        }
        if let Poll::Ready(value) = future.as_mut().poll(cx) {
            return Poll::Ready(Ok(value));
        }
        if timer.as_mut().poll(cx).is_ready() {
            timer
                .as_mut()
                .reset(tokio::time::Instant::now() + Duration::from_millis(25));
            cx.waker().wake_by_ref();
        }
        Poll::Pending
    })
    .await
}

async fn polite_wait(
    url: &str,
    delay: Duration,
    last: &mut HashMap<String, Instant>,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<(), CrawlStop> {
    let host = host_of(url).unwrap_or_default();
    if let Some(previous) = last.get(&host) {
        while previous.elapsed() < delay {
            if cancelled() {
                return Err(CrawlStop::Cancelled);
            }
            tokio::time::sleep(
                delay
                    .saturating_sub(previous.elapsed())
                    .min(Duration::from_millis(25)),
            )
            .await;
        }
    }
    last.insert(host, Instant::now());
    Ok(())
}

/// Host-visible progress shared by generic gathering and optional graph binding.
#[derive(Clone, Debug, Default)]
pub struct CrawlProgress {
    pub fetched: usize,
    pub last_url: Option<String>,
    pub running: bool,
    pub summary: Option<CrawlSummary>,
}

#[cfg(test)]
mod tests;
