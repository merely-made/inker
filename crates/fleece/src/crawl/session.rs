// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Destination binding for a gathering run. Gathering produces content of a
//! caller-owned type `T`; the session tags each run's output with a caller-owned
//! destination `D`. A host binds its own pair (Mere's was a graph id and a graph
//! contribution) and decides when and with what authority to apply them.

use super::*;

pub struct CrawlSession<D, T> {
    handle: ActorHandle<CrawlCommand>,
    rx: Receiver<CrawlEvent<T>>,
    cancel: Arc<AtomicBool>,
    run_id: u64,
    destination: Option<D>,
    progress: CrawlProgress,
    default_policy: CrawlPolicy,
}

impl<D: Copy, T: Send + 'static> CrawlSession<D, T> {
    pub fn new(wake: Wake, fetch: CrawlFetch, process_page: CrawlProcess<T>) -> Self {
        let (handle, rx) = spawn_crawl(wake, fetch, process_page);
        Self {
            handle,
            rx,
            cancel: Arc::new(AtomicBool::new(false)),
            run_id: 0,
            destination: None,
            progress: CrawlProgress::default(),
            default_policy: CrawlPolicy::default(),
        }
    }

    pub fn policy(&self) -> CrawlPolicy {
        self.default_policy
    }
    pub fn set_policy(&mut self, policy: CrawlPolicy) {
        self.default_policy = policy;
    }
    pub fn scope(&self) -> HostScope {
        self.default_policy.scope
    }
    pub fn max_depth(&self) -> u32 {
        self.default_policy.max_depth
    }
    pub fn set_scope(&mut self, scope: HostScope) {
        self.default_policy.scope = scope;
    }
    pub fn set_max_depth(&mut self, depth: u32) {
        self.default_policy.max_depth = depth;
    }
    pub fn seed_sitemap(&self) -> bool {
        self.default_policy.seed_sitemap
    }
    pub fn set_seed_sitemap(&mut self, on: bool) {
        self.default_policy.seed_sitemap = on;
    }
    pub fn max_pages(&self) -> usize {
        self.default_policy.max_pages
    }
    pub fn set_max_pages(&mut self, pages: usize) {
        self.default_policy.max_pages = pages;
    }

    pub fn start(&mut self, seed: &str, policy: CrawlPolicy, destination: D) {
        self.stop();
        self.cancel = Arc::new(AtomicBool::new(false));
        self.run_id = self.run_id.wrapping_add(1);
        self.destination = Some(destination);
        self.progress = CrawlProgress {
            running: true,
            ..CrawlProgress::default()
        };
        self.handle.command(CrawlCommand::Start {
            run_id: self.run_id,
            seed: seed.to_string(),
            policy,
            cancel: self.cancel.clone(),
        });
    }

    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn drain(&mut self) -> Vec<(D, T)> {
        let mut applied = Vec::new();
        while let Ok(event) = self.rx.try_recv() {
            fold_event(
                event,
                self.run_id,
                self.destination,
                &mut self.progress,
                &mut applied,
            );
        }
        applied
    }

    pub fn progress(&self) -> &CrawlProgress {
        &self.progress
    }
}

impl<D, T> Drop for CrawlSession<D, T> {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Ignore superseded runs before looking at the destination or progress.
pub fn fold_event<D: Copy, T>(
    event: CrawlEvent<T>,
    active_run: u64,
    destination: Option<D>,
    progress: &mut CrawlProgress,
    applied: &mut Vec<(D, T)>,
) {
    if event.run_id == active_run {
        fold_update(event.update, destination, progress, applied);
    }
}

pub fn fold_update<D: Copy, T>(
    update: CrawlUpdate<T>,
    destination: Option<D>,
    progress: &mut CrawlProgress,
    applied: &mut Vec<(D, T)>,
) {
    match update {
        CrawlUpdate::Content { content } => {
            if let Some(destination) = destination {
                applied.extend(content.into_iter().map(|item| (destination, item)));
            }
        },
        CrawlUpdate::Progress { fetched, last_url } => {
            progress.fetched = fetched;
            progress.last_url = Some(last_url);
            progress.running = true;
        },
        CrawlUpdate::Done { summary } => {
            progress.fetched = summary.fetched;
            progress.running = false;
            progress.summary = Some(summary);
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contributions_bind_to_supplied_destination_and_old_run_is_ignored() {
        let destination = 7_u32;
        let mut progress = CrawlProgress::default();
        let mut applied = Vec::new();
        fold_event(
            CrawlEvent {
                run_id: 1,
                update: CrawlUpdate::Content {
                    content: vec!["contribution"],
                },
            },
            2,
            Some(destination),
            &mut progress,
            &mut applied,
        );
        assert!(applied.is_empty());
        fold_event(
            CrawlEvent {
                run_id: 2,
                update: CrawlUpdate::Content {
                    content: vec!["contribution"],
                },
            },
            2,
            Some(destination),
            &mut progress,
            &mut applied,
        );
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].0, destination);
        fold_event(
            CrawlEvent {
                run_id: 1,
                update: CrawlUpdate::Done {
                    summary: CrawlSummary::default(),
                },
            },
            2,
            Some(destination),
            &mut progress,
            &mut applied,
        );
        assert!(progress.summary.is_none());
        fold_update(
            CrawlUpdate::Done {
                summary: CrawlSummary {
                    fetched: 3,
                    stop: CrawlStop::RequestBudget,
                    ..CrawlSummary::default()
                },
            },
            Some(destination),
            &mut progress,
            &mut applied,
        );
        assert_eq!(progress.fetched, 3);
        assert!(!progress.running);
        assert_eq!(progress.summary.unwrap().stop, CrawlStop::RequestBudget);
    }
}
