# Lua evaluator plan: Lua on didgeridoo replaces Rhai

**Status:** 2026-11-21. Plan. Nothing built. D1 ruled the same day.

**For:** one scripting language where there are two. Rhai runs note fences, rules and the command lane;
Lua on didgeridoo (the piccolo fork, `merely-made/didgeridoo`) runs sandboxed bodies in Genet. Mark
ruled that Lua takes Rhai's job too (D1). This plan builds the Lua crate behind the same two seams,
moves each consumer, and retires `script-rhai`.

**Not here:** Genet's DOM-shaped `ScriptEngine` backend (`genet/components/script-engine-piccolo`),
which stays as it is; Lua 5.4 conformance in general, which is the fork's own work; wasm components
(Graphshell's `2026-11-21_guest_runtime_plan.md`).

## Constraints

- The seams do not change: `BlockEvaluator` (`crates/inker/inker/src/document/evaluate.rs:86`) and
  `ValueEngine` (`genet/components/script-engine-api/value.rs:100`).
- Rhai keeps working until each consumer has moved. No consumer is left without an evaluator.
- Every evaluation is bounded by the VM's own fuel, and a runaway script is a tested case.
- The sandbox loads no `io` library. What a script can reach is named, not inherited.
- The Lua crate builds for `wasm32-unknown-unknown`, as `script-rhai` does today.
- Authored Rhai already in documents is not broken silently (open question 1).

## Phases

### L0. Inventory (gate)

**Done-conditions:** Findings lists every Rhai call site by repository and every place Rhai source is
authored or stored (note fences, readings, rules, command scripts), with counts; the list is checked
against `rhai` in each repository's resolved graph.

### L1. The block evaluator on Lua

A `lua eval` fence to text, with the output convention `script-rhai` documents (a string, or a table
naming a format and text).

**Done-conditions:** `script-rhai`'s evaluator tests pass translated to Lua; an endless loop is cut off
by fuel and reported as a refusal, not a hang; a script cannot reach `io`, checked by a test that
tries; the crate checks for `wasm32-unknown-unknown`.

### L2. The value engine on Lua

**Done-conditions:** the Lua engine implements `ValueEngine`; `register_libm_maths` has a Lua
counterpart; the `maths_bits` corpus produces the same hashes on Lua as on Rhai, which is the bit-exact
comparison between the two engines.

### L3. The command lane on Lua

**Done-conditions:** `collect_invocations` and its limits have Lua counterparts; the command tests pass
translated; an unregistered or unlisted script is still refused.

### L4. The fork's gaps (touches didgeridoo)

**Done-conditions:** every `string`, `table` or `utf8` function that L1 to L3 or a consumer needs is
named in Findings and lands in the fork; the fork's Lua 5.4 suite counts do not fall.

### L5. Consumers move (touches knot-editor, graphshell, turnstone)

Knot's block evaluator and readings, Scenograph's rule runner, Turnstone's command invocation.

**Done-conditions:** each consumer's tests pass on Lua and its manifest no longer names `script-rhai`;
open question 1 is answered before the first consumer that holds authored Rhai moves.

### L6. Rhai retires

**Done-conditions:** `script-rhai` is removed; `rhai` is absent from every family repository's resolved
graph; Graphshell's graph behaviors plan states the lanes as they then are.

## Decisions

Each records the evidence put to Mark, what was offered, his answer in his words, what follows, and an
outcome.

- **D1. Lua replaces Rhai (Mark, 2026-11-21):** Evidence: `script-rhai` is 1,472 lines behind two
  engine-neutral traits; nothing on Lua implements them yet; piccolo steps an executor against a `Fuel`
  value, so a budget is the VM's own; the fork's README calls `string`, `table` and `utf8` missing or
  sparse; Graphshell's graph behaviors plan assigns "rhai for privileged local automation, piccolo Lua
  for sandboxed participant bodies". He had said first: "my thinking was that didgeridoo would be able
  to do what rhai does. Indeed, I'd prefer that; lua's pretty cool". Offered: probe first and retire
  nothing (recommended); decide now that Lua replaces Rhai; a paper evaluation only. Answer: "Decide
  now: Lua replaces Rhai". What follows: this plan; the probe is its first phases, not a precondition.
  Outcome: new; amends the lane rule in
  `graphshell/design_docs/graphshell_docs/implementation_strategy/2026-10-28_graph_behaviors_plan.md`
  (Constraints), which carries a note from the same day.

  Links: area The stack · plans lua-evaluator L1, lua-evaluator L6 · summary Lua on didgeridoo takes over every job Rhai has; script-rhai retires once each consumer has moved.

## Open questions

1. Rhai already written into documents. Knot notes hold `rhai eval` fences and `rhai-command` scripts.
   Convert them, keep a reader for the old fence, or refuse them with a message? Returns to Mark in L5.
2. Where the Lua crate lives and what it is called. Beside `script-rhai` in this repository is assumed;
   the name is unruled.
3. Memory. Rhai caps string, array and map sizes. What bounds a Lua script's allocations here is
   unread.
4. Whether the Lua crate and Genet's `script-engine-piccolo` should share their sandbox setup, or stay
   two users of the fork.
5. Speed. Rhai walks a syntax tree and piccolo runs bytecode, so Lua is expected to be faster; nothing
   here has measured it.

## Findings

2026-11-21, from reading source; nothing was run.

1. `script-rhai` (`crates/script/rhai`) has three parts: `RhaiEvaluator` (`src/lib.rs:81`, with the
   shared sandbox in `base_engine`, `:56`), the command lane (`src/command.rs:157`) and
   `RhaiValueEngine` (`src/value.rs:33`). Its own doc says the knot path needs "only eval + budget +
   to-text".
2. Call sites outside this repository: `knot-editor` (`endpoint.rs`, `knot_endpoint/effects.rs`,
   `knot-readings`), Graphshell's Scenograph (`tests/script_rhai_rules.rs`), Turnstone
   (`ports/graphshell/src/command_invocation/tests.rs`). L0 completes this list.
3. Piccolo budgets execution: `Fuel` (`src/fuel.rs:9`) and `Executor::step(ctx, &mut fuel)`
   (`src/thread/executor.rs:186`). Genet's backend refuses a step budget in its `pump`; that is the
   backend's choice, not a limit of the VM.
4. Piccolo's standard library has `base`, `coroutine`, `io`, `math`, `string` and `table` modules. Its
   README says `string`, `table` and `utf8` are "either missing or very sparsely implemented", and the
   fork is working through the official Lua 5.4 suite.
5. Genet describes its Lua backend as "an option module ... not a third first-party substrate". D1
   changes what Lua is for in the script lanes; it does not touch that backend.

## Progress

- 2026-11-21: plan written; D1 ruled.
