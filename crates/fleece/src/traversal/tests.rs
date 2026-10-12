// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;
use std::cell::Cell;

fn node(id: &str) -> ContentRef {
    ContentRef::new("local-engine", id)
}

fn engine(reference: &ContentRef) -> Result<ContentObservation<String>, ()> {
    // The engine generates related results, including a cycle. There is no URL
    // or fetcher in this fixture; observation and source IDs are distinct.
    let links = match reference.node_id.as_str() {
        "root" => vec![node("irrelevant"), node("answer")],
        "answer" => vec![node("root")],
        _ => Vec::new(),
    };
    Ok(ContentObservation {
        observation_id: format!("receipt:{}", reference.node_id),
        content: reference.node_id.clone(),
        links,
    })
}

fn goal() -> TraversalGoal {
    TraversalGoal {
        query: "answer".into(),
    }
}

#[test]
fn generated_graph_selection_reaches_goal_before_irrelevant_branch() {
    let report = run_traversal(
        [node("root")],
        &goal(),
        TraversalPolicy::default(),
        engine,
        |goal, candidates| {
            candidates
                .iter()
                .position(|candidate| candidate.reference.node_id == goal.query)
                .or_else(|| breadth_first(goal, candidates))
        },
        |goal, observation| {
            if observation.content == goal.query {
                GoalDecision::Satisfied
            } else {
                GoalDecision::Continue
            }
        },
        || false,
    );
    assert_eq!(report.stop, TraversalStop::GoalSatisfied);
    assert_eq!(report.examined, 2);
    assert_eq!(report.visits[1].candidate.reference.node_id, "answer");
    assert_eq!(
        report.visits[1].observation.observation_id,
        "receipt:answer"
    );
}

#[test]
fn cycle_is_observed_once_and_node_budget_is_typed() {
    let report = run_traversal(
        [node("root")],
        &goal(),
        TraversalPolicy::default(),
        engine,
        breadth_first,
        |_, _| GoalDecision::Continue,
        || false,
    );
    assert_eq!(report.stop, TraversalStop::Exhausted);
    assert_eq!(report.examined, 3);
    assert_eq!(report.omissions.duplicates, 1);
    let capped = run_traversal(
        [node("root")],
        &goal(),
        TraversalPolicy {
            max_nodes: 1,
            ..TraversalPolicy::default()
        },
        engine,
        breadth_first,
        |_, _| GoalDecision::Continue,
        || false,
    );
    assert_eq!(capped.stop, TraversalStop::NodeBudget);
    assert_eq!(capped.examined, 1);
}

#[test]
fn depth_fanout_and_pending_bounds_report_omissions() {
    for (policy, expected) in [
        (
            TraversalPolicy {
                max_depth: 0,
                ..TraversalPolicy::default()
            },
            (2, 0, 0),
        ),
        (
            TraversalPolicy {
                max_fanout: 1,
                ..TraversalPolicy::default()
            },
            (0, 1, 0),
        ),
        (
            TraversalPolicy {
                max_pending: 1,
                ..TraversalPolicy::default()
            },
            (0, 0, 1),
        ),
    ] {
        let report = run_traversal(
            [node("root")],
            &goal(),
            policy,
            engine,
            breadth_first,
            |_, _| GoalDecision::Continue,
            || false,
        );
        assert_eq!(
            (
                report.omissions.depth,
                report.omissions.fanout,
                report.omissions.pending
            ),
            expected
        );
    }
}

#[test]
fn inference_selection_cannot_invent_an_index_or_override_budget() {
    let report = run_traversal(
        [node("root")],
        &goal(),
        TraversalPolicy::default(),
        engine,
        |_, _| Some(99),
        |_, _| GoalDecision::Continue,
        || false,
    );
    assert_eq!(report.stop, TraversalStop::InvalidSelection);
    assert_eq!(report.examined, 0);
    let calls = Cell::new(0);
    let zero = run_traversal(
        [node("root")],
        &goal(),
        TraversalPolicy {
            max_nodes: 0,
            ..TraversalPolicy::default()
        },
        engine,
        |_, _| {
            calls.set(calls.get() + 1);
            Some(0)
        },
        |_, _| GoalDecision::Continue,
        || false,
    );
    assert_eq!(zero.stop, TraversalStop::NodeBudget);
    assert_eq!(calls.get(), 0);
}

