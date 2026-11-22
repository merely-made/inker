# Lua evaluator plan: Lua on didgeridoo replaces Rhai

**Status:** 2026-11-22. In progress. L1, L2 and L3 are met: `script-didge` (named `script-lua` until
D22) evaluates a `didge eval` fence, implements `ValueEngine` and collects commands from a
`didge-command` fence, each bounded by fuel, with no `io` and no randomness, checked for wasm32; the
Lua engine matches Rhai bit for bit on the `maths_bits` rule (Findings 19). L0's inventory is read
from source (Findings 11 to 16) but not yet checked against each resolved graph. LR, L5 and L6 are
not started. Of L4, the fork has its string library, an embedder-set string limit and libm maths
(Findings 17, 18; D10, D11, D16). D1 to D22 are ruled.

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

A `didge eval` fence to text, with the output convention `script-rhai` documents (a string, or a table
naming a format and text).

**Done-conditions:** `script-rhai`'s evaluator tests pass translated to Lua; an endless loop is cut off
by fuel and reported as a refusal, not a hang; a script cannot reach `io`, checked by a test that
tries; the crate checks for `wasm32-unknown-unknown`.

### L2. The value engine on Lua

**Done-conditions:** the Lua engine implements `ValueEngine`; a Lua rule's maths is bit-exact, from
Lua's own `math` library and `^` (D17); the `maths_bits` corpus produces the same hashes on Lua as on
Rhai, which is the bit-exact comparison between the two engines.

### L3. The command lane on Lua

**Done-conditions:** `collect_invocations` and its limits have Lua counterparts; the command tests pass
translated; an unregistered or unlisted script is still refused.

### L4. The fork's gaps (touches didgeridoo)

**Done-conditions:** every `string`, `table` or `utf8` function that L1 to L3 or a consumer needs is
named in Findings and lands in the fork; the fork's Lua 5.4 suite counts do not fall.

### LR. The readings lane on Lua (touches knot-editor)

Knot's readings build their own engine: eight host types, the reading functions, and string, array and
map ceilings. This phase ports that surface to Lua behind the same backend seam, before any consumer
moves (D9).

**Done-conditions:** a Lua reading backend passes Knot's reading tests translated, the sandbox cases
included (no file, module, `eval` or network reach; a write to a host value fails); a script cannot
exceed the row, note and source limits the Rhai lane sets, and a budget of 0 is still refused; a
compile error reports its line; a runaway reading is cut off by fuel. Lengths are bytes, matching the
byte offsets spans already use, with a character count offered beside them (D8).

### L5. Consumers move (touches knot-editor, graphshell, turnstone)

Knot's block evaluator and readings, Scenograph's rule runner, Turnstone's command invocation.

**Done-conditions:** each consumer's tests pass on Lua and its manifest no longer names `script-rhai`;
open question 1 is answered before the first consumer that holds authored Rhai moves.

### L6. Rhai retires

**Done-conditions:** `script-rhai` is removed; `rhai` is absent from every family repository's resolved
graph; Graphshell's graph behaviors plan states the lanes as they then are.

## Decisions

Each records the evidence put to Mark, what was offered, his answer in his words, what follows, and an
outcome. D1 to D22 are in answer order.

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
  every target, and `math.random` sits behind its `random` feature, on by default; `script-didge` takes
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

- **D6. A missing fact is handled the Lua way (Mark, 2026-11-21):** Evidence: Scenograph rules such as
  `facts.weight > 2.0` rely on Rhai reading a missing key as `()` and `() > 2.0` as false; a test
  asserts "a traced non-match, not a crash", and that negating it matches. Lua raises an error
  comparing `nil` with a number, and didgeridoo gives custom behaviour only to tables and host objects
  (`src/meta_ops.rs:166`). Offered: Lua's way, guarded (recommended); keep the quiet non-match through
  a change to the fork; missing facts are an error by name. Answer: "Lua's way, guarded
  (Recommended)". What follows: a rule is written `facts.weight and facts.weight > 2.0`; the rule host
  reads `nil` or `false` as no match; an unguarded comparison is a rule error whose trace names the
  cause; the rule tests are rewritten. Outcome: new.

  Links: area The stack · plans lua-evaluator L2, lua-evaluator L5 · summary Lua rules guard a fact that may be missing; the rule host reads nil or false as no match, and an unguarded comparison is a rule error.

