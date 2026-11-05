// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Goal-directed traversal over sources a host already retains.
//!
//! [`run_retained_traversal`] plugs Eidetic's model-free [`GoalAssessor`] into
//! [`run_traversal`](crate::traversal::run_traversal). The host supplies a
//! [`RetainedProvider`] that maps a source (and optional version) to text it
//! already holds plus outgoing links. The provider is a read of retained
//! state: nothing here fetches, resolves a URL, opens a transport or writes.
//!
//! # Identity mapping
//!
//! A [`RecallSourceId`] `{kind, owner, source, node}` and a [`ContentRef`]
//! `{source_id, node_id, version}` convert explicitly and reversibly with
//! [`content_ref`] and [`recall_source`]:
//!
//! * `ContentRef.source_id` is the JSON array `[kind, owner]`, for example
//!   `["page_text","session-1"]` or `["clip",null]`;
//! * `ContentRef.node_id` is the JSON array `[source, node]`, for example
//!   `["https://example.test/a",null]`;
//! * `ContentRef.version` is the retained version (`None` means "whichever
//!   version the host retains", and is resolved by the provider).
//!
//! JSON quoting keeps the encoding injective whatever the strings contain.
//! A reference that is not in this form decodes to `None`.
//!
//! # Versions
//!
//! A reference naming a version is observed only if the retained text carries
//! exactly that version; any other version is refused as
//! [`RetainedFailure::VersionMismatch`]. A reference without a version accepts
//! the provider's version, which every visit then reports.
//!
//! # Budgets
//!
//! [`RetainedLimits`] adds a request budget and a byte budget to the
//! traversal policy. `TraversalStop` has no variant for either and its
//! observed type is generic over content size, so the adapter enforces both
//! through the traversal's cancellation seam and reports the cause as
//! [`RetainedStop`]. Requests count provider lookups (as `max_nodes` does);
//! bytes count UTF-8 bytes of retained body plus title that were admitted. An
//! observation that would exceed the remaining bytes is refused, recorded as
//! [`RetainedFailure::ByteBudget`], and ends the traversal.

use crate::traversal::{
    ContentObservation, ContentRef, GoalDecision, TraversalCandidate, TraversalGoal,
    TraversalPolicy, TraversalReport, TraversalStop, run_traversal,
};
use eidetic::models::recall::{RecallSourceId, RecallSourceKind, RetainedSourceText};
use eidetic_search::goal::{CandidateLabel, GoalAssessor, GoalRecord, GoalSettings, GoalVerdict};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// A link out of a retained source, as the host already holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetainedLink {
    pub source: RecallSourceId,
    pub version: Option<String>,
    /// Link text or title the host retains; used only to rank candidates.
    pub label: Option<String>,
}

/// What a host retains for one source: its text and its outgoing links.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetainedObservation {
    pub text: RetainedSourceText,
    pub links: Vec<RetainedLink>,
}

/// A read-only view of retained sources. Implementations must not fetch,
/// resolve or refresh; return `None` for anything not already retained.
pub trait RetainedProvider {
    fn retained(
        &mut self,
        source: &RecallSourceId,
        version: Option<&str>,
    ) -> Option<RetainedObservation>;
}

impl<F> RetainedProvider for F
where
    F: FnMut(&RecallSourceId, Option<&str>) -> Option<RetainedObservation>,
{
    fn retained(
        &mut self,
        source: &RecallSourceId,
        version: Option<&str>,
    ) -> Option<RetainedObservation> {
        self(source, version)
    }
}

/// A traversal start point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetainedSeed {
    pub source: RecallSourceId,
    pub version: Option<String>,
}

/// Request and byte limits, in addition to [`TraversalPolicy`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetainedLimits {
    /// Provider lookups, including ones that find nothing.
    pub max_requests: usize,
    /// UTF-8 bytes of retained body and title admitted across all visits.
    pub max_bytes: usize,
}

impl Default for RetainedLimits {
    fn default() -> Self {
        Self {
            max_requests: 50,
            max_bytes: 4 * 1024 * 1024,
        }
    }
}

