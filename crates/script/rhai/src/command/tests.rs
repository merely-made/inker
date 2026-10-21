// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;
use hest::catalogue::{command, ids};
use inker::{
    Block, BlockEvaluators, DocumentProvenance, DocumentTrustState, EngineDocument, EngineInput,
    EvaluationPolicy, InlineSpan, evaluate_blocks,
};

const SCHEMA: &str = "test.commands/v1";

fn offered() -> CommandSet {
    let mut offered = CommandSet::new();
    for id in [ids::VIEW_FIT, ids::PHYSICS_TOGGLE] {
        offered.register(command(id).unwrap());
    }
    offered
}

fn collect(source: &str) -> Result<Vec<Invocation>, String> {
    collect_invocations(
        source,
        &offered(),
        SCHEMA,
        Budget::Steps(10_000),
        CommandLimits::default(),
    )
}

#[test]
fn requests_preserve_shared_ids_declared_effects_and_exact_bytes_in_order() {
    let actual = collect(
        r#"
        command("view:fit", "hé\x00");
        let arguments = blob(3);
        arguments[1] = 255;
        arguments[2] = 128;
        command("physics:toggle", arguments);
    "#,
    )
    .unwrap();
    assert_eq!(
        actual,
        vec![
            command(ids::VIEW_FIT)
                .unwrap()
                .invocation(SCHEMA, "hé\0".as_bytes().to_vec()),
            command(ids::PHYSICS_TOGGLE)
                .unwrap()
                .invocation(SCHEMA, vec![0, 255, 128]),
        ]
    );
}

#[test]
fn disabled_unknown_and_caught_collection_errors_discard_the_whole_batch() {
    let mut declarations = offered();
    declarations.set_disabled(ids::PHYSICS_TOGGLE, Some("paused by host".into()));
    for (source, reason) in [
        (
            r#"command("view:fit", ""); command("physics:toggle", "");"#,
            "paused by host",
        ),
        (
            r#"command("view:fit", ""); command("host:unknown", "");"#,
            "not offered",
        ),
        (
            r#"try { command("host:unknown", ""); } catch (error) {} command("view:fit", "");"#,
            "not offered",
        ),
        (
            r#"try { command("physics:toggle", ""); } catch (error) {}"#,
            "paused by host",
        ),
    ] {
        let error = collect_invocations(
            source,
            &declarations,
            SCHEMA,
            Budget::Steps(10_000),
            CommandLimits::default(),
        )
        .unwrap_err();
        assert!(error.contains(reason), "{error}");
    }
}

