// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! script-rhai's evaluator tests, translated to Lua, plus the fuel and
//! sandbox cases the Lua plan adds.

use super::*;

/// A budget large enough for every ordinary test script.
const ENOUGH: u64 = 1_000_000;
/// A budget a runaway exhausts quickly.
const SMALL: u64 = 10_000;

/// Evaluate `source` in a sandbox and return what it yields as a string, so a
/// test can ask what a global is without going through the output convention.
fn probe(mut lua: Lua, source: &str) -> String {
    let executor = lua
        .try_enter(|ctx| {
            let closure = Closure::load(ctx, None, source.as_bytes())?;
            Ok(ctx.stash(Executor::start(ctx, closure.into(), ())))
        })
        .unwrap();
    drive(&mut lua, &executor, DriveLimits::new(Some(ENOUGH))).unwrap();
    lua.try_enter(|ctx| {
        let value = ctx.fetch(&executor).take_result::<Value>(ctx).unwrap()?;
        Ok(value.display().to_string())
    })
    .unwrap()
}

#[test]
fn language_tag_is_lua() {
    assert_eq!(LuaEvaluator::new().language(), "lua");
}

#[test]
fn the_evaluator_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<LuaEvaluator>();
}

#[test]
fn evaluates_arithmetic_to_plain_text() {
    let mut evaluator = LuaEvaluator::new();
    let out = evaluator
        .eval_block(
            "local s = 0 for i = 1, 10 do s = s + i end return s",
            ENOUGH,
        )
        .unwrap();
    assert_eq!(out.format, "plain");
    assert_eq!(out.text, "55");
}

#[test]
fn a_bare_value_needs_no_return() {
    let mut evaluator = LuaEvaluator::new();
    assert_eq!(evaluator.eval_block("1 + 1", ENOUGH).unwrap().text, "2");
    // A trailing comment, and a value spread over lines, still read as a value.
    assert_eq!(
        evaluator.eval_block("2 * 3 -- six", ENOUGH).unwrap().text,
        "6"
    );
    assert_eq!(evaluator.eval_block("1 +\n2", ENOUGH).unwrap().text, "3");
    // Statements are not a value, and run as written.
    let chunk = "local n = 4\nreturn n * n";
    assert_eq!(evaluator.eval_block(chunk, ENOUGH).unwrap().text, "16");
}

#[test]
fn a_compile_error_names_the_authors_line() {
    // The value reading fails first; the error shown is from the source as
    // written, on its own second line.
    let error = LuaEvaluator::new()
        .eval_block("local n = 1\nlocal = 2", ENOUGH)
        .unwrap_err();
    assert!(error.contains("line 2"), "{error}");
}

#[test]
fn a_gemtext_string_is_detected() {
    let mut evaluator = LuaEvaluator::new();
    let out = evaluator
        .eval_block(
            "return \"## generated\\n=> gemini://x.test/ a link\"",
            ENOUGH,
        )
        .unwrap();
    assert_eq!(out.format, "gemtext");
    assert!(out.text.starts_with("## generated"));
}

#[test]
fn an_explicit_format_table_is_honored() {
    let mut evaluator = LuaEvaluator::new();
    let out = evaluator
        .eval_block(
            r#"return { format = "markdown", text = "**bold**" }"#,
            ENOUGH,
        )
        .unwrap();
    assert_eq!(out.format, "markdown");
    assert_eq!(out.text, "**bold**");
}

#[test]
fn a_chunk_that_returns_nothing_is_empty_plain_text() {
    let out = LuaEvaluator::new()
        .eval_block("local x = 1", ENOUGH)
        .unwrap();
    assert_eq!((out.format.as_str(), out.text.as_str()), ("plain", ""));
}

#[test]
fn a_runaway_is_cut_off_by_fuel() {
    let mut evaluator = LuaEvaluator::new();
    let err = evaluator
        .eval_block("while true do end", SMALL)
        .unwrap_err();
    assert!(
        err.starts_with("didgeridoo refused"),
        "expected a budget refusal, got: {err}"
    );
    // The evaluator is still usable after stopping a runaway.
    let out = evaluator.eval_block("return 1 + 1", ENOUGH).unwrap();
    assert_eq!(out.text, "2");
}

/// `max_ops == 0` lets the backend pick its cap; here that cap is still a cap.
#[test]
fn a_zero_budget_still_bounds_a_runaway() {
    let err = LuaEvaluator::new()
        .eval_block("while true do end", 0)
        .unwrap_err();
    assert!(err.starts_with("didgeridoo refused"), "got: {err}");
}

#[test]
fn a_runaway_recursion_is_refused() {
    let err = LuaEvaluator::new()
        .eval_block("local function f() return f() + 1 end return f()", SMALL)
        .unwrap_err();
    assert!(err.starts_with("didgeridoo refused"), "got: {err}");
}

#[test]
fn a_compile_error_is_reported_not_panicked() {
    let mut evaluator = LuaEvaluator::new();
    assert!(
        evaluator
            .eval_block("this is not valid lua @@@", SMALL)
            .is_err()
    );
}

