// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! SE40's determinism, native side: rule maths on `libm` through the value
//! engine hashes to the recorded value. The wasm32 side is
//! `tests/wasm_maths_bits.mjs`, which builds `examples/maths_bits.rs` for
//! `wasm32-unknown-unknown`, runs it under Node and compares against the same
//! `EXPECTED_HASH`.

#[path = "support/rule_bits.rs"]
mod rule_bits;

use rule_bits::{COUNT, EXPECTED_HASH, fnv, hash_hex, inputs, run};

#[test]
fn the_rule_hashes_to_the_recorded_bits() {
    assert_eq!(hash_hex(COUNT).unwrap(), EXPECTED_HASH);
}

/// The hash must be able to move: one ulp in one result changes it.
#[test]
fn the_hash_sees_a_single_ulp() {
    let mut values = run(COUNT).unwrap();
    let before = fnv(&values);
    values[1234] = f64::from_bits(values[1234].to_bits() + 1);
    assert_ne!(fnv(&values), before);
}

#[test]
fn the_rule_ran_over_every_position() {
    let values = run(COUNT).unwrap();
    assert_eq!(values.len(), COUNT);
    assert_eq!(inputs(COUNT).0.len(), COUNT);
    assert!(values.iter().all(|v| v.is_finite()));
}