/// Why a retained lookup was not observed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetainedFailure {
    /// The host retains nothing for the reference.
    Missing,
    /// The reference names one version and the host retains another.
    VersionMismatch { requested: String, retained: String },
    /// The provider returned text for a different source.
    SourceMismatch,
    /// The text exceeds the goal settings' `max_text_chars`.
    Oversized,
    /// Observing the text would exceed the byte budget.
    ByteBudget { needed: usize, remaining: usize },
}

/// Why a retained traversal ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetainedStop {
    Exhausted,
    NodeBudget,
    RequestBudget,
    ByteBudget,
    Cancelled,
    GoalSatisfied,
    SelectionStopped,
    InvalidSelection,
}

impl From<TraversalStop> for RetainedStop {
    fn from(stop: TraversalStop) -> Self {
        match stop {
            TraversalStop::Exhausted => Self::Exhausted,
            TraversalStop::NodeBudget => Self::NodeBudget,
            TraversalStop::Cancelled => Self::Cancelled,
            TraversalStop::GoalSatisfied => Self::GoalSatisfied,
            TraversalStop::SelectionStopped => Self::SelectionStopped,
            TraversalStop::InvalidSelection => Self::InvalidSelection,
        }
    }
}

/// The in-process result: the raw traversal (with retained text) and the
/// goal record. Use [`Self::receipt`] for a serializable citation.
#[derive(Clone, Debug, PartialEq)]
pub struct RetainedTraversalReport {
    pub traversal: TraversalReport<RetainedSourceText, RetainedFailure>,
    pub goal: GoalRecord,
    pub stop: RetainedStop,
    pub requests: usize,
    pub bytes: usize,
}

/// Counts of candidates left out, mirroring `TraversalOmissions`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OmissionCounts {
    pub depth: usize,
    pub fanout: usize,
    pub pending: usize,
    pub duplicates: usize,
}

/// One observed source, citing the retained version that was read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisitReceipt {
    pub source: RecallSourceId,
    pub requested_version: Option<String>,
    pub retained_version: String,
    pub text_hash: eidetic::Hash,
    pub bytes: usize,
    pub depth: u32,
    pub verdict: GoalVerdict,
}

/// One lookup that was not observed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureReceipt {
    pub source: RecallSourceId,
    pub requested_version: Option<String>,
    pub depth: u32,
    pub failure: RetainedFailure,
}

/// The serializable account of a run. It carries no retained text: source
/// versions and hashes identify what was read, and the [`GoalRecord`] holds
/// the reasons behind every selection and assessment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RetainedTraversalReceipt {
    pub stop: RetainedStop,
    pub requests: usize,
    pub bytes: usize,
    pub visits: Vec<VisitReceipt>,
    pub failures: Vec<FailureReceipt>,
    pub omissions: OmissionCounts,
    pub goal: GoalRecord,
}

impl RetainedTraversalReport {
    pub fn receipt(&self) -> RetainedTraversalReceipt {
        let source_of = |reference: &ContentRef| {
            recall_source(reference).unwrap_or_else(|| undecodable(reference))
        };
        let omissions = self.traversal.omissions;
        RetainedTraversalReceipt {
            stop: self.stop,
            requests: self.requests,
            bytes: self.bytes,
            visits: self
                .traversal
                .visits
                .iter()
                .map(|visit| {
                    let text = &visit.observation.content;
                    VisitReceipt {
                        source: source_of(&visit.candidate.reference),
                        requested_version: visit.candidate.reference.version.clone(),
                        retained_version: text.version.clone(),
                        text_hash: eidetic::Hash::of(text.text.as_bytes()),
                        bytes: retained_bytes(text),
                        depth: visit.candidate.depth,
                        verdict: verdict_of(visit.goal),
                    }
                })
                .collect(),
            failures: self
                .traversal
                .failures
                .iter()
                .map(|failure| FailureReceipt {
                    source: source_of(&failure.candidate.reference),
                    requested_version: failure.candidate.reference.version.clone(),
                    depth: failure.candidate.depth,
                    failure: failure.error.clone(),
                })
                .collect(),
            omissions: OmissionCounts {
                depth: omissions.depth,
                fanout: omissions.fanout,
                pending: omissions.pending,
                duplicates: omissions.duplicates,
            },
            goal: self.goal.clone(),
        }
    }
}

