// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! An explicit command-producing script lane, separate from pure note fences.
//!
//! `command(id, arguments)` collects a request from the host's offered set.
//! Arguments are a UTF-8 string or Rhai blob; the host supplies their schema.
//! A complete successful evaluation returns data, never execution or permission.
//! Receivers must recheck current declarations, arguments and authority before
//! dispatch, and establish their own terminal [`hest::Outcome`].

use std::sync::{Arc, Mutex};

use hest::{Invocation, menu::CommandSet};
use inker::{BlockEvaluator, EvalOutput};
use rhai::{Blob, Dynamic, EvalAltResult, ImmutableString, Position};
pub use script_engine_api::Budget;

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
            max_invocations: 256,
            max_argument_bytes: 64 * 1024,
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
        let collect = || {
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
        };
        match collect() {
            Ok(invocation) => {
                self.argument_bytes += invocation.arguments.len();
                self.invocations.push(invocation);
                Ok(())
            },
            Err(reason) => {
                // A script may catch the binding's runtime error. A refused
                // request still invalidates the whole batch in that case.
                self.refusal.get_or_insert_with(|| reason.clone());
                Err(reason)
            },
        }
    }
}

fn script_error(reason: String) -> Box<EvalAltResult> {
    EvalAltResult::ErrorRuntime(reason.into(), Position::NONE).into()
}

/// Collect requests from one fully successful script, in call order.
///
/// `command("view:fit", "")` supplies UTF-8 bytes; a Rhai blob supplies exact
/// arbitrary bytes. The script cannot choose an effect class or an unoffered
/// id. No batch escapes a parse, runtime, operation-budget or collection error,
/// including a collection error caught inside the script. Each call has a fresh
/// engine, scope and batch. Successful collection has no terminal outcome.
pub fn collect_invocations(
    source: &str,
    offered: &CommandSet,
    schema: &str,
    budget: Budget,
    limits: CommandLimits,
) -> Result<Vec<Invocation>, String> {
    let max_ops = match budget {
        Budget::Unbounded => 0,
        Budget::Steps(0) => return Err("command script has no operation budget".into()),
        Budget::Steps(steps) => steps,
    };
    let mut engine = crate::base_engine();
    engine.set_max_operations(max_ops);
    let batch = Arc::new(Mutex::new(Batch::default()));
    let collect = {
        let batch = Arc::clone(&batch);
        let offered = Arc::new(offered.clone());
        let schema = schema.to_string();
        move |id: ImmutableString, arguments: &[u8]| {
            batch
                .lock()
                .map_err(|_| script_error("command batch is unavailable".into()))?
                .collect(&offered, &schema, limits, &id, arguments)
                .map_err(script_error)
        }
    };
    let string_call = collect.clone();
    engine.register_fn(
        "command",
        move |id: ImmutableString, arguments: ImmutableString| {
            string_call(id, arguments.as_bytes())
        },
    );
    engine.register_fn("command", move |id: ImmutableString, arguments: Blob| {
        collect(id, &arguments)
    });
    let _ = engine
        .eval::<Dynamic>(source)
        .map_err(|error| error.to_string())?;
    let mut batch = batch
        .lock()
        .map_err(|_| "command batch is unavailable".to_string())?;
    match batch.refusal.take() {
        Some(reason) => Err(reason),
        None => Ok(std::mem::take(&mut batch.invocations)),
    }
}

/// Opt-in block evaluator that renders invocation data as plain JSON.
///
/// Its distinct `rhai-command` language tag requires separate registration and
/// host allowlisting. Output is data, never automatically dispatched. Ordinary
/// [`crate::RhaiEvaluator`] note fences remain pure and have no `command` binding.
pub struct RhaiCommandEvaluator {
    offered: CommandSet,
    schema: String,
    limits: CommandLimits,
}

impl RhaiCommandEvaluator {
    pub fn new(offered: CommandSet, schema: impl Into<String>, limits: CommandLimits) -> Self {
        Self {
            offered,
            schema: schema.into(),
            limits,
        }
    }
}

impl BlockEvaluator for RhaiCommandEvaluator {
    fn language(&self) -> &str {
        "rhai-command"
    }

    fn eval_block(&mut self, source: &str, max_ops: u64) -> Result<EvalOutput, String> {
        // BlockEvaluator permits the backend to choose a cap for zero. This
        // explicit command-producing lane always chooses a finite default.
        let budget = Budget::Steps(if max_ops == 0 { 100_000 } else { max_ops });
        let invocations =
            collect_invocations(source, &self.offered, &self.schema, budget, self.limits)?;
        serde_json::to_string(&invocations)
            .map(EvalOutput::plain)
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests;
