// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

//! Optional graph destination binding. Gathering produces contributions; the
//! owning session decides when and with what authority to apply them.

use super::*;
use incipit::GraphId;
use linked_data::GraphContribution;

pub type CrawlProcess = super::CrawlProcess<GraphContribution>;

pub struct CrawlSession {
    handle: ActorHandle<CrawlCommand>,
    rx: Receiver<CrawlEvent<GraphContribution>>,
    cancel: Arc<AtomicBool>,
    run_id: u64,
    graph_id: Option<GraphId>,
    progress: CrawlProgress,
    default_policy: CrawlPolicy,
}

impl CrawlSession {
    pub fn new(wake: Wake, fetch: CrawlFetch, process_page: CrawlProcess) -> Self {
        let (handle, rx) = spawn_crawl(wake, fetch, process_page);
        Self {
            handle,
            rx,
            cancel: Arc::new(AtomicBool::new(false)),
            run_id: 0,
            graph_id: None,
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

    pub fn start(&mut self, seed: &str, policy: CrawlPolicy, graph_id: GraphId) {
        self.stop();
        self.cancel = Arc::new(AtomicBool::new(false));
        self.run_id = self.run_id.wrapping_add(1);
        self.graph_id = Some(graph_id);
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

    pub fn drain(&mut self) -> Vec<(GraphId, GraphContribution)> {
        let mut applied = Vec::new();
        while let Ok(event) = self.rx.try_recv() {
            fold_event(
                event,
                self.run_id,
                self.graph_id,
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

impl Drop for CrawlSession {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Ignore superseded runs before looking at graph destination or progress.
pub fn fold_event(
    event: CrawlEvent<GraphContribution>,
    active_run: u64,
    graph_id: Option<GraphId>,
    progress: &mut CrawlProgress,
    applied: &mut Vec<(GraphId, GraphContribution)>,
) {
    if event.run_id == active_run {
        fold_update(event.update, graph_id, progress, applied);
    }
}

pub fn fold_update(
    update: CrawlUpdate<GraphContribution>,
    graph_id: Option<GraphId>,
    progress: &mut CrawlProgress,
    applied: &mut Vec<(GraphId, GraphContribution)>,
) {
    match update {
        CrawlUpdate::Content { content } => {
            if let Some(graph_id) = graph_id {
                applied.extend(
                    content
                        .into_iter()
                        .map(|contribution| (graph_id, contribution)),
                );
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
        let graph_id = GraphId::nil();
        let mut progress = CrawlProgress::default();
        let mut applied = Vec::new();
        fold_event(
            CrawlEvent {
                run_id: 1,
                update: CrawlUpdate::Content {
                    content: vec![GraphContribution::default()],
                },
            },
            2,
            Some(graph_id),
            &mut progress,
            &mut applied,
        );
        assert!(applied.is_empty());
        fold_event(
            CrawlEvent {
                run_id: 2,
                update: CrawlUpdate::Content {
                    content: vec![GraphContribution::default()],
                },
            },
            2,
            Some(graph_id),
            &mut progress,
            &mut applied,
        );
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].0, graph_id);
        fold_event(
            CrawlEvent {
                run_id: 1,
                update: CrawlUpdate::Done {
                    summary: CrawlSummary::default(),
                },
            },
            2,
            Some(graph_id),
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
            Some(graph_id),
            &mut progress,
            &mut applied,
        );
        assert_eq!(progress.fetched, 3);
        assert!(!progress.running);
        assert_eq!(progress.summary.unwrap().stop, CrawlStop::RequestBudget);
    }
}
