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
//! dispatch, and establish their own terminal [`hest::Outcome`]. The limits and
//! the batch are hest's (`hest::batch`).

use std::sync::{Arc, Mutex};

use hest::batch::Batch;
pub use hest::batch::CommandLimits;
use hest::{Invocation, menu::CommandSet};
use inker::{BlockEvaluator, EvalOutput};
use rhai::{Blob, Dynamic, EvalAltResult, ImmutableString, Position};
pub use script_engine_api::Budget;

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
    let batch = Arc::new(Mutex::new(Batch::new(offered.clone(), schema, limits)));
    let collect = {
        let batch = Arc::clone(&batch);
        move |id: ImmutableString, arguments: &[u8]| {
            batch
                .lock()
                .map_err(|_| script_error("command batch is unavailable".into()))?
                .collect(&id, arguments)
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
    // The engine's `command` functions still hold the batch, so take a copy.
    batch
        .lock()
        .map_err(|_| "command batch is unavailable".to_string())?
        .clone()
        .finish()
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
