// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Crawl host-scope policy + the visit frontier (dedup, in-scope, robots/sitemap URLs).

use super::*;

/// Which hosts a crawl may follow links into. `SameHost` is the default
/// ([`CrawlPolicy::default`]); the settings lane's scope picker selects between all
/// three (see [`Frontier::in_scope`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostScope {
    /// Only the seed's exact host (`docs.example.com` stays on `docs.example.com`).
    SameHost,
    /// The seed's host and its subdomains (`example.com` admits
    /// `docs.example.com`). This conservative suffix test does not infer sibling
    /// hosts through a Public Suffix List.
    SameDomain,
    /// Any host. The open web; only sane with a tight depth/page cap and politeness.
    AnyHost,
}

impl HostScope {
    /// The stable key for this scope, used in settings-action ids and on disk.
    pub fn as_key(self) -> &'static str {
        match self {
            HostScope::SameHost => "same_host",
            HostScope::SameDomain => "same_domain",
            HostScope::AnyHost => "any_host",
        }
    }

    /// Parse a [`as_key`](Self::as_key) string back into a scope; `None` if unknown.
    pub fn from_key(key: &str) -> Option<HostScope> {
        match key {
            "same_host" => Some(HostScope::SameHost),
            "same_domain" => Some(HostScope::SameDomain),
            "any_host" => Some(HostScope::AnyHost),
            _ => None,
        }
    }

    /// A short human label for the settings picker.
    pub fn label(self) -> &'static str {
        match self {
            HostScope::SameHost => "Same host",
            HostScope::SameDomain => "Same domain",
            HostScope::AnyHost => "Any host",
        }
    }
}

/// How far and wide a crawl may roam — the bound the [`Frontier`] enforces.
#[derive(Clone, Copy, Debug)]
pub struct CrawlPolicy {
    /// Max link-hops from the seed: `0` fetches the seed only, `1` the seed and its
    /// direct links, and so on.
    pub max_depth: u32,
    /// Hard cap on total pages fetched — the runaway backstop.
    pub max_pages: usize,
    /// Max links enqueued from a single page (fan-out cap), so one hub page cannot
    /// flood the frontier.
    pub max_fanout: usize,
    /// Which hosts links may lead into.
    pub scope: HostScope,
    /// Seed the crawl from the site's `sitemap.xml` (its canonical page list), not
    /// only the seed page's links — for crawling a site comprehensively. Off by
    /// default (a focused crawl stays the seed's neighborhood); a "crawl whole site"
    /// mode turns it on. Still bounded by `max_pages`.
    pub seed_sitemap: bool,
    /// All transport attempts, including robots, sitemap and failed requests.
    pub max_requests: usize,
    /// Maximum raw response bytes supplied to a request adapter.
    pub max_response_bytes: usize,
    /// Maximum raw response bytes across the whole explicit crawl.
    pub max_total_bytes: usize,
    /// Minimum interval between every request to the same host.
    pub polite_delay: Duration,
    /// Deadline for one adapter operation. The runtime drops its future on expiry.
    pub request_timeout: Duration,
}

impl Default for CrawlPolicy {
    /// Conservative: shallow, bounded, same-host, no sitemap. Deliberately small so an
    /// accidental crawl is cheap and polite by default.
    fn default() -> Self {
        Self {
            max_depth: 1,
            max_pages: 25,
            max_fanout: 20,
            scope: HostScope::SameHost,
            seed_sitemap: false,
            max_requests: 32,
            max_response_bytes: 256 * 1024,
            max_total_bytes: 2 * 1024 * 1024,
            polite_delay: Duration::from_secs(1),
            request_timeout: Duration::from_secs(30),
        }
    }
}

/// The crawl frontier: the ordered set of URLs still to fetch, with dedup and the
/// [`CrawlPolicy`] bounds. Pure — no I/O; the crawl actor supplies fetched links and
/// asks what to fetch next. Breadth-first (a `VecDeque`), so a depth cap yields a
/// tidy expanding neighborhood rather than a deep tunnel.
pub struct Frontier {
    policy: CrawlPolicy,
    seed_host: String,
    queue: VecDeque<(String, u32)>,
    /// Every URL ever enqueued (normalized), so a target is fetched at most once even
    /// when many pages link to it.
    seen: HashSet<String>,
    fetched: usize,
}

impl Frontier {
    /// Seed a crawl at `seed_url`, enqueued at depth 0. The seed's host scopes the
    /// `SameHost` / `SameDomain` policies.
    pub fn new(seed_url: &str, policy: CrawlPolicy) -> Self {
        let seed = normalize(seed_url);
        let seed_host = host_of(&seed).unwrap_or_default();
        let mut seen = HashSet::new();
        let mut queue = VecDeque::new();
        if is_http(&seed) && policy.max_pages > 0 {
            seen.insert(seed.clone());
            queue.push_back((seed, 0));
        }
        Self {
            policy,
            seed_host,
            queue,
            seen,
            fetched: 0,
        }
    }

