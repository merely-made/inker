# Lua evaluator plan: Lua on didgeridoo replaces Rhai

**Status:** 2026-11-21. In progress. L1 is met: `script-lua` evaluates a `lua eval` fence, bounded by
fuel, with no `io` and no randomness, and checks for wasm32. L0's inventory is read from source
(Findings 11 to 16) but not yet checked against each resolved graph. L2 to L6 are not started; the
inventory found a fourth lane, Knot's readings, that the phases do not yet name (open question 6).
D1 to D5 ruled the same day.

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
outcome. D1 to D5 are in answer order.

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

- **D2. Notes get no randomness, and the fork's `rand` is brought up to date (Mark, 2026-11-21):**
  Evidence: didgeridoo's `math.random` was seeded from system entropy, so a note could differ between
  runs; its `randomseed` did not build for 32-bit targets; and its `rand` 0.8 put three duplicate crates
  in this graph. Offered: put it behind a feature and leave it off for notes (recommended); keep it and
  fix the 32-bit seed; defer wasm32. Answer: "I'm ok with 1, but there's no way to deduplicate and
  fix?". What follows: both. Didgeridoo moved to `rand` 0.10, whose small generator has one seed size on
  every target, and `math.random` sits behind its `random` feature, on by default; `script-lua` takes
  didgeridoo with that feature off. Outcome: new.

  Links: area The stack · plans lua-evaluator L1, lua-evaluator L4 · summary Didgeridoo is on rand 0.10 with math.random behind a default-on feature; script-lua leaves it off, so a fence is repeatable.

- **D3. gc-arena is patched in its own fork (Mark, 2026-11-21):** Evidence: the bans check refused
  `synstructure` 0.13 through `gc-arena-derive` beside 0.14 elsewhere, and upstream gc-arena's main still
  requires 0.13. Offered: a recorded skip now and the fix in L4 (recommended); fix gc-arena first; hold
  everything. Answer: "2, check upstream, if not we can edit/patch", and on where the patch lives, "Yes,
  on a branch called main". What follows: `mark-ik/gc-arena`, branch `main`, is upstream at the revision
  didgeridoo pinned plus two dependency rows (syn 3, synstructure 0.14); didgeridoo and Genet's
  `genet-didge` both take it. No skip was recorded. Outcome: new.

  Links: area The stack · plans lua-evaluator L1 · summary gc-arena comes from the patch-only fork mark-ik/gc-arena, whose derive is on syn 3 and synstructure 0.14.

- **D4. A fence may be a bare value (Mark, 2026-11-21):** Evidence: a Lua chunk returns only what it
  `return`s, so `1 + 1` is a compile error where Rhai accepted it. Offered: accept both (recommended);
  plain Lua only. Answer: "Accept both (Recommended)". What follows: the source is read as
  `return <source>` first and as written otherwise; the error reported is the one for the source as
  written, with the author's line numbers. Outcome: new.

  Links: area The stack · plans lua-evaluator L1 · summary A lua eval fence is tried as a value first and as a chunk otherwise.

- **D5. A zero budget means a default cap (Mark, 2026-11-21):** Evidence: the trait lets a caller pass
  0 for "the backend picks"; Rhai treats that as unlimited. Offered: a default cap (recommended);
  unlimited, as Rhai does. Answer: "A default cap (Recommended)". What follows: 0 means 1,000,000
  operations; nothing runs unbounded. A script that ran unbounded under Rhai can be cut off. Outcome:
  applies this plan's constraint that every evaluation is bounded.

  Links: area The stack · plans lua-evaluator L1 · summary A budget of 0 is 1,000,000 operations on Lua, never unlimited.

## Open questions

1. Rhai already written into documents. Knot notes hold `rhai eval` fences and `rhai-command` scripts.
   Convert them, keep a reader for the old fence, or refuse them with a message? Returns to Mark in L5.
2. Where the Lua crate lives and what it is called. Beside `script-rhai` in this repository is assumed;
   the name is unruled.
3. Memory. Rhai caps string, array and map sizes. Piccolo reports `total_memory()` and has no
   allocation limit; `script-lua` sets none, so only fuel bounds growth, indirectly. Whether to refuse
   above a ceiling read between fuel steps is undecided.
4. Whether the Lua crate and Genet's `script-engine-piccolo` should share their sandbox setup, or stay
   two users of the fork.
5. Speed. Rhai walks a syntax tree and piccolo runs bytecode, so Lua is expected to be faster; nothing
   here has measured it.

6. The readings lane. Knot's readings build their own Rhai engine on `base_engine()` and are the only
   product code that reads Rhai values and errors (Findings 12). L1 to L3 cover `script-rhai`'s three
   parts and none covers this. Does it get its own phase before L5, and does its host surface (eight
   types, the reading functions) move as it is?
7. Where Lua and Rhai disagree and a test says so (Findings 14): a missing key compared with a number,
   joining a string to a boolean, and whether a string's length counts characters or bytes. Each needs
   a ruling: match Rhai through a host helper, or take Lua's behaviour and rewrite the scripts.
