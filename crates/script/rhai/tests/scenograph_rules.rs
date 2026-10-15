// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The Scenograph rule runner on real rhai (Scenograph editor plan, SE92):
//! `Condition::Script` conditions evaluated by [`RhaiValueEngine`] through the
//! `ValueEngine` contract, with scenograph linking no engine itself.

use std::collections::{BTreeMap, BTreeSet};

use scenograph::presentation::{
    Condition, Context, Effect, PRESENTATION_RULE_VERSION, PrimitiveKind, Property, Rule, RuleSet,
    Scalar, Target, ViewState,
};
use script_rhai::{RhaiValueEngine, register_libm_maths};

fn rule(id: &str, priority: i32, source: &str, details: bool) -> Rule {
    Rule {
        id: id.into(),
        label: id.into(),
        enabled: true,
        priority,
        target: Target {
            kind: PrimitiveKind::Node,
            ids: BTreeSet::new(),
        },
        condition: Condition::Script {
            source: source.into(),
        },
        effects: vec![Effect::Details { visible: details }],
    }
}

fn set(rules: Vec<Rule>) -> RuleSet {
    RuleSet {
        version: PRESENTATION_RULE_VERSION,
        rules,
    }
}

fn node(id: &str) -> Context {
    Context {
        id: id.into(),
        kind: PrimitiveKind::Node,
        facts: BTreeMap::new(),
        states: BTreeSet::new(),
        screen_size: Some(400.0),
        previous_matches: BTreeSet::new(),
    }
}

fn trusted() -> RhaiValueEngine {
    let mut engine = RhaiValueEngine::new();
    register_libm_maths(&mut engine).unwrap();
    engine
}

fn winner(set: &RuleSet, context: &Context, engine: &mut RhaiValueEngine) -> Option<String> {
    set.resolve_with_scripts(context, engine)
        .unwrap()
        .winners
        .get(&Property::Details)
        .map(|decision| decision.rule_id.clone())
}

