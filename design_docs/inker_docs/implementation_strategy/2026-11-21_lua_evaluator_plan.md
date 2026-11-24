# Lua evaluator plan: Lua on didgeridoo replaces Rhai

**Status:** 2026-11-23. Every phase is met: Lua on didgeridoo (`script-didge`) runs note fences
(`lua eval`), rules and the command lane (`lua-command`) for Knot, Scenograph and Turnstone, and
`script-rhai` is removed. `rhai` resolves in no family repository that records its resolve, and no
other repository's manifest names it (Findings 31); that check stands in for L0's, whose inventory
was read from source (Findings 11 to 16). L4 closed with every library function a consumer calls
present in the fork and its Lua 5.4 suite count unchanged (Findings 33). D1 to D38 are ruled; D30
replaces D22's fence tags. D34's heap ceiling runs through didgeridoo's bounded run (D36); Knot's
readings still carry their own copy of it.

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
outcome. D1 to D31 are in answer order.

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

- **D23. Lua readings need not keep Rhai's names (Mark, 2026-11-22):** Evidence: spans have a field
  `end`, a reserved word in Lua, so `span.end` does not parse; as built `span["end"]` worked beside an
  alias `stop`. Offered: `stop` with `["end"]` still working (recommended); `stop` only; `finish` only.
  Answer: "We don't have to preserve compatibility in the semantics". What follows: one clean name,
  `stop`, on Span, Heading and Fold; relation endpoints are `subject_stop` and `object_stop`; where a
  Rhai-shaped name or shape is awkward in Lua the Lua form is taken. Outcome: new.

  Links: area The stack · plans lua-evaluator LR, lua-evaluator L5 · summary The Lua reading surface takes clean Lua names and shapes; it does not preserve the Rhai surface's.

- **D24. An absent reading value is nil (Mark, 2026-11-22):** Evidence: the Rhai surface uses stand-ins
  (`rel` is `""`, an absent selection or unplaced endpoint is `-1`, an unknown revision is `""`); D6
  rules a missing fact is handled Lua's way. Offered: nil, Lua's way (recommended); keep the
  stand-ins. Answer: "nil, Lua's way (Recommended)". What follows: those values are nil, an unplaced
  link's `span` is nil, and `has_span` is gone; a reading writes `if link.rel then`. Outcome:
  applies D6.

  Links: area The stack · plans lua-evaluator LR · summary In Lua readings an absent rel, selection, revision, span or endpoint position is nil.

- **D25. The character count is `utf8.len` (Mark, 2026-11-22):** Evidence: D8 offers a character count
  beside `#s`; the lane added a global `char_count(s)`. Offered: `char_count(s)` (recommended);
  `utf8.len(s)`, which the fork lacked; `chars(s)`. Answer: "utf8.len(s)". What follows: didgeridoo
  has `utf8.len` (`cca2162`), every `script-didge` sandbox loads `utf8`, and `char_count` is removed.
  Outcome: refines D8.

  Links: area The stack · plans lua-evaluator L4, lua-evaluator LR · summary Scripts count characters with Lua's own utf8.len; no host function stands in for it.

- **D26. A reading's heap ceiling is a budget field (Mark, 2026-11-22):** Evidence: Rhai capped a
  reading's arrays and maps and Lua tables have no size limit; the lane added a constant 16 MiB heap
  ceiling read between fuel steps; open question 3 was undecided. Offered: a budget field, 16 MiB by
  default (recommended); the constant; a 4 MiB default; no heap ceiling. Answer: "A budget field,
  16 MiB by default (Recommended)". What follows: `ReadingBudget::max_heap_bytes`, which the Rhai lane
  ignores. Notes, rules and commands still have no heap ceiling. Outcome: new; answers open question 3
  for readings.

  Links: area The stack · plans lua-evaluator LR · summary A Lua reading runs under ReadingBudget::max_heap_bytes, 16 MiB by default.

