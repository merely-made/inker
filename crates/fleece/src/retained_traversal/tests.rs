// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;
use eidetic_search::goal::SelectionRule;

/// A map of what the host retains. There is no network behind it.
#[derive(Default)]
struct Held {
    sources: HashMap<RecallSourceId, RetainedObservation>,
    lookups: Vec<RecallSourceId>,
}

impl RetainedProvider for Held {
    fn retained(
        &mut self,
        source: &RecallSourceId,
        _: Option<&str>,
    ) -> Option<RetainedObservation> {
        self.lookups.push(source.clone());
        self.sources.get(source).cloned()
    }
}

fn id(name: &str) -> RecallSourceId {
    RecallSourceId {
        kind: RecallSourceKind::PageText,
        owner: Some("session-1".into()),
        source: name.into(),
        node: None,
    }
}

fn link(name: &str, label: Option<&str>) -> RetainedLink {
    RetainedLink {
        source: id(name),
        version: None,
        label: label.map(str::to_owned),
    }
}

impl Held {
    fn with(mut self, name: &str, version: &str, text: &str, links: Vec<RetainedLink>) -> Self {
        self.sources.insert(
            id(name),
            RetainedObservation {
                text: RetainedSourceText {
                    source: id(name),
                    version: version.into(),
                    title: None,
                    text: text.into(),
                },
                links,
            },
        );
        self
    }
}

fn seed(name: &str) -> RetainedSeed {
    RetainedSeed {
        source: id(name),
        version: None,
    }
}

fn settings(satisfy_after: usize) -> GoalSettings {
    GoalSettings {
        satisfy_after,
        ..GoalSettings::default()
    }
}

fn run(
    held: &mut Held,
    seeds: Vec<RetainedSeed>,
    settings: GoalSettings,
    policy: TraversalPolicy,
    limits: RetainedLimits,
) -> RetainedTraversalReport {
    run_retained_traversal(
        held,
        seeds,
        "borrow checker",
        settings,
        policy,
        limits,
        || false,
    )
    .expect("goal accepted")
}

fn site() -> Held {
    Held::default()
        .with(
            "a",
            "v1",
            "an introduction to memory",
            vec![
                link("cooking", Some("pasta recipes")),
                link("b", Some("the borrow checker explained")),
            ],
        )
        .with("b", "v1", "the borrow checker rules", vec![link("c", None)])
        .with("c", "v1", "a borrow checker appendix", Vec::new())
        .with("cooking", "v1", "boil water", Vec::new())
}

#[test]
fn goal_satisfied_stops_with_cited_versions_and_reasons() {
    let mut held = site();
    let report = run(
        &mut held,
        vec![seed("a")],
        settings(1),
        TraversalPolicy::default(),
        RetainedLimits::default(),
    );
    assert_eq!(report.stop, RetainedStop::GoalSatisfied);
    // The labelled link was chosen over the older, irrelevant one.
    assert_eq!(held.lookups, vec![id("a"), id("b")]);
    let choice = &report.goal.selections[1];
    assert_eq!(choice.source, id("b"));
    assert_eq!(choice.rule, SelectionRule::GoalTermOverlap);
    assert_eq!(choice.matched_terms, vec!["borrow", "checker"]);
    let receipt = report.receipt();
    assert_eq!(receipt.visits.len(), 2);
    assert_eq!(receipt.visits[1].source, id("b"));
    assert_eq!(receipt.visits[1].retained_version, "v1");
    assert_eq!(receipt.visits[1].verdict, GoalVerdict::Satisfied);
    assert_eq!(receipt.goal.assessments[1].version, "v1");
    assert_eq!(receipt.goal.matched, 1);
}

#[test]
fn receipt_round_trips_through_json() {
    let mut held = site();
    let report = run(
        &mut held,
        vec![seed("a")],
        settings(1),
        TraversalPolicy::default(),
        RetainedLimits::default(),
    );
    let receipt = report.receipt();
    let json = serde_json::to_string(&receipt).expect("serialize");
    assert!(
        !json.contains("the borrow checker rules"),
        "retained text stays out"
    );
    let back: RetainedTraversalReceipt = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, receipt);
}

#[test]
fn omissions_are_reported() {
    let mut held = site();
    let report = run(
        &mut held,
        vec![seed("a")],
        settings(9),
        TraversalPolicy {
            max_fanout: 1,
            ..TraversalPolicy::default()
        },
        RetainedLimits::default(),
    );
    assert_eq!(report.traversal.omissions.fanout, 1);
    assert_eq!(report.receipt().omissions.fanout, 1);
    // `cooking` was admitted first, so the labelled link was the one left out.
    assert!(held.lookups.contains(&id("cooking")));
    assert!(!held.lookups.contains(&id("b")));
}

#[test]
fn version_mismatch_is_refused() {
    let mut held = site();
    let report = run(
        &mut held,
        vec![RetainedSeed {
            source: id("a"),
            version: Some("v0".into()),
        }],
        settings(1),
        TraversalPolicy::default(),
        RetainedLimits::default(),
    );
    assert!(report.traversal.visits.is_empty());
    assert_eq!(
        report.traversal.failures[0].error,
        RetainedFailure::VersionMismatch {
            requested: "v0".into(),
            retained: "v1".into()
        }
    );
    assert!(report.goal.assessments.is_empty());
    assert_eq!(report.stop, RetainedStop::Exhausted);
}

