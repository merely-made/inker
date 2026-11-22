// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! script-rhai's value-engine tests, translated to Lua, plus the cases Lua's
//! tables, string limit and shared namespace add.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::*;
use crate::register_libm_maths;

fn engine() -> LuaValueEngine {
    LuaValueEngine::new()
}

fn run(source: &str) -> Result<ScriptValue, ValueError> {
    engine().eval(source, Budget::Steps(100_000))
}

fn float(value: &ScriptValue) -> f64 {
    value.as_float().expect("a number")
}

fn text(value: &str) -> ScriptValue {
    ScriptValue::Str(value.into())
}

fn constant(value: ScriptValue) -> HostFunction {
    Arc::new(move |_: &[ScriptValue]| Ok(value.clone()))
}

#[test]
fn scalars_and_containers_cross_both_ways() {
    let mut e = engine();
    let value = ScriptValue::Map(BTreeMap::from([
        ("flag".to_string(), ScriptValue::Bool(true)),
        ("count".to_string(), ScriptValue::Int(3)),
        ("ratio".to_string(), ScriptValue::Float(0.5)),
        ("name".to_string(), text("node")),
        (
            "ids".to_string(),
            ScriptValue::List(vec![ScriptValue::Int(1), ScriptValue::Int(2)]),
        ),
    ]));
    e.set_global("input", value.clone()).unwrap();
    // The identity: whatever goes in comes back unchanged.
    assert_eq!(e.eval("input", Budget::Steps(1000)), Ok(value));
    // And the script can read into it (Lua counts from 1).
    assert_eq!(
        e.eval("input.count + input.ids[1]", Budget::Steps(1000)),
        Ok(ScriptValue::Int(4))
    );
}