- **D27. Rhai already written is refused with a message (Mark, 2026-11-23):** Evidence: no shipping
  document holds a `rhai` fence (Findings 13), and none was found in the application data on the
  laptop; Knot reports a failed fence only as a count. Offered: refuse with a message naming the Lua
  form (recommended); keep a Rhai reader until L6; convert fences automatically. Answer: "Refuse with
  a message (Recommended)". What follows: a host names retired languages in
  `EvaluationPolicy::retired`, each with the message its fences fail with, checked before the
  allowlist; Knot retires `rhai` and `rhai-command`; a `.rhai` reading is listed with a note, not
  skipped. No Rhai reader is kept. Outcome: new; answers open question 1.

  Links: area The stack · plans lua-evaluator L5, lua-evaluator L6 · summary An old rhai fence or reading is refused with a message naming the Lua form; no Rhai reader is kept.

- **D28. Reading scripts are `.lua` files (Mark, 2026-11-23):** Evidence: Knot finds readings by
  extension (`crates/knot-readings/src/scripts.rs:36`, `.rhai`). Offered: `.didge`, after D22
  (recommended); `.lua`. Answer: ".lua". What follows: Knot lists `.lua` readings. Outcome: new;
  answers open question 8 for the extension.

  Links: area The stack · plans lua-evaluator L5 · summary Knot's reading scripts are .lua files.

- **D29. Code is named for the language, put back as a question (Mark, 2026-11-23):** Evidence:
  Illume's highlighter knows `lua` and not `didge`, so a `didge` fence was uncoloured. Offered: add
  `didge` now and drop `rhai` in L6 (recommended); add `didge` and drop `rhai` now; leave it.
  Answer: "didge is lua. maybe a lua "- didgeridoo" tag of some sort, but files and code should be
  described/portrayed as the language. questions of the engine should highlight that the engine
  providing the language is what it is, i.e. didgeridoo, or didge for short." What follows: it
  contradicts D22's tags and was put back as D30. Outcome: contradicts D22.

  Links: area The stack · contradicts lua-evaluator D22 · plans lua-evaluator L5 · summary Files and code are named for Lua and didgeridoo where the engine is the subject; put back as D30.

- **D30. Fences are `lua eval` and `lua-command` (Mark, 2026-11-23):** Evidence: D29; the `didge`
  tags had been live in `script-didge` since 2026-11-22 and no shipping note used them. Offered:
  `lua eval` and `lua-command`, with didgeridoo named in the crate, the backend id
  `knot.didge.reading`, refusal messages and receipts (recommended); `lua` with an optional engine
  word; `lua` with `didge` kept as an alias. Answer: "lua eval / lua-command (Recommended)". What
  follows: `LuaEvaluator` handles `lua` and `LuaCommandEvaluator` `lua-command`; chunk names read
  `lua eval`, `lua command` and `lua rule`; the fuel refusal reads "didgeridoo refused: ..."; Illume
  colours `lua-command` as Lua; the crate stays `script-didge`; Knot's reading backend becomes
  `LuaReadingBackend` and keeps the id `knot.didge.reading`. Outcome: new; replaces D22's tags.

  Links: area The stack · replaces lua-evaluator D22 · plans lua-evaluator L5 · summary Notes say lua eval and lua-command; didgeridoo is named only where the engine is the subject.

- **D31. A failed fence leaves an error block that stands on its own (Mark, 2026-11-23):** Evidence:
  D27's message needs a place; Knot's Run reported only counts and never a fence's error. Offered:
  in the Run summary (recommended); in place, under the fence; both. Answer: "2 might be a nice
  error convention, add a block that could be copy pasted without the original context and just the
  trace/code/whatever we've got". What follows: Inker's evaluate pass keeps every failed fence (an
  error, a budget refusal, a render failure, a retired language) and adds a preformatted block after
  it: the fence's info string with "failed", the error, and the source; its provenance is
  `failed:<lang>`. A denied fence (evaluation off, a language not allowed) still adds nothing.
  Outcome: new.

  Links: area The stack · plans lua-evaluator L5 · summary A failed fence is followed by a self-contained error block: what failed, the error and the source.