8. Names stored outside code (Findings 15): the environment default, the backend id, the `.rhai`
   extension of user reading scripts, the `rhai-command` tag and the highlighter entry. Open question 1
   is mostly about the user's `.rhai` files.
9. What L4 is for. No authored script uses a string library function (Findings 13), so the string
   work in didgeridoo is not gated by a consumer today. It was started on 2026-11-21 before this
   inventory was read.

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

6. The fork's `string` library has seven functions: `len`, `byte`, `char`, `sub`, `lower`, `upper` and
   `reverse`. There is no `format`, `rep`, `find`, `gsub`, `gmatch` or `match`, and no `utf8`. L4 is
   larger than this plan assumed, and L3 and L5 wait on it wherever they build text.
7. `script-lua` (`crates/script/lua`) builds a fresh VM for each fence, because a piccolo `Lua` cannot
   move between threads and `BlockEvaluator` must. One unit of budget is one unit of fuel. Its cost per
   fence is unmeasured.
8. Piccolo's tables seed their hashing through `ahash`, so wasm32 needs `getrandom`'s browser backend:
   a wasm32-only row in the crate and `--cfg getrandom_backend="wasm_js"` in the build, as `script-rhai`
   already requires.
9. Floats display through Rust's formatting, so `1.0` shows as `1`; and piccolo's `sin`, `cos` and `tan`
   call the standard library, not `libm`. L2 compares raw bits and brings its own maths.
10. Two test failures predate this work and are not caused by it: didgeridoo's `goldenscripts` fails on
    a checkout with CRLF line endings, and gc-arena's `ui` test differs from its saved text in compiler
    wording on three cases, the same three with and without the patch.

11. L0, read 2026-11-21 from each repository's `origin/main`; nothing was run. Rhai is named by Inker
    (`script-rhai` itself), Knot (`knot-editor` and `knot-readings`, ungated), Graphshell's Scenograph
    (a dev-dependency, test `script_rhai_rules`, feature `scripts`) and Turnstone's `ports/graphshell`
    (a dev-dependency). Genet, Mere, Armillary, Distillery and Woodshed have no Rhai in code.
12. Product call sites are all in Knot: `RhaiEvaluator` registered when the allowed languages contain
    `"rhai"` (`crates/knot-editor/src/bin/knot_endpoint/effects.rs:45`, `:65`), and the readings lane
    (`crates/knot-readings/src/bindings.rs:99`), which builds on `base_engine()`, registers eight types
    and the reading functions, sets string, array and map ceilings (`:106`), and classifies Rhai's
    errors. `RhaiValueEngine` and the command lane have no production consumer; Scenograph's
    production code uses only the `ValueEngine` trait.
13. About 130 authored scripts exist, nearly all in tests: Inker about 55, Knot about 53, Graphshell
    about 17, Turnstone 4. No shipping document holds a `rhai` fence. None uses `sub_string`, `split`,
    `replace`, `trim` or number formatting. The most used feature is joining a string to a number or
    boolean with `+` (about 14 scripts); then `for` over a host array, `.len()`, `push`, array and map
    literals. The host functions used most are `command` (about 33), `note` (15) and `row` (12).
14. Three behaviours the tests assert differ in Lua. A missing map key is `()` and `() > 2.0` is false
    in Rhai (Scenograph's `script_rhai_rules.rs`); Lua raises an error comparing `nil` with a number.
    `"x" + true` joins in Rhai; Lua joins only numbers. A Rhai string's length counts characters
    (`knot-readings/tests/reading_bindings.rs:81`); Lua's `#s` counts bytes.
15. Stored or wire uses of the name: Turnstone's default for `TURNSTONE_KNOT_RUN_LANGUAGES` and two
    arguments passed to the `knot_endpoint` child
    (`src/knot_authoring/knot_authoring_engine_from_env.rs:68`, `:332`, `:363`); the backend id
    `knot.rhai.reading` (`crates/knot-readings/src/backend.rs:55`); the `.rhai` extension that lists a
    user's reading scripts (`crates/knot-readings/src/scripts.rs`); the `rhai-command` fence tag; and
    Illume's highlighter entry.
16. A budget of 0 means three things today: unlimited in the note lane, 100,000 in the command lane,
    and refused in the readings lane. D5 settles the note lane only. The readings lane is also the only
    one with memory ceilings (string 64 KiB, array 8,000, map 1,024). The `maths_bits` corpus hash
    `028dc38fdcbf650d` (`crates/script/rhai/tests/support/rule_bits.rs:22`) is L2's comparison target.

## Progress

- 2026-11-21: plan written; D1 ruled.
- 2026-11-21: L1 met. `script-lua` passes 17 tests, clippy is clean, and it checks for
  `wasm32-unknown-unknown`. Didgeridoo `6812717` (rand 0.10, the `random` feature, the gc-arena fork),
  `mark-ik/gc-arena` `b79d86d` and Genet `5b6b624cecf` landed for it. D2 to D5 ruled.
- 2026-11-21: L0's inventory read from source (Findings 11 to 16); the resolved-graph check is
  not done. Open questions 6 to 9 added.