#[test]
fn strings_compare_and_join() {
    assert_eq!(
        run(r#"local a = "no" .. "de"; return a == "node""#),
        Ok(ScriptValue::Bool(true))
    );
    // D7: text and numbers join with `..` and `tostring`.
    assert_eq!(
        run(r#"return "n=" .. 3 .. " " .. tostring(true)"#),
        Ok(text("n=3 true"))
    );
}

#[test]
fn a_script_returns_a_map_in_key_order() {
    let ScriptValue::Map(entries) = run("{ b = 2, a = 1 }").unwrap() else {
        panic!("a map")
    };
    assert_eq!(entries.keys().collect::<Vec<_>>(), ["a", "b"]);
}

#[test]
fn engine_built_in_maths_is_not_loaded() {
    // Positive control: the same expression resolves once the host
    // registers it, so the refusals below are a measurement.
    let mut with = engine();
    register_libm_maths(&mut with).unwrap();
    assert!(with.eval("sin(1.0)", Budget::Steps(1000)).is_ok());
    // And a loaded library is seen: `string` is there.
    assert_eq!(run("type(string.len)"), Ok(text("function")));

    for source in [
        "sin(1.0)",
        "math.sin(1.0)",
        "math.exp(1.0)",
        "math.sqrt(4.0)",
        "math.pi",
        "math.floor(1.5)",
    ] {
        assert!(
            matches!(run(source), Err(ValueError::Script(_))),
            "`{source}` reached engine maths: {:?}",
            run(source)
        );
    }
    assert_eq!(run("type(math)"), Ok(text("nil")));
}

#[test]
fn libm_maths_matches_libm_bit_for_bit() {
    let mut e = engine();
    register_libm_maths(&mut e).unwrap();
    e.set_global("x", ScriptValue::Float(0.7)).unwrap();
    let cases: [(&str, f64); 9] = [
        ("sin(x)", libm::sin(0.7)),
        ("cos(x)", libm::cos(0.7)),
        ("tan(x)", libm::tan(0.7)),
        ("exp(x)", libm::exp(0.7)),
        ("ln(x)", libm::log(0.7)),
        ("sqrt(x)", libm::sqrt(0.7)),
        ("atan2(x, 2)", libm::atan2(0.7, 2.0)),
        ("pow(x, 1.5)", libm::pow(0.7, 1.5)),
        ("floor(x * 10)", libm::floor(7.000000000000001)),
    ];
    for (source, want) in cases {
        let got = float(&e.eval(source, Budget::Steps(1000)).unwrap());
        assert_eq!(got.to_bits(), want.to_bits(), "{source}");
    }
    assert_eq!(
        e.eval("min(abs(-3), max(5, 4))", Budget::Steps(1000)),
        Ok(ScriptValue::Float(3.0))
    );
}

#[test]
fn a_host_function_sees_neutral_values_and_may_refuse() {
    let mut e = engine();
    e.register(
        "kind",
        1,
        Arc::new(|args: &[ScriptValue]| Ok(ScriptValue::Str(args[0].kind().into()))),
    )
    .unwrap();
    e.register(
        "refuse",
        0,
        Arc::new(|_: &[ScriptValue]| Err("not for you".to_string())),
    )
    .unwrap();
    // An empty table is a list; a table of strings is a map; nil is unit.
    assert_eq!(
        e.eval(
            r#"{kind(1), kind(1.5), kind("s"), kind({}), kind({ a = 1 }), kind(nil)}"#,
            Budget::Steps(1000)
        ),
        Ok(ScriptValue::List(
            ["int", "float", "string", "list", "map", "unit"]
                .map(text)
                .to_vec()
        ))
    );
    assert_eq!(
        e.eval("refuse()", Budget::Steps(1000)),
        Err(ValueError::Host("not for you".into()))
    );
    // The host's refusal is still the host's after a script catches and rethrows.
    assert_eq!(
        e.eval(
            "local ok, err = pcall(refuse) error(err, 0)",
            Budget::Steps(1000)
        ),
        Err(ValueError::Host("not for you".into()))
    );
    assert_eq!(
        e.eval("local ok = pcall(refuse) return ok", Budget::Steps(1000)),
        Ok(ScriptValue::Bool(false))
    );
}

/// Rhai's test guards a method-style call (`a.first()`) that hands the
/// callee the caller's own variable. Lua has no such call on a table, so the
/// hazard does not exist; what carries over is that a call copies.
#[test]
fn a_host_call_leaves_the_callers_table_alone() {
    let mut e = engine();
    e.register(
        "first",
        1,
        Arc::new(|args: &[ScriptValue]| match &args[0] {
            ScriptValue::List(items) => Ok(items[0].clone()),
            other => Err(other.kind().to_string()),
        }),
    )
    .unwrap();
    assert_eq!(
        e.eval(
            "local a = {7, 8} local f = first(a) return {f, #a}",
            Budget::Steps(1000)
        ),
        Ok(ScriptValue::List(vec![
            ScriptValue::Int(7),
            ScriptValue::Int(2)
        ]))
    );
}

#[test]
fn a_call_with_the_wrong_number_of_arguments_is_a_script_error() {
    let mut e = engine();
    e.register("two", 2, constant(ScriptValue::Unit)).unwrap();
    for source in ["two()", "two(1)", "two(1, 2, 3)"] {
        assert!(
            matches!(
                e.eval(source, Budget::Steps(1000)),
                Err(ValueError::Script(_))
            ),
            "{source}"
        );
    }
    assert_eq!(
        e.eval("two(1, 2)", Budget::Steps(1000)),
        Ok(ScriptValue::Unit)
    );
}

#[test]
fn a_runaway_is_a_budget_error_and_the_engine_survives() {
    let mut e = engine();
    assert_eq!(
        e.eval(
            "local i = 0 while true do i = i + 1 end",
            Budget::Steps(10_000)
        ),
        Err(ValueError::Budget)
    );
    assert_eq!(
        e.eval("1 + 1", Budget::Steps(1000)),
        Ok(ScriptValue::Int(2))
    );
}

#[test]
fn an_empty_budget_runs_nothing_and_unbounded_runs_to_the_end() {
    let mut e = engine();
    assert_eq!(e.eval("1", Budget::Steps(0)), Err(ValueError::Budget));
    assert_eq!(
        e.eval(
            "local s = 0 for i = 1, 50000 do s = s + 1 end return s",
            Budget::Unbounded
        ),
        Ok(ScriptValue::Int(50_000))
    );
}

#[test]
fn each_eval_gets_a_fresh_state_over_the_globals() {
    let mut e = engine();
    e.set_global("g", ScriptValue::Int(1)).unwrap();
    assert_eq!(
        e.eval(
            "local x = 5; g = 9; leak = 1; return x + g",
            Budget::Steps(1000)
        ),
        Ok(ScriptValue::Int(14))
    );
    // Neither the local, the new global nor the assignment survives. An
    // unknown name is nil in Lua, not an error (D6).
    assert_eq!(e.eval("x", Budget::Steps(1000)), Ok(ScriptValue::Unit));
    assert_eq!(e.eval("leak", Budget::Steps(1000)), Ok(ScriptValue::Unit));
    assert_eq!(e.eval("g", Budget::Steps(1000)), Ok(ScriptValue::Int(1)));
    // Setting a global again replaces it.
    e.set_global("g", ScriptValue::Int(2)).unwrap();
    assert_eq!(e.eval("g", Budget::Steps(1000)), Ok(ScriptValue::Int(2)));
}

#[test]
fn a_name_registered_twice_is_refused() {
    let mut e = engine();
    let f = constant(ScriptValue::Unit);
    e.register("once", 0, f.clone()).unwrap();
    assert!(matches!(
        e.register("once", 0, f.clone()),
        Err(ValueError::Host(_))
    ));
    // Even at another arity: the name is the unit.
    assert!(matches!(
        e.register("once", 2, f.clone()),
        Err(ValueError::Host(_))
    ));
    let mut maths = engine();
    register_libm_maths(&mut maths).unwrap();
    assert!(matches!(
        register_libm_maths(&mut maths),
        Err(ValueError::Host(_))
    ));
    // A global and a function share Lua's one namespace, so they collide.
    e.set_global("data", ScriptValue::Int(1)).unwrap();
    assert!(matches!(e.register("data", 0, f), Err(ValueError::Host(_))));
    assert!(matches!(
        e.set_global("once", ScriptValue::Int(1)),
        Err(ValueError::Host(_))
    ));
}

#[test]
fn names_must_be_identifiers() {
    let mut e = engine();
    for bad in ["", "1a", "a b", "a-b", "é", "end", "nil", "function"] {
        assert!(e.set_global(bad, ScriptValue::Unit).is_err(), "{bad:?}");
        assert!(
            e.register(bad, 0, constant(ScriptValue::Unit)).is_err(),
            "{bad:?}"
        );
    }
}

#[test]
fn failures_are_told_apart() {
    assert!(matches!(
        run("this is not lua @@@"),
        Err(ValueError::Script(_))
    ));
    assert!(matches!(run(r#"error("boom")"#), Err(ValueError::Script(m)) if m.contains("boom")));
    // A value the boundary cannot carry: a function, a table with a hole.
    assert!(matches!(run("function() end"), Err(ValueError::Value(_))));
    assert!(matches!(run("{1, nil, 3}"), Err(ValueError::Value(_))));
    // D6: an unguarded comparison with a missing fact is a rule error.
    assert!(matches!(
        run("local t = {} return t.age > 2.0"),
        Err(ValueError::Script(_))
    ));
}

#[test]
fn nesting_past_the_cap_is_refused_in_both_directions() {
    let mut deep = ScriptValue::Int(0);
    for _ in 0..100 {
        deep = ScriptValue::List(vec![deep]);
    }
    let mut e = engine();
    assert!(matches!(
        e.set_global("deep", deep),
        Err(ValueError::Value(_))
    ));
    assert!(matches!(
        run("local a = {} for i = 1, 100 do a = {a} end return a"),
        Err(ValueError::Value(_))
    ));
}

#[test]
fn non_finite_floats_cross_unchanged() {
    let ScriptValue::List(items) = run("{1.0 / 0.0, -1.0 / 0.0, 0.0 / 0.0}").unwrap() else {
        panic!("a list")
    };
    assert_eq!(float(&items[0]), f64::INFINITY);
    assert_eq!(float(&items[1]), f64::NEG_INFINITY);
    assert!(float(&items[2]).is_nan());
}

#[test]
fn the_sandbox_matches_the_block_evaluators() {
    // Rhai refuses `import` and `eval`; piccolo ships no `require`, `load` or
    // `dofile`, and the engine loads neither `io` nor `os`.
    assert!(run(r#"require("x")"#).is_err());
    assert!(run(r#"load("return 1")()"#).is_err());
    assert!(run(r#"io.open("x")"#).is_err());
    assert!(run("print(1)").is_err());
    assert_eq!(run("type(os)"), Ok(text("nil")));
    assert_eq!(run("type(coroutine)"), Ok(text("nil")));
    // Recursion is bounded by the budget (Lua has no call-depth cap here).
    assert!(
        run("local function f(n) return 1 + f(n + 1) end return f(0)").is_err(),
        "recursion is capped"
    );
}

#[test]
fn it_is_usable_behind_a_trait_object() {
    let mut e: Box<dyn ValueEngine> = Box::new(engine());
    e.set_global("a", ScriptValue::Int(20)).unwrap();
    assert_eq!(
        e.eval("a + 22", Budget::Steps(1000)),
        Ok(ScriptValue::Int(42))
    );
}

#[test]
fn the_engine_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<LuaValueEngine>();
}

#[test]
fn tables_map_to_lists_and_maps_by_their_keys() {
    assert_eq!(run("{}"), Ok(ScriptValue::List(vec![])));
    assert_eq!(
        run("{10, 20, 30}"),
        Ok(ScriptValue::List(
            [10, 20, 30].map(ScriptValue::Int).to_vec()
        ))
    );
    // Positions set out of order still make a list.
    assert_eq!(
        run("local t = {} t[2] = 'b' t[1] = 'a' return t"),
        Ok(ScriptValue::List(vec![text("a"), text("b")]))
    );
    for refused in [
        "{[1] = 1, [3] = 3}",
        "{[0] = 1}",
        "{[2] = 1}",
        "{1, a = 2}",
        "{[1.5] = 1}",
        "{[true] = 1}",
    ] {
        assert!(
            matches!(run(refused), Err(ValueError::Value(_))),
            "{refused}: {:?}",
            run(refused)
        );
    }
}

#[test]
fn a_unit_has_no_place_in_a_list_but_leaves_a_map_key_out() {
    let mut e = engine();
    assert!(matches!(
        e.set_global(
            "l",
            ScriptValue::List(vec![ScriptValue::Int(1), ScriptValue::Unit])
        ),
        Err(ValueError::Value(_))
    ));
    e.set_global(
        "m",
        ScriptValue::Map(BTreeMap::from([
            ("a".to_string(), ScriptValue::Int(1)),
            ("gone".to_string(), ScriptValue::Unit),
        ])),
    )
    .unwrap();
    assert_eq!(
        e.eval("{has = m.a, gone = m.gone == nil}", Budget::Steps(1000)),
        Ok(ScriptValue::Map(BTreeMap::from([
            ("gone".to_string(), ScriptValue::Bool(true)),
            ("has".to_string(), ScriptValue::Int(1)),
        ])))
    );
}

#[test]
fn a_table_holding_itself_is_refused_not_looped() {
    assert!(matches!(
        run("local t = {} t[1] = t return t"),
        Err(ValueError::Value(_))
    ));
    // A shared table doubled sixty times is a small graph and a huge tree.
    assert!(matches!(
        run("local t = {1} for i = 1, 60 do t = {t, t} end return t"),
        Err(ValueError::Value(_))
    ));
}

#[test]
fn a_string_that_is_not_utf8_is_refused() {
    assert!(matches!(
        run(r"return string.char(255)"),
        Err(ValueError::Value(_))
    ));
}

#[test]
fn the_string_limit_is_the_hosts_and_has_a_default() {
    assert_eq!(engine().max_string_bytes(), DEFAULT_MAX_STRING_BYTES);
    let doubling = "local s = 'x' for i = 1, 30 do s = s .. s end return #s";
    let mut stock = engine();
    assert!(matches!(
        stock.eval(doubling, Budget::Steps(1_000_000)),
        Err(ValueError::Script(_))
    ));
    let mut small = LuaValueEngine::new().with_max_string_bytes(16);
    assert_eq!(small.max_string_bytes(), 16);
    assert_eq!(
        small.eval("#('x'):rep(16)", Budget::Steps(1000)),
        Ok(ScriptValue::Int(16))
    );
    assert!(matches!(
        small.eval("#('x'):rep(17)", Budget::Steps(1000)),
        Err(ValueError::Script(_))
    ));
}

#[test]
fn a_host_function_returning_a_value_the_boundary_refuses_is_a_value_error() {
    let mut e = engine();
    e.register(
        "bad",
        0,
        constant(ScriptValue::List(vec![ScriptValue::Unit])),
    )
    .unwrap();
    assert!(matches!(
        e.eval("bad()", Budget::Steps(1000)),
        Err(ValueError::Value(_))
    ));
}