- **D32. Illume keeps highlighting `rhai` fences (Mark, 2026-11-23):** Evidence: L6 removes the
  engine; Illume's `rhai` entry is a keyword lexer with no Rhai dependency, and D27 keeps a refused
  fence in the note as source. Offered: keep it (recommended); drop it with the engine. Answer: "Keep
  it (Recommended)". What follows: an old note's `rhai` fence still reads as code while Knot refuses
  to run it. Outcome: new.

  Links: area The stack · plans lua-evaluator L6 · summary Illume still highlights rhai fences after the engine retires; highlighting is not execution.

- **D33. Records that name Rhai are left as they are (Mark, 2026-11-23):** Evidence: outside live
  code, Rhai is named by Mere's family-promotion `source-workspace.toml` (the frozen input of a
  finished migration script), a Netrender receipt's probe manifest, and older design docs in Knot and
  Inker; none is a resolved graph. Offered: leave them (recommended); scrub them. Answer: "Leave
  them (Recommended)". What follows: L6 restates the lanes only in Graphshell's graph behaviors plan
  and here. Outcome: new.

  Links: area The stack · plans lua-evaluator L6 · summary Files that record Rhai as it was are not edited when Rhai retires.

- **D34. `script-didge` refuses a heap above a ceiling (Mark, 2026-11-23):** Evidence: open question
  3; on didgeridoo a growing table reaches 4 to 26 MiB within the default fence budget of 1,000,000
  fuel and 64 to 240 MiB at 10,000,000, about 25 bytes a unit (Findings 32), and the value engine
  takes an unbounded budget; Knot's readings already refuse above 16 MiB, read between 4,096-fuel
  steps, in a copy of the fuel loop (Findings 27). Offered: a ceiling in `script-didge`'s shared
  fuel loop, 16 MiB by default and set by the host, the loop exported so Knot drops its copy
  (recommended); a ceiling only when the budget is unbounded; fuel alone, with the rate written
  down. Answer: "Heap ceiling in script-didge (Recommended)". What follows: one exported loop serves
  every lane and checks the heap between steps, an unbounded budget included; the refusal reads like
  fuel's. Outcome: new.

  Links: area The stack · plans lua-evaluator L1, lua-evaluator L2, lua-evaluator L3 · summary script-didge refuses a script whose heap passes a host-set ceiling, 16 MiB by default, read between fuel steps.

- **D35. Speed is measured against Rhai's last commit (Mark, 2026-11-23):** Evidence: open question
  5; nothing has timed either engine, and Inker's `ebc5b30^` still holds both, with the same
  `maths_bits` rule on each. Offered: measure now in release builds on that commit, the rule, a note
  fence and per-fence VM setup, changing no code (recommended); measure when a consumer is slow; drop
  the question. Answer: "Measure via a cheap subagent (Recommended)". What follows: the numbers land
  in Findings. Outcome: new.

  Links: area The stack · plans lua-evaluator L1 · summary Lua's speed is measured against Rhai on the last commit that has both.

- **D36. The bounded run lives in didgeridoo (Mark, 2026-11-23):** Evidence: open question 4; five
  embedders on the fork (Genet's `genet-didge`, Isocosm's generators and rules, Mesocosm's expression
  runner, `script-didge`, Knot's readings) share no code, each steps 4,096-fuel slices of its own,
  only the readings read the heap, and which libraries each loads differs for reasons of its own
  (Findings 34). Offered: a bounded-run helper in the fork, fuel slices, a heap ceiling and the fuel
  spent, with `script-didge` calling it and each host choosing its libraries (recommended); D34's
  loop in `script-didge`, others depending on it; stay separate. Answer: "In didgeridoo itself
  (Recommended)". What follows: `piccolo::{drive, DriveLimits, DriveError}` (didgeridoo
  `14ffe15`); `script-didge` re-exports them and keeps the 16 MiB default as its own policy
  (`default_limits`). Knot's copy goes next; Genet, Isocosm and Mesocosm adopt it in their own
  repositories. Outcome: refines D34.

  Links: area The stack · amends lua-evaluator D34 · plans lua-evaluator L1, lua-evaluator L2, lua-evaluator L3 · summary The fuel-and-heap loop is didgeridoo's; each embedder calls it and picks its own libraries and limits.

