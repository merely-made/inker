// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Rhai behind genet's value-level script contract.
//!
//! [`RhaiValueEngine`] implements [`ValueEngine`] (Scenograph editor plan,
//! SE39): neutral [`ScriptValue`]s in and out, host functions over them, and
//! evaluation under a [`Budget`]. It is a raw rhai engine with only the
//! language, iterator, array and map packages, so rhai's float-maths package
//! is *not* loaded (SE40): a rule reaches `sin` or `exp` only through a host
//! function, and [`register_libm_maths`] supplies those on `libm`, which gives
//! the same bits natively and on wasm32.
//!
//! Each [`eval`](ValueEngine::eval) runs in a fresh scope seeded from the
//! globals (SE95), so a rule cannot leak a binding into the next. A name
//! registered twice is refused (SE96).

use std::any::TypeId;
use std::collections::BTreeSet;
use std::sync::Arc;

use rhai::packages::{BasicArrayPackage, BasicMapPackage, CorePackage, Package};
use rhai::{Array, Dynamic, Engine, EvalAltResult, Map, NativeCallContext, Position, Scope};
use script_engine_api::{Budget, HostFunction, ScriptValue, ValueEngine, ValueError};

/// Nesting deeper than this does not cross the boundary, either way.
const MAX_DEPTH: usize = 64;

/// A rhai-backed [`ValueEngine`] with no engine-built-in maths.
pub struct RhaiValueEngine {
    engine: Engine,
    globals: Scope<'static>,
    registered: BTreeSet<String>,
}

impl RhaiValueEngine {
    pub fn new() -> Self {
        let mut engine = Engine::new_raw();
        CorePackage::new().register_into_engine(&mut engine);
        BasicArrayPackage::new().register_into_engine(&mut engine);
        BasicMapPackage::new().register_into_engine(&mut engine);
        crate::sandbox(&mut engine);
        Self {
            engine,
            globals: Scope::new(),
            registered: BTreeSet::new(),
        }
    }
}

impl Default for RhaiValueEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn identifier(name: &str) -> Result<(), ValueError> {
    let mut chars = name.chars();
    let head = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    if head && chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        Ok(())
    } else {
        Err(ValueError::Script(format!("`{name}` is not an identifier")))
    }
}

fn to_dynamic(value: ScriptValue, depth: usize) -> Result<Dynamic, ValueError> {
    if depth > MAX_DEPTH {
        return Err(ValueError::Value("nested too deeply".into()));
    }
    Ok(match value {
        ScriptValue::Unit => Dynamic::UNIT,
        ScriptValue::Bool(value) => value.into(),
        ScriptValue::Int(value) => value.into(),
        ScriptValue::Float(value) => value.into(),
        ScriptValue::Str(value) => value.into(),
        ScriptValue::List(items) => items
            .into_iter()
            .map(|item| to_dynamic(item, depth + 1))
            .collect::<Result<Array, _>>()?
            .into(),
        ScriptValue::Map(entries) => entries
            .into_iter()
            .map(|(key, item)| Ok((key.into(), to_dynamic(item, depth + 1)?)))
            .collect::<Result<Map, ValueError>>()?
            .into(),
    })
}

fn from_dynamic(value: &Dynamic, depth: usize) -> Result<ScriptValue, ValueError> {
    if depth > MAX_DEPTH {
        return Err(ValueError::Value("nested too deeply".into()));
    }
    if value.is_unit() {
        Ok(ScriptValue::Unit)
    } else if value.is_bool() {
        Ok(ScriptValue::Bool(value.as_bool().unwrap_or_default()))
    } else if value.is_int() {
        Ok(ScriptValue::Int(value.as_int().unwrap_or_default()))
    } else if value.is_float() {
        Ok(ScriptValue::Float(value.as_float().unwrap_or_default()))
    } else if value.is_string() {
        Ok(ScriptValue::Str(
            value.clone().into_string().unwrap_or_default(),
        ))
    } else if let Some(items) = value.read_lock::<Array>() {
        items
            .iter()
            .map(|item| from_dynamic(item, depth + 1))
            .collect::<Result<_, _>>()
            .map(ScriptValue::List)
    } else if let Some(entries) = value.read_lock::<Map>() {
        entries
            .iter()
            .map(|(key, item)| Ok((key.to_string(), from_dynamic(item, depth + 1)?)))
            .collect::<Result<_, ValueError>>()
            .map(ScriptValue::Map)
    } else {
        Err(ValueError::Value(format!(
            "rhai value of type {}",
            value.type_name()
        )))
    }
}

