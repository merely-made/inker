// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Explicit, bounded traversal of supplied content and engine-generated graphs.
//!
//! The provider observes a reference; a selector proposes an index into the
//! admitted frontier; the goal assessor decides whether the query is satisfied.
//! None of these seams requires a URL, transport, graph store or inference model.

use std::collections::HashMap;

/// A source-owned content reference. A version is part of traversal identity;
/// it is not an observation ID or a promise of byte-for-byte equivalence.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContentRef {
    pub source_id: String,
    pub node_id: String,
    pub version: Option<String>,
}

impl ContentRef {
    pub fn new(source_id: impl Into<String>, node_id: impl Into<String>) -> Self {
        Self {
            source_id: source_id.into(),
            node_id: node_id.into(),
            version: None,
        }
    }
}

/// User/application intent, supplied to both selection and goal assessment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraversalGoal {
    pub query: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraversalPolicy {
    /// Total provider calls, including failures and shorter-path reobservations.
    pub max_nodes: usize,
    pub max_depth: u32,
    /// Maximum outgoing references admitted from one observation.
    pub max_fanout: usize,
    /// Maximum pending references. Excess candidates are reported as omissions.
    pub max_pending: usize,
}

impl Default for TraversalPolicy {
    fn default() -> Self {
        Self {
            max_nodes: 50,
            max_depth: 2,
            max_fanout: 20,
            max_pending: 500,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraversalCandidate {
    pub reference: ContentRef,
    pub depth: u32,
}

/// One provider observation. Its receipt ID stays separate from source identity
/// and version; links can address other sources or engine-generated results.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentObservation<T> {
    pub observation_id: String,
    pub content: T,
    pub links: Vec<ContentRef>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoalDecision {
    Continue,
    Match,
    Satisfied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraversalStop {
    Exhausted,
    NodeBudget,
    Cancelled,
    GoalSatisfied,
    SelectionStopped,
    InvalidSelection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraversalVisit<T> {
    pub candidate: TraversalCandidate,
    pub observation: ContentObservation<T>,
    pub goal: GoalDecision,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservationFailure<E> {
    pub candidate: TraversalCandidate,
    pub error: E,
}

/// Omitted candidates remain visible in the receipt rather than masquerading as
/// complete coverage. Depth/fanout/pending bounds never grant more authority.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TraversalOmissions {
    pub depth: usize,
    pub fanout: usize,
    pub pending: usize,
    pub duplicates: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraversalReport<T, E> {
    pub visits: Vec<TraversalVisit<T>>,
    pub failures: Vec<ObservationFailure<E>>,
    pub examined: usize,
    pub stop: TraversalStop,
    pub omissions: TraversalOmissions,
}

/// Select the oldest admitted candidate. A caller can instead rank candidates
/// with local heuristics or inference. Selection can only choose an admitted
/// index and cannot enlarge the policy, add candidates, or initiate fetching.
pub fn breadth_first(_: &TraversalGoal, candidates: &[TraversalCandidate]) -> Option<usize> {
    (!candidates.is_empty()).then_some(0)
}

/// Run one explicitly requested traversal. The host/provider retains observation
/// and compute policy. Cancellation is checked before and after each synchronous
/// provider call; an in-progress engine operation must be bounded by its owner.
/// A strictly shorter admitted path updates a pending candidate or reobserves a
/// visited reference, so goal ranking cannot make a valid shallow path disappear.
/// Every repeat consumes the node budget and preserves its own observation receipt.
pub fn run_traversal<T, E>(
    seeds: impl IntoIterator<Item = ContentRef>,
    goal: &TraversalGoal,
    policy: TraversalPolicy,
    mut observe: impl FnMut(&ContentRef) -> Result<ContentObservation<T>, E>,
    mut select: impl FnMut(&TraversalGoal, &[TraversalCandidate]) -> Option<usize>,
    mut assess: impl FnMut(&TraversalGoal, &ContentObservation<T>) -> GoalDecision,
    mut cancelled: impl FnMut() -> bool,
) -> TraversalReport<T, E> {
    let mut best_depths = HashMap::new();
    let mut pending = Vec::new();
    let mut report = TraversalReport {
        visits: Vec::new(),
        failures: Vec::new(),
        examined: 0,
        stop: TraversalStop::Exhausted,
        omissions: TraversalOmissions::default(),
    };
    // Only a bounded prefix of seed input is consumed. This also permits callers
    // to supply generated seed iterators without requiring an unbounded scan.
    let mut seeds = seeds.into_iter();
    for reference in seeds.by_ref().take(policy.max_pending) {
        if best_depths.insert(reference.clone(), 0).is_none() {
            pending.push(TraversalCandidate {
                reference,
                depth: 0,
            });
        } else {
            report.omissions.duplicates += 1;
        }
    }

    // This count is a lower bound for an unconsumed generated seed tail. Exact
    // counting could itself require unbounded provider computation.
    if seeds.next().is_some() {
        report.omissions.pending += 1;
    }

    while !pending.is_empty() {
        if cancelled() {
            report.stop = TraversalStop::Cancelled;
            break;
        }
        if report.examined >= policy.max_nodes {
            report.stop = TraversalStop::NodeBudget;
            break;
        }
        let Some(index) = select(goal, &pending) else {
            report.stop = TraversalStop::SelectionStopped;
            break;
        };
        if index >= pending.len() {
            report.stop = TraversalStop::InvalidSelection;
            break;
        }
        if cancelled() {
            report.stop = TraversalStop::Cancelled;
            break;
        }
        let candidate = pending.remove(index);
        report.examined += 1;
        match observe(&candidate.reference) {
            Ok(observation) => {
                if cancelled() {
                    report.visits.push(TraversalVisit {
                        candidate,
                        observation,
                        goal: GoalDecision::Continue,
                    });
                    report.stop = TraversalStop::Cancelled;
                    break;
                }
                let decision = assess(goal, &observation);
                if decision != GoalDecision::Satisfied {
                    if candidate.depth >= policy.max_depth {
                        report.omissions.depth += observation.links.len();
                    } else {
                        let mut admitted = 0;
                        let child_depth = candidate.depth + 1;
                        for reference in &observation.links {
                            if best_depths
                                .get(reference)
                                .is_some_and(|best| *best <= child_depth)
                            {
                                report.omissions.duplicates += 1;
                            } else if admitted >= policy.max_fanout {
                                report.omissions.fanout += 1;
                            } else if let Some(queued) = pending
                                .iter_mut()
                                .find(|queued| queued.reference == *reference)
                            {
                                queued.depth = child_depth;
                                best_depths.insert(reference.clone(), child_depth);
                                admitted += 1;
                            } else if pending.len() >= policy.max_pending {
                                report.omissions.pending += 1;
                            } else {
                                best_depths.insert(reference.clone(), child_depth);
                                pending.push(TraversalCandidate {
                                    reference: reference.clone(),
                                    depth: child_depth,
                                });
                                admitted += 1;
                            }
                        }
                    }
                }
                report.visits.push(TraversalVisit {
                    candidate,
                    observation,
                    goal: decision,
                });
                if decision == GoalDecision::Satisfied {
                    report.stop = TraversalStop::GoalSatisfied;
                    break;
                }
            },
            Err(error) => {
                report
                    .failures
                    .push(ObservationFailure { candidate, error });
                if cancelled() {
                    report.stop = TraversalStop::Cancelled;
                    break;
                }
            },
        }
    }
    report
}

#[cfg(test)]
mod tests;
