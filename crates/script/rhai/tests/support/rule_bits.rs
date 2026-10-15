// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! One rule, one hash: shared by the native test (`tests/maths_bits.rs`) and
//! the wasm32 example (`examples/maths_bits.rs`), so both compute the same
//! thing from the same source (Scenograph editor plan, SE40, SE97).
//!
//! The rule is the plan's F8 probe: a Gaussian falloff with `exp`, `sin`,
//! `cos`, `sqrt` and a power over generated positions, hashed by FNV-1a over
//! the result's float bits so a one-ulp drift shows. `EXPECTED_HASH` is the
//! value the probe recorded as `028dc38f…` for rhai on `libm`, in both
//! native Windows and the browser's engines.

use script_engine_api::{Budget, ScriptValue, ValueEngine, ValueError};
use script_rhai::{RhaiValueEngine, register_libm_maths};

pub const COUNT: usize = 10_000;
pub const EXPECTED_HASH: &str = "028dc38fdcbf650d";

const RULE: &str = r#"
let out = [];
for i in 0..xs.len() {
    let dx = xs[i] - 12.5;
    let dy = ys[i] + 7.25;
    let d2 = dx * dx + dy * dy;
    let s = exp(-d2 / 28800.0) * sin(dx * 0.01) * cos(dy * 0.01) + pow(sqrt(d2 + 1.0), 0.5) * 0.01;
    out.push(s);
}
out
"#;

/// Deterministic node positions in [-500, 500), from a fixed LCG.
pub fn inputs(n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 11) as f64 / (1u64 << 53) as f64) * 1000.0 - 500.0
    };
    let xs = (0..n).map(|_| next()).collect();
    let ys = (0..n).map(|_| next()).collect();
    (xs, ys)
}

/// FNV-1a over the float bit patterns.
pub fn fnv(values: &[f64]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for value in values {
        for byte in value.to_bits().to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    hash
}

/// Run the rule over `n` generated positions on the value engine.
pub fn run(n: usize) -> Result<Vec<f64>, ValueError> {
    let mut engine = RhaiValueEngine::new();
    register_libm_maths(&mut engine)?;
    let (xs, ys) = inputs(n);
    let list =
        |values: Vec<f64>| ScriptValue::List(values.into_iter().map(ScriptValue::Float).collect());
    engine.set_global("xs", list(xs))?;
    engine.set_global("ys", list(ys))?;
    let ScriptValue::List(items) = engine.eval(RULE, Budget::Steps(200_000_000))? else {
        return Err(ValueError::Value(
            "the rule returned something other than a list".into(),
        ));
    };
    items
        .iter()
        .map(|item| {
            item.as_float()
                .ok_or_else(|| ValueError::Value(item.kind().into()))
        })
        .collect()
}

/// The hash as the sixteen hex digits the plan and the runner compare.
pub fn hash_hex(n: usize) -> Result<String, ValueError> {
    run(n).map(|values| format!("{:016x}", fnv(&values)))
}