#[test]
fn a_rule_reads_facts_and_states() {
    let rules = set(vec![
        rule("heavy", 1, "facts.weight > 2.0", true),
        rule("picked", 2, r#"states.contains("selected")"#, true),
        rule("image", 3, r#"facts.mime == "image/png""#, true),
    ]);
    let mut item = node("a");
    item.facts.insert("weight".into(), Scalar::Number(2.5));
    assert_eq!(
        winner(&rules, &item, &mut trusted()).as_deref(),
        Some("heavy")
    );
    item.facts.insert("weight".into(), Scalar::Number(1.0));
    assert_eq!(winner(&rules, &item, &mut trusted()), None);
    item.states.insert(ViewState::Selected);
    assert_eq!(
        winner(&rules, &item, &mut trusted()).as_deref(),
        Some("picked")
    );
    item.facts
        .insert("mime".into(), Scalar::Text("image/png".into()));
    assert_eq!(
        winner(&rules, &item, &mut trusted()).as_deref(),
        Some("image")
    );
}

#[test]
fn a_missing_fact_is_a_traced_non_match_not_a_crash() {
    let rules = set(vec![
        rule("needs-weight", 1, "facts.weight > 2.0", true),
        rule(
            "checks-first",
            0,
            r#""weight" in facts && facts.weight > 2.0"#,
            true,
        ),
    ]);
    let resolved = rules
        .resolve_with_scripts(&node("a"), &mut trusted())
        .unwrap();
    assert!(resolved.matched_rules.is_empty());
    // Rhai reads an absent map key as `()`, and `() > 2.0` is false, not an
    // error: a missing fact is a quiet non-match, like the declarative
    // conditions' "fact: missing". Negating it would match, as `Not` does there.
    assert_eq!(resolved.traces[0].reason, "script: false");
    assert_eq!(resolved.traces[1].reason, "script: false");
    let negated = set(vec![rule("not-heavy", 1, "!(facts.weight > 2.0)", true)]);
    let resolved = negated
        .resolve_with_scripts(&node("a"), &mut trusted())
        .unwrap();
    assert!(resolved.matched_rules.contains("not-heavy"));
}

#[test]
fn a_script_runs_the_hysteresis_the_declarative_condition_has() {
    // Compact below 100 pixels; back to detail only above 140.
    let compact = set(vec![rule(
        "compact",
        1,
        "if previously_matched { screen_size < 140.0 } else { screen_size <= 100.0 }",
        false,
    )]);
    let at = |size: f64, was: bool| {
        let mut item = node("a");
        item.screen_size = Some(size);
        if was {
            item.previous_matches.insert("compact".into());
        }
        compact
            .resolve_with_scripts(&item, &mut trusted())
            .unwrap()
            .matched_rules
            .contains("compact")
    };
    assert!(at(90.0, false));
    assert!(!at(120.0, false), "does not enter between the thresholds");
    assert!(at(120.0, true), "does not leave between the thresholds");
    assert!(!at(150.0, true));
}

#[test]
fn rule_maths_goes_through_the_host_functions() {
    // exp(-d*d) falls off with distance; the threshold is the libm value.
    let falloff = set(vec![rule("near", 1, "exp(-facts.d * facts.d) > 0.5", true)]);
    let mut near = node("a");
    near.facts.insert("d".into(), Scalar::Number(0.8));
    assert_eq!(
        winner(&falloff, &near, &mut trusted()).as_deref(),
        Some("near")
    );
    near.facts.insert("d".into(), Scalar::Number(0.9));
    assert_eq!(winner(&falloff, &near, &mut trusted()), None);
    assert!(libm::exp(-0.8 * 0.8) > 0.5 && libm::exp(-0.9 * 0.9) < 0.5);
}

#[test]
fn a_narrower_binding_set_has_no_maths_and_the_other_rules_still_resolve() {
    // A shared scene's rules run with fewer bindings (SE36): no host maths.
    let rules = set(vec![
        rule("maths", 9, "sin(1.0) > 0.0", true),
        rule("plain", 0, "true", false),
    ]);
    let mut narrow = RhaiValueEngine::new();
    let resolved = rules.resolve_with_scripts(&node("a"), &mut narrow).unwrap();
    assert!(
        resolved.traces[0]
            .reason
            .starts_with("script: script error")
    );
    assert_eq!(resolved.winners[&Property::Details].rule_id, "plain");
}

#[test]
fn a_runaway_rule_is_stopped_by_the_budget() {
    let rules = set(vec![
        rule("forever", 9, "loop { }", true),
        rule("plain", 0, "true", false),
    ]);
    let mut engine = trusted();
    let resolved = rules.resolve_with_scripts(&node("a"), &mut engine).unwrap();
    assert_eq!(
        resolved.traces[0].reason,
        "script: the script ran past its budget"
    );
    assert_eq!(resolved.winners[&Property::Details].rule_id, "plain");
    // And the engine is still good for the next occurrence.
    assert!(winner(&rules, &node("b"), &mut engine).is_some());
}

#[test]
fn a_script_cannot_carry_state_from_one_occurrence_to_the_next() {
    let rules = set(vec![rule(
        "leak",
        1,
        "let seen = facts; id == \"first\"",
        true,
    )]);
    let mut engine = trusted();
    assert_eq!(
        winner(&rules, &node("first"), &mut engine).as_deref(),
        Some("leak")
    );
    assert_eq!(winner(&rules, &node("second"), &mut engine), None);
    let probe = set(vec![rule("probe", 1, "seen == ()", true)]);
    let resolved = probe
        .resolve_with_scripts(&node("third"), &mut engine)
        .unwrap();
    assert!(
        resolved.traces[0]
            .reason
            .starts_with("script: script error")
    );
}

#[test]
fn the_same_occurrence_resolves_the_same_way_twice() {
    let rules = set(vec![rule("a", 1, "exp(facts.x) < 3.0", true)]);
    let mut item = node("n");
    item.facts.insert("x".into(), Scalar::Number(1.0));
    let first = rules.resolve_with_scripts(&item, &mut trusted()).unwrap();
    let second = rules.resolve_with_scripts(&item, &mut trusted()).unwrap();
    assert_eq!(first, second);
}