- **D7. Text is joined as plain Lua (Mark, 2026-11-21):** Evidence: the most used feature in the
  authored scripts is joining a string to a number or boolean with `+`, in about 14 scripts; Lua's `..`
  joins strings and numbers and raises an error on a boolean. Offered: plain Lua (recommended); a host
  helper that joins anything; make `..` accept booleans in the fork. Answer: "Plain Lua
  (Recommended)". What follows: numbers join as they are and anything else is wrapped in `tostring`;
  the affected scripts are rewritten; the fork's `..` is unchanged. Outcome: new.

  Links: area The stack · plans lua-evaluator L3, lua-evaluator LR · summary Lua scripts join text with .. and tostring; no host helper and no change to the language.

- **D8. Length is bytes, with a character count beside it (Mark, 2026-11-21):** Evidence: a Rhai
  string's length counts characters, and `knot-readings/tests/reading_bindings.rs:81` asserts it; Lua's
  `#s` counts bytes; reading spans are already byte offsets; the fork has no `utf8` library. Offered:
  bytes with a character count beside it (recommended); bytes only. Answer: "Bytes, with a character
  count beside it (Recommended)". What follows: `#s` stays bytes; a character count comes from
  `utf8.len` once the fork has it, or a host function until then; the test that asserts characters says
  which it means. Outcome: new.

  Links: area The stack · plans lua-evaluator LR, lua-evaluator L4 · summary In Lua scripts #s is bytes, as spans are; a character count is offered separately.

- **D9. Readings get their own phase, before consumers move (Mark, 2026-11-21):** Evidence: the
  inventory found Knot's readings build their own Rhai engine, with eight host types, the reading
  functions and the only memory ceilings, and are the only product code that reads script values
  directly (Findings 12); L1 to L3 cover `script-rhai`'s three parts and none covers this. Offered: its
  own phase before consumers move (recommended); fold it into the consumers phase; leave readings on
  Rhai for now. Answer: "Its own phase, before consumers move (Recommended)". What follows: phase LR,
  placed before L5; its key is a name so the existing phase keys keep their meaning. Outcome: new.

  Links: area The stack · plans lua-evaluator LR · summary Knot's readings surface is ported to Lua in its own phase, LR, before any consumer moves.

- **D10. The string limit is the embedder's (Mark, 2026-11-21):** Evidence: didgeridoo's new
  `string.rep` built its result before fuel was looked at, under a constant 1 GiB ceiling held twice
  while interned; nothing bounded `..`, `format`, `gsub` or `table.concat`. Knot's Rhai readings cap a
  string at 64 KiB (`max_note_bytes`); `script-rhai`'s note lane sets no string cap. Offered: the
  embedder sets it (recommended); a lower constant in the fork; leave it at 1 GiB. Answer: "The
  embedder sets it (Recommended)". What follows: a didgeridoo state carries the limit
  (`Context::set_max_string_bytes`, 1 GiB until set) and every builder refuses a longer result;
  `script-didge` passes its host's limit; readings pass Knot's own budget in LR. Outcome: new.

  Links: area The stack · plans lua-evaluator L4, lua-evaluator LR · summary A didgeridoo state carries a string limit its embedder sets; script-lua and the readings lane pass their host's.

- **D11. A note fence may build 64 KiB (Mark, 2026-11-21):** Evidence: D10 needs a default for the note
  sandbox; Knot allows a note 64 KiB (`knot-readings/src/lib.rs:87`). Offered: 64 KiB (recommended);
  1 MiB; 16 MiB. Answer: "64 KiB (Recommended)". What follows: `script-didge`'s
  `DEFAULT_MAX_STRING_BYTES` is 64 KiB and a host replaces it with
  `LuaEvaluator::with_max_string_bytes`. Outcome: new.

  Links: area The stack · plans lua-evaluator L1, lua-evaluator L4 · summary script-lua's note sandbox limits a built string to 64 KiB unless the host sets another.