#[test]
fn matching_version_is_accepted_and_missing_is_a_failure() {
    let mut held = Held::default().with(
        "a",
        "v1",
        "x",
        vec![RetainedLink {
            source: id("gone"),
            version: Some("v9".into()),
            label: None,
        }],
    );
    let report = run(
        &mut held,
        vec![RetainedSeed {
            source: id("a"),
            version: Some("v1".into()),
        }],
        settings(9),
        TraversalPolicy::default(),
        RetainedLimits::default(),
    );
    assert_eq!(report.traversal.visits.len(), 1);
    assert_eq!(report.traversal.failures[0].error, RetainedFailure::Missing);
    let receipt = report.receipt();
    assert_eq!(receipt.failures[0].source, id("gone"));
    assert_eq!(receipt.failures[0].requested_version.as_deref(), Some("v9"));
}

#[test]
fn source_mismatch_is_refused() {
    let mut held = Held::default().with("a", "v1", "x", Vec::new());
    held.sources.get_mut(&id("a")).unwrap().text.source = id("other");
    let report = run(
        &mut held,
        vec![seed("a")],
        settings(1),
        TraversalPolicy::default(),
        RetainedLimits::default(),
    );
    assert_eq!(
        report.traversal.failures[0].error,
        RetainedFailure::SourceMismatch
    );
}

#[test]
fn request_budget_stops_with_its_own_reason() {
    let mut held = site();
    let report = run(
        &mut held,
        vec![seed("a")],
        settings(9),
        TraversalPolicy::default(),
        RetainedLimits {
            max_requests: 2,
            ..RetainedLimits::default()
        },
    );
    assert_eq!(report.stop, RetainedStop::RequestBudget);
    assert_eq!(report.requests, 2);
    // The second visit was still assessed.
    assert_eq!(report.goal.assessments.len(), 2);
    assert_eq!(held.lookups.len(), 2);
}

#[test]
fn byte_budget_refuses_the_overflowing_source_and_stops() {
    let mut held = site();
    let a_bytes = "an introduction to memory".len();
    let report = run(
        &mut held,
        vec![seed("a")],
        settings(9),
        TraversalPolicy::default(),
        RetainedLimits {
            max_bytes: a_bytes + 5,
            ..RetainedLimits::default()
        },
    );
    assert_eq!(report.stop, RetainedStop::ByteBudget);
    assert_eq!(report.bytes, a_bytes);
    assert_eq!(report.traversal.visits.len(), 1);
    assert_eq!(
        report.traversal.failures[0].error,
        RetainedFailure::ByteBudget {
            needed: "the borrow checker rules".len(),
            remaining: 5
        }
    );
    // Nothing further was looked up after the refusal.
    assert_eq!(held.lookups.len(), 2);
}

#[test]
fn exactly_spent_byte_budget_still_assesses_the_last_visit() {
    let mut held = site();
    let a_bytes = "an introduction to memory".len();
    let report = run(
        &mut held,
        vec![seed("a")],
        settings(9),
        TraversalPolicy::default(),
        RetainedLimits {
            max_bytes: a_bytes,
            ..RetainedLimits::default()
        },
    );
    assert_eq!(report.stop, RetainedStop::ByteBudget);
    assert_eq!(report.goal.assessments.len(), 1);
    assert_eq!(held.lookups.len(), 1);
}

#[test]
fn oversized_text_is_refused_before_assessment() {
    let mut held = site();
    let report = run(
        &mut held,
        vec![seed("a")],
        GoalSettings {
            max_text_chars: 10,
            ..settings(1)
        },
        TraversalPolicy::default(),
        RetainedLimits::default(),
    );
    assert_eq!(
        report.traversal.failures[0].error,
        RetainedFailure::Oversized
    );
    assert!(report.goal.assessments.is_empty());
}

#[test]
fn host_cancellation_is_reported_as_cancelled() {
    let mut held = site();
    let calls = Cell::new(0);
    let report = run_retained_traversal(
        &mut held,
        vec![seed("a")],
        "borrow checker",
        settings(9),
        TraversalPolicy::default(),
        RetainedLimits::default(),
        || {
            calls.set(calls.get() + 1);
            calls.get() > 4
        },
    )
    .expect("goal accepted");
    assert_eq!(report.stop, RetainedStop::Cancelled);
    assert!(held.lookups.len() < 4);
}

#[test]
fn refused_goals_error_before_any_lookup() {
    let mut held = site();
    let error = run_retained_traversal(
        &mut held,
        vec![seed("a")],
        "   ",
        GoalSettings::default(),
        TraversalPolicy::default(),
        RetainedLimits::default(),
        || false,
    )
    .expect_err("empty goal");
    assert!(error.contains("searchable"));
    assert!(held.lookups.is_empty());
}

#[test]
fn identity_mapping_round_trips_and_keeps_awkward_strings_distinct() {
    let awkward = RecallSourceId {
        kind: RecallSourceKind::Clip,
        owner: Some(r#"o"],["x"#.into()),
        source: "a|b:c".into(),
        node: Some(String::new()),
    };
    let none_node = RecallSourceId {
        node: None,
        ..awkward.clone()
    };
    let reference = content_ref(&awkward, Some("v3"));
    assert_eq!(reference.version.as_deref(), Some("v3"));
    assert_eq!(recall_source(&reference), Some(awkward.clone()));
    assert_ne!(
        content_ref(&awkward, None).node_id,
        content_ref(&none_node, None).node_id
    );
    assert_eq!(recall_source(&ContentRef::new("page", "x")), None);
}
