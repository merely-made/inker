// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Lua backend for the knot block-evaluator lane.
//!
//! [`LuaEvaluator`] implements inker's thin [`BlockEvaluator`] slice over
//! didgeridoo, the piccolo fork: evaluate a `lua eval` fence to text, bounded
//! by the VM's own [`Fuel`] so a runaway is refused rather than hanging the
//! render. It replaced `script-rhai`'s `RhaiEvaluator` (Lua plan, L6).
//!
//! ## Sandbox
//!
//! Each evaluation runs in a fresh Lua state that loads only the libraries
//! named in [`sandbox`]: `base`, `coroutine`, `math`, `string`, `table` and `utf8`.
//! `math` comes without `random`: didgeridoo is built with that feature off,
//! so the same fence gives the same output every time.
//! piccolo's `io` library (which holds `print`) is never loaded, and piccolo
//! has no `load`, `dofile` or `require`, so a fence cannot touch files,
//! stdout or other code.
//!
//! ## Memory
//!
//! Fuel bounds work, not size, so the sandbox also sets didgeridoo's string
//! limit: no string a fence builds may pass
//! [`LuaEvaluator::max_string_bytes`], [`DEFAULT_MAX_STRING_BYTES`] unless the
//! host sets another. The heap is capped too: [`drive`] refuses a script whose
//! state grows past [`LuaEvaluator::max_heap_bytes`],
//! [`DEFAULT_MAX_HEAP_BYTES`] unless the host sets another, even when the
//! operation budget is unbounded.
//!
//! ## Output convention
//!
//! A chunk's first return value becomes the rendered block. Return a string
//! (or any value, stringified) for plain/auto-detected output, or a table
//! `{ format = "gemtext", text = "## hi" }` to declare the format explicitly.
//! A fence may be a bare value (`1 + 1`) or a full chunk that `return`s one;
//! a chunk that returns nothing renders as empty plain text.

use inker::{BlockEvaluator, EvalOutput};
use piccolo::{Closure, Context, Executor, Lua, Value};
pub use piccolo::{DriveError, DriveLimits, drive};

mod command;
mod value;

pub use command::{Budget, CommandLimits, LuaCommandEvaluator, collect_invocations};
pub use value::LuaValueEngine;

/// The budget used when the caller passes `max_ops == 0` ("backend picks").
/// Every evaluation is bounded, so `0` never means unlimited here.
pub const DEFAULT_MAX_OPS: u64 = 1_000_000;

/// The longest string a fence may build unless the host says otherwise: the
/// size Knot allows a note.
pub const DEFAULT_MAX_STRING_BYTES: usize = 64 * 1024;

/// The most memory, in bytes, a script's Lua state may hold unless the host
/// says otherwise.
pub const DEFAULT_MAX_HEAP_BYTES: usize = 16 * 1024 * 1024;

/// The name compiled chunks carry in error messages.
const CHUNK_NAME: &str = "lua eval";

/// Put before the source to read it as a value. On the same line, so line
/// numbers in errors stay the author's.
const RETURN_PREFIX: &str = "return ";

/// A Lua-backed [`BlockEvaluator`] for `lua eval` knot fences.
///
/// It holds no Lua state: each call builds a fresh sandbox, so nothing leaks
/// between fences and the evaluator stays `Send` (a piccolo `Lua` is not).
#[derive(Debug, Clone, Copy)]
pub struct LuaEvaluator {
    max_string_bytes: usize,
    max_heap_bytes: usize,
}

impl Default for LuaEvaluator {
    fn default() -> Self {
        Self {
            max_string_bytes: DEFAULT_MAX_STRING_BYTES,
            max_heap_bytes: DEFAULT_MAX_HEAP_BYTES,
        }
    }
}

impl LuaEvaluator {
    pub fn new() -> Self {
        Self::default()
    }

    /// The longest string, in bytes, a fence may build.
    pub fn max_string_bytes(&self) -> usize {
        self.max_string_bytes
    }