- **D12. Lua's `^` is libm's pow (Mark, 2026-11-22):** Evidence: the Lua value engine matched Rhai bit
  for bit because the rule called a host `pow`; Lua's own `^` used the platform's `powf` and differed
  from libm's `pow` in 175 of 1,999 samples; didgeridoo computes `^` in one place (`constant.rs:166`,
  reached from `meta_ops.rs:682`). Offered: patch didgeridoo so `^` uses libm (recommended); leave `^`
  and have rules call `pow()`; refuse `^` in rule source. Answer: "1, but I wonder if we should use
  libm more broadly. How's it compare to powf? What is the intention behind using powf?" What follows:
  `^` calls libm in the fork; the wider question went back with measurements and became D16.
  Outcome: new.

  Links: area The stack · plans lua-evaluator L2, lua-evaluator L4 · summary didgeridoo's power operator calls libm, so it gives the same bits on every target.

- **D13. The shared maths function belongs beside the trait, in Genet (Mark, 2026-11-22):** Evidence:
  `register_libm_maths` (12 host functions) takes any `ValueEngine`; `script-rhai` has one and the L2
  lane put an identical copy in `script-didge`. Offered: beside the trait in Genet's `script-engine-api`
  (recommended); keep the copy in `script-didge`; a small crate in Inker. Answer: "Beside the trait, in
  Genet (Recommended)". What follows: D17 took the Lua copy away, so only Rhai's remains and it
  retires in L6; the move into Genet waits for another engine that needs host maths. Outcome: new.

  Links: area The stack · plans lua-evaluator L2, lua-evaluator L6 · summary Engine-neutral host maths belongs in Genet's script-engine-api; the move waits until an engine other than Rhai needs it.

- **D14. How a Lua table crosses to the host (Mark, 2026-11-22):** Evidence: a Lua table does not say
  whether it is a list or a map. As built: keys exactly `1..n` make a List, all-string keys a Map, an
  empty table an empty List, anything else is refused; a `Unit` in a list is refused, since `nil` would
  end the list, and in a map it leaves the key out. An empty Map sent in comes back an empty List.
  Offered: accept as built (recommended); an empty table is refused; an empty table reads as Unit.
  Answer: "Accept as built (Recommended)". What follows: those rules stand; an ambiguous table is an
  error, never a guess. Outcome: new.

  Links: area The stack · plans lua-evaluator L2, lua-evaluator L5 · summary A Lua table is a List when its keys are 1..n or it is empty, a Map when every key is a string, and refused otherwise.

- **D15. A rule may build 64 KiB of string (Mark, 2026-11-22):** Evidence: Rhai's value engine sets no
  string, array or map ceiling; the lane gave the Lua value engine the note sandbox's 64 KiB, settable
  by the host, applying to strings a rule builds and not to ones the host passes in. Offered: 64 KiB as
  notes (recommended); 1 MiB; unset as Rhai's is. Answer: "64 KiB, as notes (Recommended)". What
  follows: `LuaValueEngine` defaults to `DEFAULT_MAX_STRING_BYTES`; a host replaces it with
  `with_max_string_bytes`. Outcome: applies D10 and D11.

  Links: area The stack · plans lua-evaluator L2 · summary The Lua value engine limits a built string to 64 KiB unless the host sets another.

- **D16. All of didgeridoo's float maths is libm's (Mark, 2026-11-22):** Evidence: `powf` and the rest
  are Rust's defaults and call the operating system's C library, which rounds the last bit differently
  on each platform; the `libm` crate is one implementation everywhere. Over 2 million inputs each on
  the laptop the two differed in 0.01% to 10% of results (pow 9.9%, exp 9.5%, tan 3.9%, sin 1.6%),
  always by one unit in the last place, and never for sqrt, floor or remainder; libm took 0.9 to 2.7
  times as long (pow 33 ns against 12 ns). Besides `^`, the `math` library had ten platform calls.
  Offered: `^` and the whole math library (recommended); the same behind a feature; `^` only. Answer:
  "^ and the whole math library (Recommended)". What follows: `math.exp`, `log`, `sin`, `cos`, `tan`,
  `asin`, `acos` and `atan` call libm, with no switch back; Genet's scripts on the fork change with
  it. Outcome: refines D12.

  Links: area The stack · plans lua-evaluator L4 · summary Every float result a didgeridoo script computes comes from libm, the same bits on every target.