/// Encode a source and optional version as a traversal reference.
pub fn content_ref(source: &RecallSourceId, version: Option<&str>) -> ContentRef {
    let kind = serde_json::to_value(&source.kind).unwrap_or(serde_json::Value::Null);
    ContentRef {
        source_id: serde_json::json!([kind, source.owner]).to_string(),
        node_id: serde_json::json!([source.source, source.node]).to_string(),
        version: version.map(str::to_owned),
    }
}

/// Decode a reference made by [`content_ref`]; `None` for any other form.
pub fn recall_source(reference: &ContentRef) -> Option<RecallSourceId> {
    let (kind, owner): (RecallSourceKind, Option<String>) =
        serde_json::from_str(&reference.source_id).ok()?;
    let (source, node): (String, Option<String>) = serde_json::from_str(&reference.node_id).ok()?;
    Some(RecallSourceId {
        kind,
        owner,
        source,
        node,
    })
}

fn undecodable(reference: &ContentRef) -> RecallSourceId {
    RecallSourceId {
        kind: RecallSourceKind::Fleece,
        owner: Some(reference.source_id.clone()),
        source: reference.node_id.clone(),
        node: None,
    }
}

fn retained_bytes(text: &RetainedSourceText) -> usize {
    text.text.len() + text.title.as_ref().map_or(0, String::len)
}

fn verdict_of(decision: GoalDecision) -> GoalVerdict {
    match decision {
        GoalDecision::Continue => GoalVerdict::Continue,
        GoalDecision::Match => GoalVerdict::Match,
        GoalDecision::Satisfied => GoalVerdict::Satisfied,
    }
}

fn decision_of(verdict: GoalVerdict) -> GoalDecision {
    match verdict {
        GoalVerdict::Continue => GoalDecision::Continue,
        GoalVerdict::Match => GoalDecision::Match,
        GoalVerdict::Satisfied => GoalDecision::Satisfied,
    }
}

/// Mutable state of one run, shared by the traversal's four seams.
struct Run<'a, P> {
    provider: RefCell<&'a mut P>,
    assessor: RefCell<GoalAssessor>,
    labels: RefCell<HashMap<ContentRef, String>>,
    limits: RetainedLimits,
    max_text_chars: usize,
    requests: Cell<usize>,
    bytes: Cell<usize>,
    just_observed: Cell<bool>,
    refused_for_bytes: Cell<bool>,
    cause: Cell<Option<RetainedStop>>,
}

