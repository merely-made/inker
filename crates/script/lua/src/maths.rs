// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Rule maths for a [`ValueEngine`], on `libm`.
//!
//! The same names and arities as `script-rhai`'s `register_libm_maths`, and
//! engine-neutral for the same reason: it only calls [`ValueEngine::register`].
//! It is a provisional copy; where the one function finally lives is open.

use std::sync::Arc;

use script_engine_api::{HostFunction, ScriptValue, ValueEngine, ValueError};

type Unary = (&'static str, fn(f64) -> f64);
type Binary = (&'static str, fn(f64, f64) -> f64);

/// Register the rule maths on `libm`: `exp`, `ln`, `sin`, `cos`, `tan`,
/// `atan2`, `sqrt`, `pow`, `floor`, `abs`, `min` and `max`. Integers are
/// widened; every result is a float.
///
/// # Errors
///
/// Whatever [`ValueEngine::register`] refuses, such as a name already taken.
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
    let unaries: [Unary; 8] = [
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
    let binaries: [Binary; 4] = [
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