- **D17. Lua rules use Lua's own maths (Mark, 2026-11-22):** Evidence: with D16 the engine's `math`
  library is bit-exact, so a rule no longer needs host functions for maths; the value engine loaded no
  `math` and rules called bare host names, as Rhai rules do. Offered: Lua's own math library
  (recommended); both; bare host names only, as built. Answer: "Lua's own math library
  (Recommended)". What follows: the value engine loads `math` (no `random`); rules write `math.sin(x)`
  and `x ^ y`; `script-didge` has no `register_libm_maths`; the bit comparison with Rhai runs on that
  form; the move D13 names waits. Outcome: refines D13.

  Links: area The stack · plans lua-evaluator L2, lua-evaluator L5 · summary Lua rules write math.sin(x) and x ^ y; the value engine loads Lua's math library and registers no host maths.

- **D18. A malformed command call voids the batch (Mark, 2026-11-22):** Evidence: in Rhai, calling
  `command()` with the wrong kind of argument is a missing-function error a script can catch with no
  effect on the batch; the Lua lane as built refuses it and voids the batch even when caught, as for
  an unoffered command. Offered: refuse and void the batch (recommended); an ordinary error, as in
  Rhai; coerce numbers to text. Answer: "Refuse and void the batch (Recommended)". What follows:
  `command` takes exactly an id string and an arguments string; anything else voids the batch.
  Outcome: new.

  Links: area The stack · plans lua-evaluator L3 · summary A command call that is not an id string and an arguments string voids the whole batch, caught or not.

- **D19. A command script's default budget is a note's (Mark, 2026-11-22):** Evidence: for a budget of
  zero Rhai's command lane used 100,000 operations and the Lua note lane uses 1,000,000 units of fuel
  (D5); the two units are not the same amount of work. Offered: 1,000,000 as notes (recommended);
  100,000 as built; no default, zero is refused. Answer: "1,000,000, as notes (Recommended)". What
  follows: the command evaluator uses `DEFAULT_MAX_OPS`; `collect_invocations` still refuses
  `Budget::Steps(0)`, as Rhai's does. Outcome: applies D5.

  Links: area The stack · plans lua-evaluator L3 · summary A didge-command fence given a zero budget runs under the note lane's default of 1,000,000.

- **D20. The command limits and the batch live in hest (Mark, 2026-11-22):** Evidence: `CommandLimits`
  and the batch bookkeeping (256 invocations, 64 KiB of arguments, the first refusal voids the batch)
  are neither engine's and existed in `script-rhai` and again in the Lua crate. Offered: leave both
  until Rhai retires (recommended); move them into hest now. Answer: "Move them into hest now". What
  follows: `hest::batch::{Batch, CommandLimits}` in hest 0.3.1 (Graphshell `1071722f`); both command
  lanes use it and re-export `CommandLimits`. Outcome: new.

  Links: area The stack · plans lua-evaluator L3, lua-evaluator L6 · summary hest owns the command batch and its limits; script-rhai and script-didge both collect through it.

- **D21. A fence names the engine, put back as a question (Mark, 2026-11-22):** Evidence: the command
  evaluator's tag was `lua-command`, mirroring `rhai-command`; hosts allowlist the tag and notes carry
  it. Offered: `lua-command` (recommended); decide with the other stored names in L5. Answer: "Didge?
  Or is there potential of using another lua backend (i.e. shoukd we consider them interchangeable)".
  What follows: answered and put back as D22. Outcome: a question, not a ruling.

  Links: area The stack · plans lua-evaluator L3, lua-evaluator L5 · summary Whether a fence tag names the language or the engine was asked back and ruled as D22.

