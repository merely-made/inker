// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! script-rhai's command tests, translated to Lua, plus the cases Lua adds:
//! a refusal caught by `pcall`, argument types, and the string limit.

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
        command("view:fit", "hé\0");
        command("physics:toggle", string.char(0, 255, 128));
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
            r#"command("view:fit", ""); command("physics:toggle", "")"#,
            "paused by host",
        ),
        (
            r#"command("view:fit", ""); command("host:unknown", "")"#,
            "not offered",
        ),
        (
            r#"pcall(command, "host:unknown", ""); command("view:fit", "")"#,
            "not offered",
        ),
        (r#"pcall(command, "physics:toggle", "")"#, "paused by host"),
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
        run(r#"command("view:fit", "ab"); command("view:fit", "cd")"#)
            .unwrap()
            .len(),
        2
    );
    assert!(
        run(r#"command("view:fit", ""); command("view:fit", ""); command("view:fit", "")"#)
            .unwrap_err()
            .contains("invocation limit")
    );
    assert!(
        run(r#"command("view:fit", "abc"); command("view:fit", string.rep("x", 2))"#)
            .unwrap_err()
            .contains("argument byte limit")
    );
    assert!(
        run(r#"pcall(command, "view:fit", "12345")"#)
            .unwrap_err()
            .contains("argument byte limit")
    );
    assert!(
        collect_invocations(
            r#"command("view:fit", "")"#,
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
    let mut evaluator = LuaCommandEvaluator::new(offered(), SCHEMA, CommandLimits::default());
    for source in [
        r#"command("view:fit", ""); this is invalid @@@"#,
        r#"command("view:fit", ""); error("failed after collection")"#,
        r#"command("view:fit", ""); while true do end"#,
    ] {
        assert!(evaluator.eval_block(source, 100).is_err());
        let empty = evaluator.eval_block("local x = 42", 1_000).unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<Invocation>>(&empty.text).unwrap(),
            Vec::new()
        );
        let valid = evaluator
            .eval_block(r#"command("view:fit", "next")"#, 1_000)
            .unwrap();
        let requests: Vec<Invocation> = serde_json::from_str(&valid.text).unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].arguments, b"next");
    }
    assert!(
        collect_invocations(
            "local x = 42",
            &offered(),
            SCHEMA,
            Budget::Steps(0),
            CommandLimits::default()
        )
        .is_err()
    );
    assert_eq!(
        collect_invocations(
            "local x = 42",
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
    let mut note = crate::LuaEvaluator::new();
    assert!(
        note.eval_block(r#"command("view:fit", "")"#, 10_000)
            .is_err()
    );
    assert_eq!(
        note.eval_block("command == nil", 10_000).unwrap().text,
        "true"
    );
    for source in [
        r#"require("sibling")"#,
        r#"load("return 40 + 2")()"#,
        r#"dofile("/etc/passwd")"#,
        r#"io.open("/etc/passwd")"#,
        r#"os.execute("true")"#,
    ] {
        assert!(collect(source).is_err(), "{source}");
    }
}

#[test]
fn a_caught_refusal_still_invalidates_the_batch_and_later_calls_fail() {
    // The caught call returns false and the script carries on, yet the batch
    // is refused, and a later valid call is refused with the first reason.
    let source = r#"
        local ok, message = pcall(command, "host:unknown", "")
        assert(not ok)
        local again = pcall(command, "view:fit", "")
        command("view:fit", "after")
    "#;
    let mut evaluator = LuaCommandEvaluator::new(offered(), SCHEMA, CommandLimits::default());
    let error = evaluator.eval_block(source, 10_000).unwrap_err();
    assert!(error.contains("not offered"), "{error}");
    // A coroutine catches the error as well.
    let source = r#"coroutine.resume(coroutine.create(command), "host:unknown", "")"#;
    let error = collect(source).unwrap_err();
    assert!(error.contains("not offered"), "{error}");
}

#[test]
fn only_a_string_id_and_a_string_of_arguments_are_collected() {
    for source in [
        r#"command("view:fit", 5)"#,
        r#"command("view:fit", nil)"#,
        r#"command("view:fit", {})"#,
        r#"command("view:fit")"#,
        r#"command("view:fit", "", "extra")"#,
        r#"command(1, "")"#,
        r#"pcall(command, "view:fit", {})"#,
    ] {
        let error = collect(source).unwrap_err();
        assert!(
            error.contains("id string and an arguments string"),
            "{source}: {error}"
        );
    }
}

#[test]
fn the_string_limit_bounds_what_a_command_script_builds() {
    let source = r#"command("view:fit", string.rep("x", 100000))"#;
    let limits = CommandLimits {
        max_invocations: 4,
        max_argument_bytes: 1 << 20,
    };
    assert!(
        collect_invocations(source, &offered(), SCHEMA, Budget::Steps(100_000), limits).is_err()
    );
    let mut evaluator = LuaCommandEvaluator::new(offered(), SCHEMA, limits);
    assert_eq!(evaluator.max_string_bytes(), DEFAULT_MAX_STRING_BYTES);
    assert!(evaluator.eval_block(source, 100_000).is_err());
    let mut roomy =
        LuaCommandEvaluator::new(offered(), SCHEMA, limits).with_max_string_bytes(1 << 20);
    let out = roomy.eval_block(source, 100_000).unwrap();
    let requests: Vec<Invocation> = serde_json::from_str(&out.text).unwrap();
    assert_eq!(requests[0].arguments.len(), 100_000);
}

#[test]
fn a_zero_budget_means_the_command_default_and_a_runaway_is_refused() {
    let mut evaluator = LuaCommandEvaluator::new(offered(), SCHEMA, CommandLimits::default());
    let out = evaluator
        .eval_block(r#"command("view:fit", "")"#, 0)
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Vec<Invocation>>(&out.text)
            .unwrap()
            .len(),
        1
    );
    let error = evaluator.eval_block("while true do end", 0).unwrap_err();
    assert!(error.contains("budget"), "{error}");
}

#[test]
fn the_language_tag_is_lua_command_and_the_evaluator_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<LuaCommandEvaluator>();
    let evaluator = LuaCommandEvaluator::new(offered(), SCHEMA, CommandLimits::default());
    assert_eq!(evaluator.language(), "lua-command");
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
            language: Some("lua-command eval".into()),
            text: source.into(),
        }],
    }
}

fn registry() -> BlockEvaluators {
    let mut registry = BlockEvaluators::new();
    registry.register(Box::new(LuaCommandEvaluator::new(
        offered(),
        SCHEMA,
        CommandLimits::default(),
    )));
    registry
}

#[test]
fn actual_block_pass_emits_plain_invocation_data_with_generated_provenance() {
    let source = r#"command("view:fit", "\0payload")"#;
    let mut document = document(source);
    document.navigation.block_count = 1;
    let mut registry = registry();
    assert_eq!(registry.languages(), vec!["lua-command"]);
    let mut evaluate = |language: &str, source: &str| registry.evaluate(language, source, 10_000);
    let mut render = |_: &EngineInput| -> Result<EngineDocument, String> {
        panic!("plain JSON must not be nested rendered")
    };
    let policy = EvaluationPolicy::for_own_notes(vec!["lua-command".into()]);
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
        Some("evaluated:lua-command")
    );
}

#[test]
fn denied_fences_remain_source_and_failed_scripts_add_only_an_error_block() {
    for policy in [
        EvaluationPolicy::deny_all(),
        EvaluationPolicy::for_own_notes(vec!["lua".into()]),
    ] {
        let mut document = document(r#"command("view:fit", "")"#);
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
    let mut document = document(r#"command("view:fit", ""); error("stop")"#);
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
        &EvaluationPolicy::for_own_notes(vec!["lua-command".into()]),
    );
    assert_eq!(outcome.failed.len(), 1);
    assert_eq!(outcome.evaluated, 0);
    // The source stays; the only addition is the error block, with no
    // partial request data from the command that ran before the error.
    assert_eq!(document.blocks[..1], before[..]);
    assert!(matches!(
        &document.blocks[1..],
        [Block::Preformatted { text }]
            if text.starts_with("lua-command eval failed\n") && !text.contains("\"id\"")
    ));
    assert!(outcome.provenance.get(0).is_none());
}