#[test]
fn invocation_and_aggregate_argument_limits_reject_the_entire_batch() {
    let limits = CommandLimits {
        max_invocations: 2,
        max_argument_bytes: 4,
    };
    let run =
        |source| collect_invocations(source, &offered(), SCHEMA, Budget::Steps(10_000), limits);
    assert_eq!(
        run(r#"command("view:fit", "ab"); command("view:fit", "cd");"#)
            .unwrap()
            .len(),
        2
    );
    assert!(
        run(r#"command("view:fit", ""); command("view:fit", ""); command("view:fit", "");"#)
            .unwrap_err()
            .contains("invocation limit")
    );
    assert!(
        run(r#"command("view:fit", "abc"); command("view:fit", blob(2));"#)
            .unwrap_err()
            .contains("argument byte limit")
    );
    assert!(
        run(r#"try { command("view:fit", "12345"); } catch (error) {}"#)
            .unwrap_err()
            .contains("argument byte limit")
    );
    assert!(
        collect_invocations(
            r#"command("view:fit", "");"#,
            &offered(),
            SCHEMA,
            Budget::Steps(10_000),
            CommandLimits {
                max_invocations: 0,
                max_argument_bytes: 0
            }
        )
        .is_err()
    );
}

#[test]
fn parse_runtime_and_budget_failures_leave_no_requests_for_the_next_run() {
    let mut evaluator = RhaiCommandEvaluator::new(offered(), SCHEMA, CommandLimits::default());
    for source in [
        r#"command("view:fit", ""); this is invalid @@@"#,
        r#"command("view:fit", ""); throw "failed after collection";"#,
        r#"command("view:fit", ""); loop {}"#,
    ] {
        assert!(evaluator.eval_block(source, 100).is_err());
        let empty = evaluator.eval_block("42", 1_000).unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<Invocation>>(&empty.text).unwrap(),
            Vec::new()
        );
        let valid = evaluator
            .eval_block(r#"command("view:fit", "next");"#, 1_000)
            .unwrap();
        let requests: Vec<Invocation> = serde_json::from_str(&valid.text).unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].arguments, b"next");
    }
    assert!(
        collect_invocations(
            "42",
            &offered(),
            SCHEMA,
            Budget::Steps(0),
            CommandLimits::default()
        )
        .is_err()
    );
    assert_eq!(
        collect_invocations(
            "42",
            &offered(),
            SCHEMA,
            Budget::Unbounded,
            CommandLimits::default()
        )
        .unwrap(),
        Vec::new()
    );
}

#[test]
fn default_note_evaluator_has_no_command_binding_and_sandbox_stays_closed() {
    assert!(
        crate::RhaiEvaluator::new()
            .eval_block(r#"command("view:fit", "");"#, 10_000)
            .is_err()
    );
    for source in [
        r#"import "sibling" as m;"#,
        r#"eval("40+2")"#,
        r#"open_file("/etc/passwd")"#,
    ] {
        assert!(collect(source).is_err());
    }
}

fn document(source: &str) -> EngineDocument {
    EngineDocument {
        address: "commands.knot".into(),
        title: None,
        content_type: "text/x-knot".into(),
        lang: None,
        provenance: DocumentProvenance::default(),
        trust: DocumentTrustState::Unknown,
        diagnostics: Vec::new(),
        navigation: Default::default(),
        blocks: vec![Block::CodeBlock {
            language: Some("rhai-command eval".into()),
            text: source.into(),
        }],
    }
}

fn registry() -> BlockEvaluators {
    let mut registry = BlockEvaluators::new();
    registry.register(Box::new(RhaiCommandEvaluator::new(
        offered(),
        SCHEMA,
        CommandLimits::default(),
    )));
    registry
}

#[test]
fn actual_block_pass_emits_plain_invocation_data_with_generated_provenance() {
    let source = r#"command("view:fit", "\x00payload");"#;
    let mut document = document(source);
    document.navigation.block_count = 1;
    let mut registry = registry();
    assert_eq!(registry.languages(), vec!["rhai-command"]);
    let mut evaluate = |language: &str, source: &str| registry.evaluate(language, source, 10_000);
    let mut render = |_: &EngineInput| -> Result<EngineDocument, String> {
        panic!("plain JSON must not be nested rendered")
    };
    let policy = EvaluationPolicy::for_own_notes(vec!["rhai-command".into()]);
    let outcome = evaluate_blocks(&mut document, &mut evaluate, &mut render, &policy);
    assert_eq!(outcome.evaluated, 1);
    assert!(outcome.denied.is_empty());
    assert!(outcome.failed.is_empty());
    assert_eq!(document.navigation, Default::default());
    let Block::Paragraph { spans } = &document.blocks[0] else {
        panic!("plain JSON paragraph");
    };
    let InlineSpan::Text(text) = &spans[0] else {
        panic!("plain JSON text");
    };
    let requests: Vec<Invocation> = serde_json::from_str(text).unwrap();
    assert_eq!(
        requests,
        vec![
            command(ids::VIEW_FIT)
                .unwrap()
                .invocation(SCHEMA, b"\0payload".to_vec())
        ]
    );
    assert_eq!(
        outcome
            .provenance
            .get(0)
            .unwrap()
            .provenance
            .source_label
            .as_deref(),
        Some("evaluated:rhai-command")
    );
}

#[test]
fn denied_fences_and_failed_scripts_remain_source_without_generated_provenance() {
    for policy in [
        EvaluationPolicy::deny_all(),
        EvaluationPolicy::for_own_notes(vec!["rhai".into()]),
    ] {
        let mut document = document(r#"command("view:fit", "");"#);
        let before = document.blocks.clone();
        let mut evaluate = |_: &str, _: &str| -> Result<EvalOutput, String> {
            panic!("denied fences must not evaluate")
        };
        let mut render = |_: &EngineInput| -> Result<EngineDocument, String> {
            panic!("denied fences must not render")
        };
        let outcome = evaluate_blocks(&mut document, &mut evaluate, &mut render, &policy);
        assert_eq!(outcome.denied.len(), 1);
        assert_eq!(outcome.evaluated, 0);
        assert_eq!(document.blocks, before);
        assert!(outcome.provenance.get(0).is_none());
    }
    let mut document = document(r#"command("view:fit", ""); throw "stop";"#);
    let before = document.blocks.clone();
    let mut registry = registry();
    let mut evaluate = |language: &str, source: &str| registry.evaluate(language, source, 10_000);
    let mut render = |_: &EngineInput| -> Result<EngineDocument, String> {
        panic!("failed fences must not render")
    };
    let outcome = evaluate_blocks(
        &mut document,
        &mut evaluate,
        &mut render,
        &EvaluationPolicy::for_own_notes(vec!["rhai-command".into()]),
    );
    assert_eq!(outcome.failed.len(), 1);
    assert_eq!(outcome.evaluated, 0);
    assert_eq!(document.blocks, before);
    assert!(outcome.provenance.get(0).is_none());
}
