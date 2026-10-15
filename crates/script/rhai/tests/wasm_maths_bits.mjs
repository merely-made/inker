// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

// SE40's determinism, wasm32 side (Scenograph editor plan, SE97): build
// `examples/maths_bits.rs` for wasm32-unknown-unknown, run it under Node (V8,
// the browser's engine), and compare its hash with `EXPECTED_HASH` in
// `tests/support/rule_bits.rs`, which `tests/maths_bits.rs` holds the native
// build to. Identical hash, identical bits.
//
//   node crates/script/rhai/tests/wasm_maths_bits.mjs
//
// Needs the wasm32-unknown-unknown target and `wasm-bindgen` 0.2.129 on PATH
// (the version the workspace lock pins). No network: cargo runs --offline.

import { execFileSync } from "node:child_process";
import { readFileSync, mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../../../..");
const support = readFileSync(join(here, "support/rule_bits.rs"), "utf8");
const expected = support.match(/EXPECTED_HASH: &str = "([0-9a-f]{16})"/)?.[1];
const count = Number(support.match(/COUNT: usize = ([0-9_]+)/)?.[1].replaceAll("_", ""));
if (!expected || !count) {
  console.error("could not read EXPECTED_HASH and COUNT from tests/support/rule_bits.rs");
  process.exit(2);
}

const run = (command, args, env = {}) =>
  execFileSync(command, args, {
    cwd: root,
    stdio: ["ignore", "inherit", "inherit"],
    env: { ...process.env, ...env },
  });

// getrandom 0.3 (rhai -> ahash) has no wasm32 backend until told which; the
// flag is the one Mere's other web lanes pass.
run(
  "cargo",
  [
    "build", "--locked", "--offline", "-j1",
    "-p", "script-rhai", "--example", "maths_bits", "--features", "wasm-check",
    "--target", "wasm32-unknown-unknown",
  ],
  {
    RUSTFLAGS: '--cfg getrandom_backend="wasm_js"',
    CARGO_PROFILE_DEV_DEBUG: "0",
    CARGO_PROFILE_DEV_OPT_LEVEL: "1",
  },
);

const out = join(root, "target/wasm-maths-bits");
mkdirSync(out, { recursive: true });
run("wasm-bindgen", [
  "--target", "nodejs", "--out-dir", out,
  join(root, "target/wasm32-unknown-unknown/debug/examples/maths_bits.wasm"),
]);

const rule = createRequire(import.meta.url)(join(out, "maths_bits.js"));
const got = rule.rule_hash(count);
// Positive controls: the module really ran the rule (an empty run is the FNV
// offset basis; a shorter run hashes differently), so a match is not an echo.
const empty = rule.rule_hash(0);
const shorter = rule.rule_hash(count - 1);
const controls = empty === "cbf29ce484222325" && shorter !== got && /^[0-9a-f]{16}$/.test(got);
console.log(JSON.stringify({ target: "wasm32-unknown-unknown", count, got, expected, controls }));
if (!controls) {
  console.error("the wasm module did not run the rule as expected");
  process.exit(2);
}
if (got !== expected) {
  console.error(`wasm32 hash ${got} differs from the native record ${expected}`);
  process.exit(1);
}
console.log("wasm32 and native hash to the same bits");