/// A failure carried through rhai as a runtime error, so `eval` can hand back
/// the host's own [`ValueError`] rather than rhai's wording of it.
fn carried(error: ValueError) -> Box<EvalAltResult> {
    EvalAltResult::ErrorRuntime(Dynamic::from(error), Position::NONE).into()
}

impl ValueEngine for RhaiValueEngine {
    fn set_global(&mut self, name: &str, value: ScriptValue) -> Result<(), ValueError> {
        identifier(name)?;
        self.globals
            .set_or_push(name.to_string(), to_dynamic(value, 0)?);
        Ok(())
    }

    fn register(
        &mut self,
        name: &str,
        arity: usize,
        function: HostFunction,
    ) -> Result<(), ValueError> {
        identifier(name)?;
        if !self.registered.insert(name.to_string()) {
            return Err(ValueError::Host(format!("`{name}` is already registered")));
        }
        let call = move |_: NativeCallContext,
                         args: &mut [&mut Dynamic]|
              -> Result<Dynamic, Box<EvalAltResult>> {
            // Copied, never taken: a method-style call passes the caller's own
            // variable as the first argument.
            let values = args
                .iter()
                .map(|arg| from_dynamic(arg, 0))
                .collect::<Result<Vec<_>, _>>()
                .map_err(carried)?;
            let returned = function(&values).map_err(|m| carried(ValueError::Host(m)))?;
            to_dynamic(returned, 0).map_err(carried)
        };
        self.engine
            .register_raw_fn(name, vec![TypeId::of::<Dynamic>(); arity], call);
        Ok(())
    }

    fn eval(&mut self, source: &str, budget: Budget) -> Result<ScriptValue, ValueError> {
        match budget {
            Budget::Unbounded => {
                self.engine.set_max_operations(0);
            },
            // Rhai reads 0 as "unlimited"; an empty budget runs nothing.
            Budget::Steps(0) => return Err(ValueError::Budget),
            Budget::Steps(steps) => {
                self.engine.set_max_operations(steps);
            },
        }
        let mut scope = self.globals.clone();
        let value = self
            .engine
            .eval_with_scope::<Dynamic>(&mut scope, source)
            .map_err(|error| match *error {
                EvalAltResult::ErrorTooManyOperations(_) => ValueError::Budget,
                EvalAltResult::ErrorRuntime(ref payload, _) if payload.is::<ValueError>() => {
                    payload.clone().cast::<ValueError>()
                },
                other => ValueError::Script(other.to_string()),
            })?;
        from_dynamic(&value, 0)
    }
}