#[test]
fn a_runtime_error_is_reported() {
    let err = LuaEvaluator::new()
        .eval_block(r#"error("nope")"#, ENOUGH)
        .unwrap_err();
    assert!(err.contains("nope"), "got: {err}");
}

/// A script that reaches for `io` fails. The positive control is a full
/// piccolo state, where the io library's `print` is a function, so the probe
/// can see an io binding when one is loaded.
#[test]
fn the_sandbox_cannot_reach_io() {
    assert_eq!(probe(Lua::full(), "return type(print)"), "function");
    assert_eq!(
        probe(sandbox(DEFAULT_MAX_STRING_BYTES), "return type(print)"),
        "nil"
    );
    assert_eq!(
        probe(sandbox(DEFAULT_MAX_STRING_BYTES), "return type(io)"),
        "nil"
    );

    let mut evaluator = LuaEvaluator::new();
    assert!(
        evaluator
            .eval_block(r#"io.open("/etc/passwd")"#, ENOUGH)
            .is_err()
    );
    assert!(evaluator.eval_block(r#"print("leak")"#, ENOUGH).is_err());
}

/// Rhai's module import has no Lua twin in piccolo: it ships no `require`,
/// `dofile`, `loadfile` or `load`. The control shows the probe does see a
/// library that is loaded (`string`), so the `nil`s are a measurement.
#[test]
fn the_sandbox_has_no_randomness() {
    // Control: the rest of `math` is there.
    assert_eq!(
        probe(sandbox(DEFAULT_MAX_STRING_BYTES), "return type(math.floor)"),
        "function"
    );
    assert_eq!(
        probe(
            sandbox(DEFAULT_MAX_STRING_BYTES),
            "return type(math.random)"
        ),
        "nil"
    );
    assert_eq!(
        probe(
            sandbox(DEFAULT_MAX_STRING_BYTES),
            "return type(math.randomseed)"
        ),
        "nil"
    );
}

#[test]
fn the_sandbox_cannot_load_code_from_disk_or_strings() {
    assert_eq!(
        probe(sandbox(DEFAULT_MAX_STRING_BYTES), "return type(string)"),
        "table"
    );
    for name in [
        "require",
        "dofile",
        "loadfile",
        "load",
        "loadstring",
        "os",
        "debug",
        "package",
    ] {
        let source = format!("return type({name})");
        assert_eq!(
            probe(sandbox(DEFAULT_MAX_STRING_BYTES), &source),
            "nil",
            "{name} is reachable"
        );
    }
    assert!(
        LuaEvaluator::new()
            .eval_block(r#"return require("sibling")"#, ENOUGH)
            .is_err()
    );
}

/// The full polyglot path: a `lua eval` fence, routed through the
/// `BlockEvaluators` registry and inker's evaluate pass, renders inline.
#[test]
fn a_lua_fence_evaluates_through_the_inker_pass() {
    use inker::{
        Block, BlockEvaluators, DocumentProvenance, DocumentTrustState, EngineDocument,
        EngineInput, EvaluationPolicy, InlineSpan, evaluate_blocks,
    };

    let mut registry = BlockEvaluators::new();
    registry.register(Box::new(LuaEvaluator::new()));
    assert_eq!(registry.languages(), vec!["lua"]);

    let mut document = EngineDocument {
        address: "note.knot".into(),
        title: None,
        content_type: "text/x-knot".into(),
        lang: None,
        provenance: DocumentProvenance::default(),
        trust: DocumentTrustState::Unknown,
        diagnostics: Vec::new(),
        navigation: Default::default(),
        blocks: vec![Block::CodeBlock {
            language: Some("lua eval".into()),
            text: "local s = 0 for i = 1, 10 do s = s + i end return \"sum is \" .. s".into(),
        }],
    };

    let mut evaluate = |lang: &str, source: &str| registry.evaluate(lang, source, ENOUGH);
    let mut render =
        |_input: &EngineInput| -> Result<EngineDocument, String> { unreachable!("plain output") };
    let policy = EvaluationPolicy::for_own_notes(vec!["lua".into()]);

    let outcome = evaluate_blocks(&mut document, &mut evaluate, &mut render, &policy);
    assert_eq!(outcome.evaluated, 1);
    assert!(matches!(
        &document.blocks[0],
        Block::Paragraph { spans }
            if spans == &vec![InlineSpan::Text("sum is 55".into())]
    ));
}

#[test]
fn a_fence_cannot_build_a_string_past_the_limit() {
    let mut eval = LuaEvaluator::new();
    let limit = eval.max_string_bytes();
    assert_eq!(limit, DEFAULT_MAX_STRING_BYTES);
    // The control: exactly at the limit it builds.
    let at = eval
        .eval_block(&format!("return #string.rep('a', {limit})"), ENOUGH)
        .unwrap();
    assert_eq!(at.text, limit.to_string());
    for build in [
        format!("string.rep('a', {})", limit + 1),
        format!("string.rep('a', {limit}) .. 'b'"),
        format!("string.format('%s%s', string.rep('a', {limit}), 'b')"),
        format!("table.concat({{string.rep('a', {limit}), 'b'}})"),
        format!(
            "(string.gsub(string.rep('a', {}), 'a', 'bb'))",
            limit / 2 + 1
        ),
    ] {
        let err = eval
            .eval_block(&format!("return #({build})"), ENOUGH)
            .expect_err(&build);
        assert!(
            err.contains("too large") || err.contains("too long"),
            "{build}: {err}"
        );
    }
}

#[test]
fn doubling_a_string_is_refused_long_before_the_budget() {
    // Thirty doublings would be a gibibyte; the limit stops it at the seventeenth.
    let err = LuaEvaluator::new()
        .eval_block(
            "local s = 'a' for _ = 1, 30 do s = s .. s end return #s",
            ENOUGH,
        )
        .unwrap_err();
    assert!(err.contains("too long"), "{err}");
}

#[test]
fn the_host_can_set_its_own_string_limit() {
    let source = format!("return #string.rep('a', {})", DEFAULT_MAX_STRING_BYTES + 1);
    let mut roomy = LuaEvaluator::new().with_max_string_bytes(DEFAULT_MAX_STRING_BYTES * 2);
    assert_eq!(
        roomy.eval_block(&source, ENOUGH).unwrap().text,
        (DEFAULT_MAX_STRING_BYTES + 1).to_string()
    );
    let mut tight = LuaEvaluator::new().with_max_string_bytes(16);
    assert!(
        tight
            .eval_block("return string.rep('a', 17)", ENOUGH)
            .is_err()
    );
    assert_eq!(
        tight
            .eval_block("return string.rep('a', 16)", ENOUGH)
            .unwrap()
            .text,
        "a".repeat(16)
    );
}

#[test]
fn length_is_bytes_and_utf8_len_counts_characters() {
    let out = LuaEvaluator::new()
        .eval_block(
            "local s = 'h\u{e9}llo' return #s .. ' ' .. utf8.len(s)",
            ENOUGH,
        )
        .unwrap();
    assert_eq!(out.text, "6 5");
}

const GROWTH: &str = "local t = {} local i = 0 while true do i = i + 1 t[i] = {} end";
/// Finishes, but holds well over the default 16 MiB on the way.
const BIG_BUT_FINITE: &str = "local t = {} for i = 1, 400000 do t[i] = {} end return #t";
const PLENTY: u64 = 100_000_000;

#[test]
fn a_growing_table_is_refused_for_heap() {
    let mut evaluator = LuaEvaluator::new();
    let err = evaluator.eval_block(GROWTH, PLENTY).unwrap_err();
    assert_eq!(
        err,
        format!(
            "didgeridoo refused: the script used more than {DEFAULT_MAX_HEAP_BYTES} bytes of memory"
        )
    );
    // The evaluator survives the refusal.
    assert_eq!(
        evaluator.eval_block("return 1 + 1", ENOUGH).unwrap().text,
        "2"
    );
}

#[test]
fn a_host_ceiling_replaces_the_default() {
    assert_eq!(LuaEvaluator::new().max_heap_bytes(), DEFAULT_MAX_HEAP_BYTES);
    let err = LuaEvaluator::new()
        .eval_block(BIG_BUT_FINITE, PLENTY)
        .unwrap_err();
    assert!(err.contains("bytes of memory"), "got: {err}");
    let mut roomy = LuaEvaluator::new().with_max_heap_bytes(DEFAULT_MAX_HEAP_BYTES * 16);
    assert_eq!(
        roomy.eval_block(BIG_BUT_FINITE, PLENTY).unwrap().text,
        "400000"
    );
    let err = LuaEvaluator::new()
        .with_max_heap_bytes(1024)
        .eval_block(GROWTH, PLENTY)
        .unwrap_err();
    assert!(err.contains("more than 1024 bytes"), "got: {err}");
}

#[test]
fn drive_reports_fuel_spent_and_distinguishes_its_errors() {
    let mut lua = sandbox(DEFAULT_MAX_STRING_BYTES);
    let start = |lua: &mut Lua, source: &str| {
        lua.try_enter(|ctx| {
            let closure = Closure::load(ctx, None, source.as_bytes())?;
            Ok(ctx.stash(Executor::start(ctx, closure.into(), ())))
        })
        .unwrap()
    };
    let executor = start(
        &mut lua,
        "local n = 0 for i = 1, 100 do n = n + i end return n",
    );
    let spent = drive(&mut lua, &executor, DriveLimits::new(None)).unwrap();
    assert!(spent > 0);
    let executor = start(&mut lua, "while true do end");
    assert_eq!(
        drive(&mut lua, &executor, DriveLimits::new(Some(SMALL))),
        Err(DriveError::OutOfFuel { max_ops: SMALL })
    );
    let executor = start(&mut lua, GROWTH);
    assert_eq!(
        drive(&mut lua, &executor, DriveLimits::new(None)),
        Err(DriveError::OverHeap {
            max_heap_bytes: DEFAULT_MAX_HEAP_BYTES
        })
    );
    // A script's own error is its result, read with `take_result`, not a drive error.
    let executor = start(&mut lua, "error('boom')");
    assert!(drive(&mut lua, &executor, DriveLimits::new(None)).is_ok());
}
