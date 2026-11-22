// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! An explicit command-producing script lane, separate from pure note fences.
//!
//! `command(id, arguments)` collects a request from the host's offered set.
//! Both are Lua strings, which are byte strings: `arguments` carries exact
//! bytes, and the host supplies their schema. Anything else is refused.
//! A complete successful evaluation returns data, never execution or
//! permission. Receivers must recheck current declarations, arguments and
//! authority before dispatch, and establish their own terminal
//! [`hest::Outcome`]. This is the Lua counterpart of `script-rhai`'s command
//! lane.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use hest::{Invocation, menu::CommandSet};
use inker::{BlockEvaluator, EvalOutput};
use piccolo::{Callback, CallbackReturn, Closure, Error, Executor, Value};
pub use script_engine_api::Budget;

use crate::{DEFAULT_MAX_STRING_BYTES, run_bounded, sandbox};

/// The fuel `eval_block` gives a command script when the caller passes zero.
/// This lane always chooses a finite default.
pub const COMMAND_DEFAULT_MAX_OPS: u64 = 100_000;

/// The default cap on invocations in one batch.
const DEFAULT_MAX_INVOCATIONS: usize = 256;

/// The default cap on the argument bytes of one batch.
const DEFAULT_MAX_ARGUMENT_BYTES: usize = 64 * 1024;

/// The name compiled chunks carry in error messages.
const CHUNK_NAME: &str = "lua command";

/// Arguments `command` takes: an id and its argument bytes.
const COMMAND_ARITY: usize = 2;

/// Retention limits for one batch, including repeated calls to the same id.
/// These limit collected data, rather than the script engine's total heap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandLimits {
    pub max_invocations: usize,
    pub max_argument_bytes: usize,
}

impl Default for CommandLimits {
    fn default() -> Self {
        Self {
            max_invocations: DEFAULT_MAX_INVOCATIONS,
            max_argument_bytes: DEFAULT_MAX_ARGUMENT_BYTES,
        }
    }
}

#[derive(Default)]
struct Batch {
    invocations: Vec<Invocation>,
    argument_bytes: usize,
    refusal: Option<String>,
}

impl Batch {
    fn collect(
        &mut self,
        offered: &CommandSet,
        schema: &str,
        limits: CommandLimits,
        id: &str,
        arguments: &[u8],
    ) -> Result<(), String> {
        if let Some(reason) = &self.refusal {
            return Err(reason.clone());
        }
        match self.admit(offered, schema, limits, id, arguments) {
            Ok(invocation) => {
                self.argument_bytes = self.argument_bytes.saturating_add(arguments.len());
                self.invocations.push(invocation);
                Ok(())
            },
            Err(reason) => Err(self.refuse(reason)),
        }
    }

    fn admit(
        &self,
        offered: &CommandSet,
        schema: &str,
        limits: CommandLimits,
        id: &str,
        arguments: &[u8],
    ) -> Result<Invocation, String> {
        let declaration = offered
            .get(id)
            .ok_or_else(|| format!("command `{id}` is not offered"))?;
        if let Some(reason) = &declaration.disabled_reason {
            return Err(format!("command `{id}` is disabled: {reason}"));
        }
        if self.invocations.len() >= limits.max_invocations {
            return Err("command batch exceeds its invocation limit".into());
        }
        let remaining = limits
            .max_argument_bytes
            .saturating_sub(self.argument_bytes);
        if arguments.len() > remaining {
            return Err("command batch exceeds its argument byte limit".into());
        }
        Ok(declaration.invocation(schema, arguments.to_vec()))
    }

    /// A script may catch the binding's error with `pcall`. A refused request
    /// still invalidates the whole batch in that case. The first reason stands.
    fn refuse(&mut self, reason: String) -> String {
        self.refusal.get_or_insert_with(|| reason.clone());
        reason
    }
}

/// The error `command` raises into the script.
#[derive(Debug)]
struct CommandRefused(String);

impl fmt::Display for CommandRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CommandRefused {}

/// What a batch needs to admit a call, shared with the `command` function.
struct Shared {
    batch: RefCell<Batch>,
    offered: CommandSet,
    schema: String,
    limits: CommandLimits,
}

/// Collect requests from one fully successful script, in call order, with the
/// default string limit ([`DEFAULT_MAX_STRING_BYTES`]).
///
/// `command("view:fit", "")` supplies the bytes of a Lua string, which may hold
/// any byte. The script cannot choose an effect class or an unoffered id. No
/// batch escapes a parse, runtime, fuel or collection error, including a
/// collection error caught inside the script. Each call has a fresh Lua state
/// and batch. Successful collection has no terminal outcome.
pub fn collect_invocations(
    source: &str,
    offered: &CommandSet,
    schema: &str,
    budget: Budget,
    limits: CommandLimits,
) -> Result<Vec<Invocation>, String> {
    collect_with(
        source,
        offered,
        schema,
        budget,
        limits,
        DEFAULT_MAX_STRING_BYTES,
    )
}

