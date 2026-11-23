// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The bit receipt for Lua rules (Lua plan, L2): the rule of script-rhai's
//! `tests/support/rule_bits.rs`, in Lua, with the same inputs, the same hash
//! and the same arithmetic in the same order. Rhai's recorded hash is the
//! target; the element-by-element comparison with the Rhai engine left with
//! script-rhai (L6). The Lua rule uses Lua's own `math` and `^` (D17), which
//! didgeridoo computes with libm.

use script_engine_api::{Budget, ScriptValue, ValueEngine, ValueError};
use script_didge::LuaValueEngine;

const COUNT: usize = 10_000;
/// Recorded by the Rhai engine (`script-rhai/tests/support/rule_bits.rs`, retired
/// in L6) and matched element by element until then.
const EXPECTED_HASH: &str = "028dc38fdcbf650d";
const BUDGET: Budget = Budget::Steps(200_000_000);

const LUA_RULE: &str = r#"
local out = {}
for i = 1, #xs do
    local dx = xs[i] - 12.5
    local dy = ys[i] + 7.25
    local d2 = dx * dx + dy * dy
    local s = math.exp(-d2 / 28800.0) * math.sin(dx * 0.01) * math.cos(dy * 0.01)
        + math.sqrt(d2 + 1.0) ^ 0.5 * 0.01
    out[#out + 1] = s
end
return out
"#;

/// Deterministic node positions in [-500, 500), from a fixed LCG.
fn inputs(n: usize) -> (Vec<f64>, Vec<f64>) {
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
fn fnv(values: &[f64]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for value in values {
        for byte in value.to_bits().to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    hash
}

fn run_on(engine: &mut dyn ValueEngine, rule: &str, n: usize) -> Result<Vec<f64>, ValueError> {
    let (xs, ys) = inputs(n);
    let list =
        |values: Vec<f64>| ScriptValue::List(values.into_iter().map(ScriptValue::Float).collect());
    engine.set_global("xs", list(xs))?;
    engine.set_global("ys", list(ys))?;
    let ScriptValue::List(items) = engine.eval(rule, BUDGET)? else {
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

fn run_lua(n: usize) -> Result<Vec<f64>, ValueError> {
    let mut engine = LuaValueEngine::new();
    run_on(&mut engine, LUA_RULE, n)
}

fn hash_hex(values: &[f64]) -> String {
    format!("{:016x}", fnv(values))
}

#[test]
fn the_rule_hashes_to_the_recorded_bits() {
    assert_eq!(hash_hex(&run_lua(COUNT).unwrap()), EXPECTED_HASH);
}

/// The hash must be able to move: one ulp in one result changes it.
#[test]
fn the_hash_sees_a_single_ulp() {
    let mut values = run_lua(COUNT).unwrap();
    let before = fnv(&values);
    values[1234] = f64::from_bits(values[1234].to_bits() + 1);
    assert_ne!(fnv(&values), before);
}

#[test]
fn the_rule_ran_over_every_position() {
    let values = run_lua(COUNT).unwrap();
    assert_eq!(values.len(), COUNT);
    assert_eq!(inputs(COUNT).0.len(), COUNT);
    assert!(values.iter().all(|v| v.is_finite()));
}