    /// The next `(url, depth)` to fetch, or `None` when the frontier is empty or the
    /// page cap is reached. Each call counts toward [`CrawlPolicy::max_pages`].
    pub fn next(&mut self) -> Option<(String, u32)> {
        if self.fetched >= self.policy.max_pages {
            return None;
        }
        let item = self.queue.pop_front()?;
        self.fetched += 1;
        Some(item)
    }

    /// Enqueue the (already-resolved, absolute) outbound `links` found on a page at
    /// `depth`. Applies the depth cap (children at `depth + 1`), the per-page fan-out
    /// cap, the host scope, and visited-dedup; non-http(s) URLs are dropped. Returns
    /// how many were newly enqueued.
    pub fn enqueue(&mut self, links: &[String], depth: u32) -> usize {
        let Some(child_depth) = depth.checked_add(1) else {
            return 0;
        };
        if child_depth > self.policy.max_depth {
            return 0;
        }
        let mut added = 0;
        for link in links {
            if added >= self.policy.max_fanout || self.seen.len() >= self.policy.max_pages {
                break;
            }
            let url = normalize(link);
            if !is_http(&url) || !self.in_scope(&url) || self.seen.contains(&url) {
                continue;
            }
            self.seen.insert(url.clone());
            self.queue.push_back((url, child_depth));
            added += 1;
        }
        added
    }

    /// Enqueue bulk seed URLs (e.g. a site's `sitemap.xml`), at depth 1, with host
    /// scope + visited-dedup and the same depth, fanout and page bounds as links.
    /// A sitemap does not authorize broader acquisition than a page does.
    pub fn enqueue_seeds(&mut self, urls: &[String]) -> usize {
        self.enqueue(urls, 0)
    }

    /// Pages handed out by [`next`](Self::next) so far (against `max_pages`).
    pub fn fetched(&self) -> usize {
        self.fetched
    }

    /// Whether `url`'s host is within the policy scope.
    fn in_scope(&self, url: &str) -> bool {
        match self.policy.scope {
            HostScope::AnyHost => true,
            HostScope::SameHost => host_of(url).as_deref() == Some(self.seed_host.as_str()),
            HostScope::SameDomain => {
                host_of(url).is_some_and(|h| same_registrable_domain(&h, &self.seed_host))
            },
        }
    }
}

/// `url`'s host, lowercased, or `None` if it does not parse / has no host.
pub(crate) fn host_of(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()?
        .host_str()
        .map(|h| h.to_ascii_lowercase())
}

/// Robots policies belong to an origin, including scheme and effective port.
pub(crate) fn origin_of(url: &str) -> Option<String> {
    Some(url::Url::parse(url).ok()?.origin().ascii_serialization())
}

/// The `/robots.txt` URL for `page_url`'s origin, or `None` if it does not parse.
pub(crate) fn robots_url(page_url: &str) -> Option<String> {
    url::Url::parse(page_url)
        .ok()?
        .join("/robots.txt")
        .ok()
        .map(|u| u.to_string())
}

/// The `/sitemap.xml` URL for `page_url`'s origin, or `None` if it does not parse.
pub(crate) fn sitemap_url(page_url: &str) -> Option<String> {
    url::Url::parse(page_url)
        .ok()?
        .join("/sitemap.xml")
        .ok()
        .map(|u| u.to_string())
}

/// `page_url`'s path (e.g. `/foo/bar`), for matching against robots rules; `/` if it
/// does not parse.
pub(crate) fn path_of(page_url: &str) -> String {
    url::Url::parse(page_url)
        .map(|u| match u.query() {
            Some(query) => format!("{}?{query}", u.path()),
            None => u.path().to_string(),
        })
        .unwrap_or_else(|_| "/".to_string())
}

/// Normalize a URL for dedup: parse, drop the fragment, re-serialize. Falls back to
/// the trimmed input when it does not parse, so a malformed link still dedups against
/// itself.
pub(crate) fn normalize(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut u) => {
            u.set_fragment(None);
            u.to_string()
        },
        Err(_) => url.trim().to_string(),
    }
}

/// Whether `url` is http(s) — the schemes a web crawl follows.
pub(crate) fn is_http(url: &str) -> bool {
    url::Url::parse(url)
        .map(|u| {
            matches!(u.scheme(), "http" | "https")
                && u.host_str().is_some()
                && u.username().is_empty()
                && u.password().is_none()
        })
        .unwrap_or(false)
}

/// Conservative domain match: `host` equals `seed_host`, or `host` ends
/// with `.seed_host` (so `docs.example.com` matches a seed host of `example.com`).
/// It does not infer a common registrable parent from two sibling hosts.
pub(crate) fn same_registrable_domain(host: &str, seed_host: &str) -> bool {
    host == seed_host || host.ends_with(&format!(".{seed_host}"))
}