    /// Replace the string limit with the host's own.
    pub fn with_max_string_bytes(mut self, bytes: usize) -> Self {
        self.max_string_bytes = bytes;
        self
    }

    /// The most memory, in bytes, a fence's state may hold.
    pub fn max_heap_bytes(&self) -> usize {
        self.max_heap_bytes
    }

    /// Replace the heap ceiling with the host's own.
    pub fn with_max_heap_bytes(mut self, bytes: usize) -> Self {
        self.max_heap_bytes = bytes;
        self
    }
}

/// A Lua state with exactly the libraries a note fence may reach, and a limit
/// on the strings it may build. Everything else, `io` included, is absent
/// because it is never loaded.
pub fn sandbox(max_string_bytes: usize) -> Lua {
    let mut lua = Lua::empty();
    lua.enter(|ctx| {
        ctx.set_max_string_bytes(max_string_bytes);
        piccolo::stdlib::load_base(ctx);
        piccolo::stdlib::load_coroutine(ctx);
        piccolo::stdlib::load_math(ctx);
        piccolo::stdlib::load_string(ctx);
        piccolo::stdlib::load_table(ctx);
        piccolo::stdlib::load_utf8(ctx);
    });
    lua
}

impl BlockEvaluator for LuaEvaluator {
    fn language(&self) -> &str {
        "lua"
    }

    fn eval_block(&mut self, source: &str, max_ops: u64) -> Result<EvalOutput, String> {
        let budget = if max_ops == 0 {
            DEFAULT_MAX_OPS
        } else {
            max_ops
        };
        let mut lua = sandbox(self.max_string_bytes);
        let executor = lua
            .try_enter(|ctx| {
                // A bare value first, as Lua's own prompt does, so `1 + 1`
                // works; otherwise the source as written, whose compile
                // error is the one reported.
                let as_value = format!("{RETURN_PREFIX}{source}");
                let closure = match Closure::load(ctx, Some(CHUNK_NAME), as_value.as_bytes()) {
                    Ok(closure) => closure,
                    Err(_) => Closure::load(ctx, Some(CHUNK_NAME), source.as_bytes())?,
                };
                Ok(ctx.stash(Executor::start(ctx, closure.into(), ())))
            })
            .map_err(|e| e.to_string())?;
        let limits = DriveLimits {
            max_ops: Some(budget),
            max_heap_bytes: self.max_heap_bytes,
        };
        drive(&mut lua, &executor, limits).map_err(|e| e.to_string())?;
        lua.try_enter(|ctx| {
            let value = ctx.fetch(&executor).take_result::<Value>(ctx);
            // A main chunk that yields leaves the executor suspended, not
            // finished; that is a script error, not a result.
            let value = value.map_err(piccolo::RuntimeError::new)??;
            Ok(to_output(ctx, value))
        })
        .map_err(|e| e.to_string())
    }
}

/// `max_ops` of fuel and the default heap ceiling.
pub fn default_limits(max_ops: Option<u64>) -> DriveLimits {
    DriveLimits {
        max_ops,
        max_heap_bytes: DEFAULT_MAX_HEAP_BYTES,
    }
}

/// Turn a Lua result into an [`EvalOutput`]. A table carrying `format` and
/// `text` declares its rendering explicitly; anything else stringifies and is
/// format-detected.
fn to_output<'gc>(ctx: Context<'gc>, value: Value<'gc>) -> EvalOutput {
    match value {
        Value::Nil => EvalOutput::plain(String::new()),
        Value::Table(table) => {
            let text = match table.get_value(ctx, "text") {
                Value::Nil => String::new(),
                text => text.display().to_string(),
            };
            match table.get_value(ctx, "format") {
                Value::Nil => EvalOutput::detect(text),
                format => EvalOutput {
                    format: format.display().to_string(),
                    text,
                },
            }
        },
        other => EvalOutput::detect(other.display().to_string()),
    }
}

#[cfg(test)]
mod tests;