/// Register the rule maths on `libm` (SE40, SE94): `exp`, `ln`, `sin`, `cos`,
/// `tan`, `atan2`, `sqrt`, `pow`, `floor`, `abs`, `min` and `max`. Integers
/// are widened; every result is a float.
pub fn register_libm_maths(engine: &mut dyn ValueEngine) -> Result<(), ValueError> {
    fn unary(name: &'static str, f: fn(f64) -> f64) -> HostFunction {
        Arc::new(move |args: &[ScriptValue]| match args[0].as_float() {
            Some(x) => Ok(ScriptValue::Float(f(x))),
            None => Err(format!("{name} wants a number, got {}", args[0].kind())),
        })
    }
    fn binary(name: &'static str, f: fn(f64, f64) -> f64) -> HostFunction {
        Arc::new(
            move |args: &[ScriptValue]| match (args[0].as_float(), args[1].as_float()) {
                (Some(x), Some(y)) => Ok(ScriptValue::Float(f(x, y))),
                _ => Err(format!(
                    "{name} wants numbers, got {} and {}",
                    args[0].kind(),
                    args[1].kind()
                )),
            },
        )
    }
    let unaries: [(&'static str, fn(f64) -> f64); 8] = [
        ("exp", libm::exp),
        ("ln", libm::log),
        ("sin", libm::sin),
        ("cos", libm::cos),
        ("tan", libm::tan),
        ("sqrt", libm::sqrt),
        ("floor", libm::floor),
        ("abs", libm::fabs),
    ];
    for (name, f) in unaries {
        engine.register(name, 1, unary(name, f))?;
    }
    let binaries: [(&'static str, fn(f64, f64) -> f64); 4] = [
        ("atan2", libm::atan2),
        ("pow", libm::pow),
        ("min", libm::fmin),
        ("max", libm::fmax),
    ];
    for (name, f) in binaries {
        engine.register(name, 2, binary(name, f))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use super::*;

    fn engine() -> RhaiValueEngine {
        RhaiValueEngine::new()
    }

    fn run(source: &str) -> Result<ScriptValue, ValueError> {
        engine().eval(source, Budget::Steps(100_000))
    }

    fn float(value: &ScriptValue) -> f64 {
        value.as_float().expect("a number")
    }

    #[test]
    fn scalars_and_containers_cross_both_ways() {
        let mut e = engine();
        let value = ScriptValue::Map(BTreeMap::from([
            ("flag".to_string(), ScriptValue::Bool(true)),
            ("count".to_string(), ScriptValue::Int(3)),
            ("ratio".to_string(), ScriptValue::Float(0.5)),
            ("name".to_string(), ScriptValue::Str("node".into())),
            (
                "ids".to_string(),
                ScriptValue::List(vec![ScriptValue::Int(1), ScriptValue::Unit]),
            ),
        ]));
        e.set_global("input", value.clone()).unwrap();
        // The identity: whatever goes in comes back unchanged.
        assert_eq!(e.eval("input", Budget::Steps(1000)), Ok(value));
        // And the script can read into it.
        assert_eq!(
            e.eval("input.count + input.ids[0]", Budget::Steps(1000)),
            Ok(ScriptValue::Int(4))
        );
    }

    #[test]
    fn strings_compare_and_join() {
        assert_eq!(
            run(r#"let a = "no" + "de"; a == "node""#),
            Ok(ScriptValue::Bool(true))
        );
    }

    #[test]
    fn a_script_returns_a_map_in_key_order() {
        let ScriptValue::Map(entries) = run("#{ b: 2, a: 1 }").unwrap() else {
            panic!("a map")
        };
        assert_eq!(entries.keys().collect::<Vec<_>>(), ["a", "b"]);
    }

    #[test]
    fn engine_built_in_maths_is_not_loaded() {
        // Positive control: the same expression resolves once the host
        // registers it, so the refusal below is a measurement.
        let mut with = engine();
        register_libm_maths(&mut with).unwrap();
        assert!(with.eval("sin(1.0)", Budget::Steps(1000)).is_ok());

        for source in [
            "sin(1.0)",
            "1.0.sin()",
            "exp(1.0)",
            "sqrt(4.0)",
            "PI",
            "floor(1.5)",
        ] {
            assert!(
                matches!(run(source), Err(ValueError::Script(_))),
                "`{source}` reached engine maths: {:?}",
                run(source)
            );
        }
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
        assert_eq!(
            e.eval(
                r#"[kind(1), kind(1.5), kind("s"), kind([]), kind(#{}), kind(())]"#,
                Budget::Steps(1000)
            ),
            Ok(ScriptValue::List(
                ["int", "float", "string", "list", "map", "unit"]
                    .map(|k| ScriptValue::Str(k.into()))
                    .to_vec()
            ))
        );
        assert_eq!(
            e.eval("refuse()", Budget::Steps(1000)),
            Err(ValueError::Host("not for you".into()))
        );
    }

    #[test]
    fn a_method_style_call_leaves_the_callers_variable_alone() {
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
                "let a = [7, 8]; let f = a.first(); [f, a.len()]",
                Budget::Steps(1000)
            ),
            Ok(ScriptValue::List(vec![
                ScriptValue::Int(7),
                ScriptValue::Int(2)
            ]))
        );
    }

    #[test]
    fn a_runaway_is_a_budget_error_and_the_engine_survives() {
        let mut e = engine();
        assert_eq!(
            e.eval("let i = 0; loop { i += 1; }", Budget::Steps(10_000)),
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
                "let s = 0; for i in 0..50000 { s += 1; } s",
                Budget::Unbounded
            ),
            Ok(ScriptValue::Int(50_000))
        );
    }

    #[test]
    fn each_eval_gets_a_fresh_scope_over_the_globals() {
        let mut e = engine();
        e.set_global("g", ScriptValue::Int(1)).unwrap();
        assert_eq!(
            e.eval("let x = 5; g = 9; x + g", Budget::Steps(1000)),
            Ok(ScriptValue::Int(14))
        );
        // Neither the local nor the assignment to the global survives.
        assert!(matches!(
            e.eval("x", Budget::Steps(1000)),
            Err(ValueError::Script(_))
        ));
        assert_eq!(e.eval("g", Budget::Steps(1000)), Ok(ScriptValue::Int(1)));
        // Setting a global again replaces it.
        e.set_global("g", ScriptValue::Int(2)).unwrap();
        assert_eq!(e.eval("g", Budget::Steps(1000)), Ok(ScriptValue::Int(2)));
    }

    #[test]
    fn a_name_registered_twice_is_refused() {
        let mut e = engine();
        let f: HostFunction = Arc::new(|_: &[ScriptValue]| Ok(ScriptValue::Unit));
        e.register("once", 0, f.clone()).unwrap();
        assert!(matches!(
            e.register("once", 0, f.clone()),
            Err(ValueError::Host(_))
        ));
        // Even at another arity: the name is the unit.
        assert!(matches!(e.register("once", 2, f), Err(ValueError::Host(_))));
        let mut maths = engine();
        register_libm_maths(&mut maths).unwrap();
        assert!(matches!(
            register_libm_maths(&mut maths),
            Err(ValueError::Host(_))
        ));
    }

    #[test]
    fn names_must_be_identifiers() {
        let mut e = engine();
        for bad in ["", "1a", "a b", "a-b", "é"] {
            assert!(e.set_global(bad, ScriptValue::Unit).is_err(), "{bad:?}");
            assert!(
                e.register(bad, 0, Arc::new(|_: &[ScriptValue]| Ok(ScriptValue::Unit)))
                    .is_err(),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn failures_are_told_apart() {
        assert!(matches!(
            run("this is not rhai @@@"),
            Err(ValueError::Script(_))
        ));
        assert!(matches!(run(r#"throw "boom""#), Err(ValueError::Script(m)) if m.contains("boom")));
        // A value the boundary cannot carry: a character, a function pointer.
        assert!(matches!(run("'a'"), Err(ValueError::Value(_))));
        assert!(matches!(
            run(r#"Fn("run")"#),
            Err(ValueError::Value(_)) | Err(ValueError::Script(_))
        ));
    }

    #[test]
    fn nesting_past_the_cap_is_refused_in_both_directions() {
        let mut deep = ScriptValue::Unit;
        for _ in 0..100 {
            deep = ScriptValue::List(vec![deep]);
        }
        let mut e = engine();
        assert!(matches!(
            e.set_global("deep", deep),
            Err(ValueError::Value(_))
        ));
        assert!(matches!(
            run("let a = []; for i in 0..100 { a = [a]; } a"),
            Err(ValueError::Value(_))
        ));
    }

    #[test]
    fn non_finite_floats_cross_unchanged() {
        let ScriptValue::List(items) = run("[1.0 / 0.0, -1.0 / 0.0, 0.0 / 0.0]").unwrap() else {
            panic!("a list")
        };
        assert_eq!(float(&items[0]), f64::INFINITY);
        assert_eq!(float(&items[1]), f64::NEG_INFINITY);
        assert!(float(&items[2]).is_nan());
    }

    #[test]
    fn the_sandbox_matches_the_block_evaluators() {
        assert!(run(r#"import "x" as m; m::f()"#).is_err());
        assert!(run(r#"eval("1")"#).is_err());
        assert!(
            run("fn f(n) { f(n + 1) } f(0)").is_err(),
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
}