fn collect_with(
    source: &str,
    offered: &CommandSet,
    schema: &str,
    budget: Budget,
    limits: CommandLimits,
    max_string_bytes: usize,
) -> Result<Vec<Invocation>, String> {
    let fuel = match budget {
        // The same bound as a budget of every step a run can count.
        Budget::Unbounded => u64::MAX,
        Budget::Steps(0) => return Err("command script has no operation budget".into()),
        Budget::Steps(steps) => steps,
    };
    let shared = Rc::new(Shared {
        batch: RefCell::new(Batch::default()),
        offered: offered.clone(),
        schema: schema.to_string(),
        limits,
    });
    let mut lua = sandbox(max_string_bytes);
    let executor = lua
        .try_enter(|ctx| {
            let shared = Rc::clone(&shared);
            let command = Callback::from_fn(&ctx, move |_, _, mut stack| {
                let result = match (stack.len(), stack.get(0), stack.get(1)) {
                    (COMMAND_ARITY, Value::String(id), Value::String(arguments)) => {
                        let id = String::from_utf8_lossy(id.as_bytes());
                        shared.batch.borrow_mut().collect(
                            &shared.offered,
                            &shared.schema,
                            shared.limits,
                            &id,
                            arguments.as_bytes(),
                        )
                    },
                    _ => {
                        let reason = "command takes an id string and an arguments string".into();
                        Err(shared.batch.borrow_mut().refuse(reason))
                    },
                };
                result.map_err(|reason| Error::from(CommandRefused(reason)))?;
                stack.clear();
                Ok(CallbackReturn::Return)
            });
            ctx.globals().set(ctx, "command", command)?;
            // The source as written: a command script is statements.
            let closure = Closure::load(ctx, Some(CHUNK_NAME), source.as_bytes())?;
            Ok(ctx.stash(Executor::start(ctx, closure.into(), ())))
        })
        .map_err(|e| e.to_string())?;
    run_bounded(&mut lua, &executor, fuel)?;
    lua.try_enter(|ctx| {
        // A main chunk that yields leaves the executor suspended, not
        // finished; that is a script error, not a result.
        ctx.fetch(&executor)
            .take_result::<Value>(ctx)
            .map_err(piccolo::RuntimeError::new)??;
        Ok(())
    })
    .map_err(|e| e.to_string())?;
    let mut batch = shared.batch.borrow_mut();
    match batch.refusal.take() {
        Some(reason) => Err(reason),
        None => Ok(std::mem::take(&mut batch.invocations)),
    }
}

/// Opt-in block evaluator that renders invocation data as plain JSON.
///
/// Its distinct `lua-command` language tag requires separate registration and
/// host allowlisting. Output is data, never automatically dispatched. Ordinary
/// [`crate::LuaEvaluator`] note fences remain pure and have no `command` binding.
pub struct LuaCommandEvaluator {
    offered: CommandSet,
    schema: String,
    limits: CommandLimits,
    max_string_bytes: usize,
}

impl LuaCommandEvaluator {
    pub fn new(offered: CommandSet, schema: impl Into<String>, limits: CommandLimits) -> Self {
        Self {
            offered,
            schema: schema.into(),
            limits,
            max_string_bytes: DEFAULT_MAX_STRING_BYTES,
        }
    }

    /// The longest string, in bytes, a script may build.
    pub fn max_string_bytes(&self) -> usize {
        self.max_string_bytes
    }

    /// Replace the string limit with the host's own.
    pub fn with_max_string_bytes(mut self, bytes: usize) -> Self {
        self.max_string_bytes = bytes;
        self
    }
}

impl BlockEvaluator for LuaCommandEvaluator {
    fn language(&self) -> &str {
        "lua-command"
    }

    fn eval_block(&mut self, source: &str, max_ops: u64) -> Result<EvalOutput, String> {
        // BlockEvaluator permits the backend to choose a cap for zero. This
        // explicit command-producing lane always chooses a finite default.
        let budget = Budget::Steps(if max_ops == 0 {
            COMMAND_DEFAULT_MAX_OPS
        } else {
            max_ops
        });
        let invocations = collect_with(
            source,
            &self.offered,
            &self.schema,
            budget,
            self.limits,
            self.max_string_bytes,
        )?;
        serde_json::to_string(&invocations)
            .map(EvalOutput::plain)
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
