// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The wasm32 half of SE40's determinism check (SE97). Built as a `cdylib`
//! for `wasm32-unknown-unknown` and run under Node by
//! `tests/wasm_maths_bits.mjs`; natively it builds and does nothing, because
//! `tests/maths_bits.rs` covers the native side.

#[path = "../tests/support/rule_bits.rs"]
#[allow(dead_code)]
mod rule_bits;

use wasm_bindgen::prelude::wasm_bindgen;

/// The hash of the rule over `n` positions, or the error in words.
#[wasm_bindgen]
pub fn rule_hash(n: usize) -> String {
    rule_bits::hash_hex(n).unwrap_or_else(|error| format!("error: {error}"))
}