- **D22. Notes name the engine: `didge` and `didge-command` (Mark, 2026-11-22):** Evidence: evaluators
  are looked up by tag, so the engine behind a tag can change without touching a note; the scripts are
  plain Lua 5.4, and what is didgeridoo's alone is the sandbox's promises (fuel, the string limit,
  maths with the same bits everywhere, no `io`). Offered: the language, `lua` and `lua-command`
  (recommended); the engine, `didge` and `didge-command`; both, with `didge` as an alias. Answer: "The
  engine: didge and didge-command". What follows: the tags are `didge` and `didge-command`; the crate
  is `script-didge` at `crates/script/didge`; a different Lua backend would take its own tag. The
  type names (`LuaEvaluator`, `LuaValueEngine`, `LuaCommandEvaluator`) are unchanged and unruled.
  Outcome: new; answers open question 2.

  Links: area The stack · plans lua-evaluator L1, lua-evaluator L3, lua-evaluator L5 · summary Notes written for didgeridoo use the fence tags didge and didge-command, and the crate is script-didge.

## Open questions

1. Rhai already written into documents. Knot notes hold `rhai eval` fences and `rhai-command` scripts.
   Convert them, keep a reader for the old fence, or refuse them with a message? Returns to Mark in L5.
2. Answered 2026-11-22 (D22): the crate is `script-didge`, beside `script-rhai`. Whether its types
   keep the `Lua` prefix is open.
3. Memory. Strings are answered 2026-11-21 (D10, D11). Rhai also caps array and map sizes; piccolo
   reports `total_memory()` and has no allocation limit, so a table grows until fuel stops the
   script. Whether to refuse above a ceiling read between fuel steps is undecided.
4. Whether the Lua crate and Genet's `script-engine-piccolo` should share their sandbox setup, or stay
   two users of the fork.
5. Speed. Rhai walks a syntax tree and piccolo runs bytecode, so Lua is expected to be faster; nothing
   here has measured it.

6. Answered 2026-11-21 (D9): the readings lane is phase LR. Whether its host surface (eight types, the
   reading functions) moves exactly as it is, or is reshaped for Lua, is still open.
7. Answered 2026-11-21 (D6, D7, D8): Lua's behaviour in all three, and the scripts are rewritten.
8. Names stored outside code (Findings 15): the environment default, the backend id, the `.rhai`
   extension of user reading scripts, the `rhai-command` tag and the highlighter entry. Open question 1
   is mostly about the user's `.rhai` files. Since D22 the highlighter (`illume`) knows `lua` and not
   `didge`, so a `didge` fence is not coloured as Lua until it is taught the tag.
9. What L4 is for. No authored script uses a string library function (Findings 13), so the string
   work in didgeridoo was not gated by a consumer; it landed 2026-11-21 (Findings 17). What L4 still
   owes is `utf8.len` for D8's character count, and whatever LR's reading functions need.

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
7. `script-didge` (`crates/script/didge`) builds a fresh VM for each fence, because a piccolo `Lua` cannot
   move between threads and `BlockEvaluator` must. One unit of budget is one unit of fuel. Its cost per
   fence is unmeasured.
8. Piccolo's tables seed their hashing through `ahash`, so wasm32 needs `getrandom`'s browser backend:
   a wasm32-only row in the crate and `--cfg getrandom_backend="wasm_js"` in the build, as `script-rhai`
   already requires.
9. Floats display through Rust's formatting, so `1.0` shows as `1`. Piccolo's `sin`, `cos` and `tan`
   called the standard library until D16 moved them to `libm`.
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

17. Landed in didgeridoo 2026-11-21 (`98f58a2`): methods on strings (`s:upper()`), `string.rep`,
    `string.format`, and `find`, `match`, `gmatch` and `gsub` on a matcher with no native recursion.
    One call stops with an error at 32 million matcher steps, about 0.2 s on the laptop in a release
    build, and at 200 pending backtracking points; matcher work is charged to fuel. `gmatch` reads a
    leading `^` as a literal, as Lua 5.4 does. `getmetatable("")` raises instead of returning the
    string metatable; `pack`, `unpack`, `packsize`, `dump` and `utf8` are still absent.