- **D37. A heap refusal is its own `ValueError` (Mark, 2026-11-23):** Evidence: D34's heap ceiling
  reached the value engine, whose `ValueError` (Genet's `script-engine-api`) had no heap kind; the
  build used `ValueError::Script` with the refusal text, and `ValueError::Budget` would drop the
  text; nothing outside `script-engine-api` matches `ValueError` exhaustively. Offered: `Script`
  with the message (recommended); `Budget`; a heap variant in Genet. Answer: "Add a heap variant in
  Genet". What follows: `ValueError::Heap { max_bytes }` (genet `72ef547e7e2`), and the value
  engine returns it. Outcome: new.

  Links: area The stack · plans lua-evaluator L2 · summary A value engine past its memory ceiling returns ValueError::Heap with the ceiling.

- **D38. The value engine's cost is recorded, and Scenograph asked (Mark, 2026-11-23):** Evidence:
  D35's timings (Findings 35): Lua is 2.5 to 2.7 times Rhai's speed on work, but one value-engine
  evaluation costs 0.214 ms against Rhai's 0.0078 ms, unexamined. Offered: find the cause and cache
  (recommended); record only; record and ask how often Scenograph evaluates. Answer: "Record, and ask
  Scenograph". What follows: Findings 36 answers for Scenograph; a cache waits for a consumer that
  runs scripted rules. Outcome: new.

  Links: area The stack · plans lua-evaluator L2 · summary The value engine's per-evaluation cost is on record; caching waits until scripted rules run somewhere.

## Open questions

1. Answered 2026-11-23 (D27): Rhai already written is refused with a message, in an error block (D31).
2. Answered 2026-11-22 (D22) and 2026-11-23 (D30): the crate is `script-didge`. Its types keep the
   `Lua` prefix, since code is named for the language (D29, D30; Mark, 2026-11-23: "Mark both
   answered").
3. Answered 2026-11-23 (D34): a heap ceiling read between fuel steps. Strings were answered
   2026-11-21 (D10, D11).
4. Answered 2026-11-23 (D36): the bounded run is didgeridoo's; each embedder keeps its own choice of
   libraries.
5. Answered 2026-11-23 (D35, D38; Findings 35, 36): Lua is faster on work and slower per
   value-engine evaluation.

6. Answered 2026-11-21 (D9): the readings lane is phase LR. Its host surface moved as it is, with
   the same readers and constructors (Findings 25; Mark, 2026-11-23: "Mark both answered").
7. Answered 2026-11-21 (D6, D7, D8): Lua's behaviour in all three, and the scripts are rewritten.
8. Answered 2026-11-23 (D28, D30); Turnstone's environment default moved with L5 (Findings 30). Names stored outside code (Findings 15): the environment default, the backend id, the `.rhai`
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

25. LR, built 2026-11-22 on knot-editor's branch `didge-readings`: `DidgeReadingBackend`
    (`knot.didge.reading`) sits behind the same `ReadingBackend` seam as the Rhai backend, with the
    same readers and constructors. Host values are userdata whose fields read and never write; a script
    cannot reach their metatable. `max_ops` is fuel, `max_note_bytes` is also the string limit, a
    compile error carries its line, and a returned list longer than four times `max_rows` is refused.
    Rhai's reading tests are translated beside its own: 63 tests pass across both lanes.
26. LR landed on knot-editor `main` as `ef0091f` and `9534cab`, the second carrying D23 to D26 and the
    `deny.toml` upkeep a fresh resolve needed (six stale skips removed, `libloading` 0.8.9 skipped).
    Unverified for LR: wasm32, release builds, speed, very deep recursion.
27. What `script-didge` should export for LR and does not: its fuel loop (knot-readings carries a
    25-line copy that also counts fuel spent and checks the heap), the `return`-first compile, and the
    classification of string-limit errors.
28. L5 in Knot, knot-editor `main` `06d25e7`: readings run only on Lua; a `.rhai` reading is listed
    with a note asking for a `.lua` rewrite; Knot's evaluate policy retires `rhai` and `rhai-command`
    with messages naming `lua eval` and `lua-command eval`; `script-rhai` leaves the workspace. A
    fresh resolve needed a `windows-sys` 0.59.0 skip (dirs-sys 0.5.0, latest). The desktop
    `readings_panel` tests fail at "the Readings toolbar toggle is missing" on `main` before and after
    this change, so the readings panel is unverified end to end.
29. L5 in Scenograph, graphshell `main` `0ee16afe`: the rule runner's real-engine test runs on
    `LuaValueEngine`. No value (`nil`) is no match and negates like `false` ("script: no value").
    Rhai's `states.contains(x)` has no Lua form; the rules walk `ipairs(states)`. Handing rules a
    set-shaped `states` table is an option not taken.
30. L5 in Turnstone, turnstone `main` `39796c25`: the authoring default allows `lua`, the authoring
    fixture says `lua eval`, and the command-invocation test runs a Lua batch through `script-didge`.
    The endpoint receipts pass 7 of 7 against a `knot_endpoint` built from `06d25e7`, but only with
    the `endpoint-process` feature: under `authoring` alone the hub is a stub and every receipt fails
    with "Knot authoring worker is unavailable". The commit carries `deny.toml` upkeep unrelated to
    Lua (the `itertools` skip moved to 0.12.1; the `windows-sys` 0.59.0 skip went).
    `.cargo/config.toml.example` still names pre-split `../mere/...` paths. Unverified for L5:
    consumers on wasm32, release builds.
31. L6, Inker: `crates/script/rhai` is removed with its workspace row; `script-didge` loses its dev
    edge to it, and `maths_bits` keeps the hash Rhai recorded (`028dc38fdcbf650d`) while the
    element-by-element comparison with the Rhai engine goes. `illume` 37, `inker` 140,
    `script-didge` 59 and `maths_bits` 3 tests pass; `cargo tree -i rhai` finds no package; bans pass
    with the `getrandom` 0.3.4 skip argued through piccolo. Across the family, `house resolved show`
    on each `main` finds `rhai` in Inker's note before this change (the positive control) and in none
    of the seven others that carry a note (graphshell, knot-editor, turnstone, isocosm, murm,
    armillary, dramatis); the twenty-one heads without a note name neither `rhai` nor `script-rhai` in
    a manifest. Finding 29's option was taken: Scenograph's `states` is a set (graphshell
    `01858e8e`, `graphshell:scenograph-editor D1`).
32. Memory, measured 2026-11-23 on didgeridoo `cca2162` (a scratch probe stepping 4,096 fuel at a
    time and reading `total_memory()` after each step): a fresh VM holds about 22 KB. Within 1,000,000
    fuel an array of integers peaks at 4.0 MiB, nested tables at 15.4 MiB and table values under
    float keys at 25.7 MiB; within 10,000,000 fuel, 64.0, 153.4 and 240.4 MiB. Growth is about 25
    bytes a unit of fuel at worst, so fuel bounds memory only as tightly as the budget does, and not
    at all when the budget is unbounded.
33. L4, checked 2026-11-23. Library calls in the Lua the consumers ship and test, by search: Inker's
    `script-didge` calls `string.rep`, `string.char`, `string.format`, `string.gsub`,
    `table.concat`, `utf8.len` and `math`'s `sin`, `cos`, `tan`, `atan`, `exp`, `log`, `sqrt`,
    `floor`, `abs`, `min` and `max`; Scenograph's rules call `math.exp`; Knot's readings call none.
    Turnstone's `os.time` and `os.clock`, and Inker's `io.open`, `os.execute`, `coroutine` and
    `math.random`, appear only in tests that assert the sandbox refuses them. Isocosm's generators,
    rules and Mesocosm's expression pack, which run on the same fork, call only `ipairs`. Every one
    is registered in the fork's `src/stdlib` (didgeridoo `cca2162`). The fork's suite reads 1 of 32
    files passing, 1 skipped, 22 failing and 8 not loading, the count `spec-results.toml` recorded
    on 2026-10-18, so it did not fall.
34. Open question 4, surveyed 2026-11-23 (read only). Four sandboxes on the fork share no code:
    Genet's `genet-didge` loads `Lua::full()` (io included), installs host functions and a promise
    `await`, keeps one VM per engine, and caps only `eval_bounded` by steps; Isocosm's generators
    use `Lua::core()` with a total fuel cap, output and depth limits and a fresh VM each, while its
    rules lane keeps one VM and runs uncapped `execute`; Mesocosm's expression runner uses
    `Lua::core()` without `math.random`, an 8,192-fuel policy, output, depth and length limits;
    `script-didge` and Knot's readings step 4,096-fuel slices, and only the readings read the heap.
    None has a memory limit except the readings. Mesocosm's process-definition plan names shared
    sandbox code "an extraction candidate. It is not extracted first." Isocosm and Mesocosm scripts
    return proposals the host validates, as `lua-command` returns invocations; Mesocosm admits a pack
    all or nothing, with an ABI number, declared files only and `deny_unknown_fields`, and pins a
    fixture's exact proposal and entropy draws.

35. Speed (D35), measured 2026-11-23 in release builds at Inker `274fa4c`, the last commit with both
    engines; medians of five runs after a warm-up, on a shared Ryzen 9 7940HS. The `maths_bits` rule
    at n = 10,000: Rhai 53.2 ms, Lua 21.1 ms (both hash `028dc38fdcbf650d`); at n = 100,000: 524 and
    203 ms. A 100,000-iteration note fence: 104.5 and 38.1 ms. A trivial fence on a fresh evaluator:
    0.347 and 0.227 ms (a reused Rhai evaluator costs 0.0012 ms; `script-didge` builds a VM for every
    fence). One value-engine evaluation of `x * x + math.sqrt(x) * 2.0`: 0.0078 and 0.214 ms, Rhai
    27 times faster; that the chunk is compiled again each time is likely and unchecked. Spreads run
    up to 21 per cent, more on sub-millisecond rows.
36. How often Scenograph evaluates (D38), read 2026-11-23 at graphshell `7809663e`: `RuleSet::resolve`
    walks every rule for each occurrence, and a rule holding a script binds six globals and evaluates
    once (`presentation.rs`, the rule loop). Scripted rules need the `scripts` feature, off by
    default, and no crate in Graphshell, Turnstone or Knot turns it on, so none runs in production.
    At Findings 35's rate a resolve over a hundred occurrences with one scripted rule would spend
    about 21 ms evaluating.

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
- 2026-11-22: LR built and tested on knot-editor's `didge-readings`; D23 to D26 ruled and applied
  there. Didgeridoo `cca2162` adds `utf8.len`; `script-didge` loads it (`6493200`). LR's second
  commit waits on knot-editor's `deny.toml` (Findings 26).
- 2026-11-23: LR met; it landed on knot-editor `main` (`9534cab`). L5 is next.
- 2026-11-23: L5 begun. D27 to D31 ruled. Inker: fences are `lua eval` and `lua-command`, a failed
  fence leaves an error block, a host can retire a language with a message, and Illume colours
  `lua-command`; `inker`, `script-didge`, `script-rhai` and `illume` tests pass.
- 2026-11-23: L5 met. Knot `06d25e7`, Scenograph on graphshell `0ee16afe` and Turnstone `39796c25`
  run on Lua, and none names `script-rhai` (Findings 28 to 30). L6 is next.
- 2026-11-23: L6 met. `script-rhai` is removed and `rhai` resolves nowhere in the family (Findings
  31). D32 and D33 ruled. Graphshell's graph behaviors plan states the lanes as they now are.
- 2026-11-23: L4 met (Findings 33). D34 (a heap ceiling, Findings 32) and D35 (time Lua against Rhai)
  ruled; questions 2 and 6 closed on D29, D30 and Findings 25; question 4 surveyed (Findings 34).
- 2026-11-23: D34 built (Inker `a24df36`). D36 to D38 ruled: the bounded run moved into didgeridoo
  (`14ffe15`), `ValueError::Heap` added (genet `72ef547e7e2`), and the timings recorded (Findings 35,
  36). Questions 4 and 5 answered.