#[test]
fn versions_are_separate_from_observations_and_cancellation_stops_expansion() {
    let mut first = node("root");
    first.version = Some("v1".into());
    let mut second = first.clone();
    second.version = Some("v2".into());
    let observations = Cell::new(0);
    let report = run_traversal(
        [first, second],
        &goal(),
        TraversalPolicy::default(),
        |reference| {
            observations.set(observations.get() + 1);
            engine(reference)
        },
        breadth_first,
        |_, _| GoalDecision::Continue,
        || observations.get() > 0,
    );
    assert_eq!(report.stop, TraversalStop::Cancelled);
    assert_eq!(report.examined, 1);
    assert_eq!(
        report.visits[0].candidate.reference.version.as_deref(),
        Some("v1")
    );
    assert_eq!(report.visits[0].observation.observation_id, "receipt:root");
    let report = run_traversal(
        [report.visits[0].candidate.reference.clone(), {
            let mut version = node("root");
            version.version = Some("v2".into());
            version
        }],
        &goal(),
        TraversalPolicy {
            max_depth: 0,
            ..TraversalPolicy::default()
        },
        engine,
        breadth_first,
        |_, _| GoalDecision::Continue,
        || false,
    );
    assert_eq!(report.examined, 2);
}

#[test]
fn cancellation_during_final_failed_observation_preserves_failure_and_stop_reason() {
    let cancelled = Cell::new(false);
    let report = run_traversal(
        [node("last")],
        &goal(),
        TraversalPolicy::default(),
        |_| {
            cancelled.set(true);
            Err::<ContentObservation<()>, _>("engine stopped")
        },
        breadth_first,
        |_, _| GoalDecision::Continue,
        || cancelled.get(),
    );
    assert_eq!(report.stop, TraversalStop::Cancelled);
    assert_eq!(report.examined, 1);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].error, "engine stopped");
}

#[test]
fn goal_ranking_retains_shorter_path_reobservations_within_node_budget() {
    for max_nodes in [7, 6] {
        let mut receipt = 0;
        let report = run_traversal(
            [node("root")],
            &goal(),
            TraversalPolicy {
                max_depth: 3,
                max_nodes,
                ..TraversalPolicy::default()
            },
            |reference| {
                receipt += 1;
                let links = match reference.node_id.as_str() {
                    "root" => vec![node("A"), node("B")],
                    "A" => vec![node("A2")],
                    "A2" | "B" => vec![node("C")],
                    "C" => vec![node("answer")],
                    _ => Vec::new(),
                };
                Ok::<_, ()>(ContentObservation {
                    observation_id: format!("receipt:{receipt}"),
                    content: reference.node_id.clone(),
                    links,
                })
            },
            |_, candidates| {
                candidates
                    .iter()
                    .enumerate()
                    .min_by_key(
                        |(_, candidate)| match candidate.reference.node_id.as_str() {
                            "A" => 0,
                            "A2" => 1,
                            "C" => 2,
                            "B" => 3,
                            _ => 4,
                        },
                    )
                    .map(|(index, _)| index)
            },
            |goal, observation| {
                if observation.content == goal.query {
                    GoalDecision::Satisfied
                } else {
                    GoalDecision::Continue
                }
            },
            || false,
        );
        assert_eq!(report.examined, max_nodes);
        let repeated = report
            .visits
            .iter()
            .filter(|visit| visit.candidate.reference.node_id == "C")
            .collect::<Vec<_>>();
        assert_eq!(repeated.len(), 2);
        assert_eq!(repeated[0].candidate.depth, 3);
        assert_eq!(repeated[1].candidate.depth, 2);
        assert_ne!(
            repeated[0].observation.observation_id,
            repeated[1].observation.observation_id
        );
        if max_nodes == 7 {
            assert_eq!(report.stop, TraversalStop::GoalSatisfied);
            let answer = report.visits.last().unwrap();
            assert_eq!(answer.candidate.reference.node_id, "answer");
            assert_eq!(answer.candidate.depth, 3);
        } else {
            assert_eq!(report.stop, TraversalStop::NodeBudget);
            assert!(
                !report
                    .visits
                    .iter()
                    .any(|visit| visit.candidate.reference.node_id == "answer")
            );
        }
    }
}