18. The string limit (D10) landed in didgeridoo `106b3d6`: `..`, `rep`, `format`, `gsub` and
    `table.concat` each succeed at the limit and are refused one byte past it, with an error a script
    can catch. `script-didge` sets 64 KiB (D11): thirty doublings of a string stop at the seventeenth.
    Tables and total memory are still unbounded except by fuel.

19. L2, run 2026-11-22: `LuaValueEngine` (`crates/script/didge/src/value.rs`) keeps globals and host
    functions as plain data and builds a fresh Lua state for each evaluation, loading `base`, `math`,
    `string` and `table`. On the `maths_bits` rule, written with `math.exp`, `math.sin`, `math.cos`,
    `math.sqrt` and `^`, it gives Rhai's recorded hash `028dc38fdcbf650d` and equals Rhai's result at
    each of the 10,000 positions.
20. What the Lua engine does that Rhai's does not: a name that is a Lua reserved word is refused; a
    global and a host function cannot share a name, since Lua has one namespace; values read out of a
    script are capped at 2^20 nodes and 64 levels, so shared or self-holding tables cannot outrun
    fuel; a non-UTF-8 string or a function returned by a rule is a value error. An unknown name is
    `Unit`, not an error (D6). Rhai's method-style call test has no Lua form and became a plain call.
21. Not verified for L2: `Budget::Unbounded` with deep recursion; the maths on wasm32 (the crate is
    checked for that target, not run there; Rhai has a Node runner and Lua has none); speed. Each
    evaluation rebuilds the state and copies the globals in.
22. Didgeridoo tracks a `Cargo.lock`, which the family's no-locks rule does not expect of a
    repository we own.

23. L3, run 2026-11-22: `collect_invocations` and `LuaCommandEvaluator` (`src/command.rs`) register one
    `command(id, arguments)` function on the note sandbox; a Lua string carries exact bytes, so Rhai's
    separate blob form has no counterpart. All of Rhai's command tests are translated; a refusal caught
    with `pcall` or inside a coroutine still voids the batch; an ordinary `didge` fence has no
    `command`. `Budget::Unbounded` runs with the largest countable fuel, not literally without a
    limit. Piccolo has no `xpcall`.
24. Not verified for L3: nothing ran on wasm32 (the crate is checked for it); the text of piccolo's own
    error for a malformed call is not asserted.

## Progress

- 2026-11-21: plan written; D1 ruled.
- 2026-11-21: L1 met. `script-didge` passes 17 tests, clippy is clean, and it checks for
  `wasm32-unknown-unknown`. Didgeridoo `6812717` (rand 0.10, the `random` feature, the gc-arena fork),
  `mark-ik/gc-arena` `b79d86d` and Genet `5b6b624cecf` landed for it. D2 to D5 ruled.
- 2026-11-21: L0's inventory read from source (Findings 11 to 16); the resolved-graph check is
  not done. Open questions 6 to 9 added.
- 2026-11-21: D6 to D9 ruled on the differences between Lua and Rhai and on the readings lane;
  phase LR added before L5.
- 2026-11-21: L4 in part. Didgeridoo `98f58a2` (string methods, `rep`, `format`, patterns) and
  `106b3d6` (the string limit); `script-didge` passes 64 KiB and its 20 tests pass, wasm32 check
  included. D10 and D11 ruled.
- 2026-11-22: L2 met. `LuaValueEngine` landed with 46 lib tests and the 4 `maths_bits` tests
  passing, wasm32 check included. Didgeridoo `0d4bf1f` moved `^` and the `math` library to libm.
  D12 to D17 ruled. L3 is next.
- 2026-11-22: L3 met. The command lane landed with 58 lib tests passing; the batch and its limits
  moved into hest 0.3.1 and `script-rhai` uses them too (33 tests pass). The crate became
  `script-didge` and its tags `didge` and `didge-command`. D18 to D22 ruled. LR is next.