impl<P: RetainedProvider> Run<'_, P> {
    fn observe(
        &self,
        reference: &ContentRef,
    ) -> Result<ContentObservation<RetainedSourceText>, RetainedFailure> {
        self.requests.set(self.requests.get() + 1);
        self.just_observed.set(true);
        let source = recall_source(reference).ok_or(RetainedFailure::Missing)?;
        let found = self
            .provider
            .borrow_mut()
            .retained(&source, reference.version.as_deref())
            .ok_or(RetainedFailure::Missing)?;
        let text = found.text;
        if text.source != source {
            return Err(RetainedFailure::SourceMismatch);
        }
        if let Some(requested) = &reference.version
            && *requested != text.version
        {
            return Err(RetainedFailure::VersionMismatch {
                requested: requested.clone(),
                retained: text.version.clone(),
            });
        }
        let chars =
            text.text.chars().count() + text.title.as_ref().map_or(0, |t| t.chars().count());
        if chars > self.max_text_chars {
            return Err(RetainedFailure::Oversized);
        }
        let needed = retained_bytes(&text);
        let remaining = self.limits.max_bytes.saturating_sub(self.bytes.get());
        if needed > remaining {
            self.refused_for_bytes.set(true);
            return Err(RetainedFailure::ByteBudget { needed, remaining });
        }
        self.bytes.set(self.bytes.get() + needed);
        let mut links = Vec::with_capacity(found.links.len());
        for link in found.links {
            let target = content_ref(&link.source, link.version.as_deref());
            if let Some(label) = link.label {
                self.labels
                    .borrow_mut()
                    .entry(target.clone())
                    .or_insert(label);
            }
            links.push(target);
        }
        Ok(ContentObservation {
            observation_id: format!("retained:{}", eidetic::Hash::of(text.text.as_bytes())),
            content: text,
            links,
        })
    }

    fn select(&self, candidates: &[TraversalCandidate]) -> Option<usize> {
        let labels = self.labels.borrow();
        let labelled: Vec<CandidateLabel> = candidates
            .iter()
            .map(|candidate| CandidateLabel {
                source: recall_source(&candidate.reference)
                    .unwrap_or_else(|| undecodable(&candidate.reference)),
                label: labels.get(&candidate.reference).cloned(),
            })
            .collect();
        self.assessor.borrow_mut().select(&labelled)
    }

    fn assess(&self, observation: &ContentObservation<RetainedSourceText>) -> GoalDecision {
        // Oversized text was refused in `observe`, so this refusal cannot occur.
        self.assessor
            .borrow_mut()
            .assess(&observation.content)
            .map_or(GoalDecision::Continue, decision_of)
    }

    /// `run_traversal` calls its cancellation seam before and after each
    /// lookup. The call that directly follows a lookup honours only the host's
    /// cancellation and a refused over-budget observation, so a visit that
    /// exactly reaches a budget is still assessed.
    fn should_stop(&self, host_cancelled: bool) -> bool {
        let after_lookup = self.just_observed.replace(false);
        let hit = if host_cancelled {
            Some(RetainedStop::Cancelled)
        } else if self.refused_for_bytes.get() {
            Some(RetainedStop::ByteBudget)
        } else if after_lookup {
            None
        } else if self.requests.get() >= self.limits.max_requests {
            Some(RetainedStop::RequestBudget)
        } else if self.bytes.get() >= self.limits.max_bytes {
            Some(RetainedStop::ByteBudget)
        } else {
            None
        };
        if hit.is_some() {
            self.cause.set(hit);
        }
        hit.is_some()
    }
}

/// Walk retained sources from `seeds` toward `goal`.
///
/// Everything that goes wrong during the walk (missing text, version mismatch,
/// budgets) is recorded in the report.
///
/// # Errors
///
/// Returns the assessor's message when the goal is empty, oversized or has no
/// searchable term, or when `settings` are unusable. Nothing is looked up then.
pub fn run_retained_traversal(
    provider: &mut impl RetainedProvider,
    seeds: impl IntoIterator<Item = RetainedSeed>,
    goal: &str,
    settings: GoalSettings,
    policy: TraversalPolicy,
    limits: RetainedLimits,
    mut cancelled: impl FnMut() -> bool,
) -> Result<RetainedTraversalReport, String> {
    let run = Run {
        max_text_chars: settings.max_text_chars,
        assessor: RefCell::new(GoalAssessor::new(goal, settings)?),
        provider: RefCell::new(provider),
        labels: RefCell::new(HashMap::new()),
        limits,
        requests: Cell::new(0),
        bytes: Cell::new(0),
        just_observed: Cell::new(false),
        refused_for_bytes: Cell::new(false),
        cause: Cell::new(None),
    };
    let goal_text = TraversalGoal {
        query: goal.to_owned(),
    };
    let traversal = run_traversal(
        seeds
            .into_iter()
            .map(|seed| content_ref(&seed.source, seed.version.as_deref())),
        &goal_text,
        policy,
        |reference| run.observe(reference),
        |_, candidates| run.select(candidates),
        |_, observation| run.assess(observation),
        || run.should_stop(cancelled()),
    );
    let stop = match traversal.stop {
        TraversalStop::Cancelled => run.cause.get().unwrap_or(RetainedStop::Cancelled),
        other => other.into(),
    };
    let goal = run.assessor.borrow().record();
    Ok(RetainedTraversalReport {
        traversal,
        goal,
        stop,
        requests: run.requests.get(),
        bytes: run.bytes.get(),
    })
}

#[cfg(test)]
mod tests;
