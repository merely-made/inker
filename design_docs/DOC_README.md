# Inker Documentation Index

Inker turns an address into a document a host can show: it selects and
orchestrates the engine for a resource, runs the reader and smolweb lanes,
lays the result out on the document canvas, renders the smolweb and lexes text
for highlighting. It is the engine controller; Mere's engine consumes the RDF
it emits and the host decides what to keep.

## Required reading order

1. [DOC_POLICY.md](DOC_POLICY.md) — the canonical core and the Inker addendum.
2. `inker_docs/` — the engine controller (index below).
3. `nematic_docs/` — the smolweb engine and knot composition (index below).
4. `verso_docs/` — rendering surfaces and the engine flip (index below).

Entries below are copied from Mere's DOC_README. Entries whose files did not
come across in the 2026-11-07 cut are dropped.

## inker_docs/ — the engine controller

It describes `crates/inker/`: `inker`, `document-canvas`, the scrying/graft/weld
engine adapters.

### inker_docs/research/

- [web_surface_contract_assessment](inker_docs/research/2026-09-03_web_surface_contract_assessment.md)
  (the neutral `webcue`/`wgpu-cue` contract is implemented and published;
  Genet/Servo and Turnstone have bounded Metal, Vulkan, Wayland and physical
  IBus receiving evidence at their recorded sources. Other native bindings,
  physical CJK methods, human accessibility and physical device-loss gates
  retain their separate qualification. September's extraction assessment
  remains historical; Mere/Inker owns orchestration above the shared boundary.)

### inker_docs/implementation_strategy/

- [presentation_engines_plan](inker_docs/implementation_strategy/2026-10-28_presentation_engines_plan.md) — **cut 2026-10-28**: engine routing and surface engines, the engine flip host, shared surface receiving, the polyglot block resolver.

## nematic_docs/ — the smolweb engine and knot composition

It describes `crates/nematic/` (`nematic`, `illume`).

### nematic_docs/implementation_strategy/

- [smolweb_fidelity_plan](nematic_docs/implementation_strategy/2026-10-28_smolweb_fidelity_plan.md) — **cut 2026-10-28**: WS2 trust, Micron lanes 1 and 4, Turnstone S1 to S3, fetch actor adoption.
- [micron_navigation_plan](nematic_docs/implementation_strategy/2026-10-28_micron_navigation_plan.md) — **cut 2026-10-28**: A1's Turnstone half and R1; keeps its decisions.
- [polyglot_knot_design](nematic_docs/implementation_strategy/2026-05-08_polyglot_knot_design.md)
  (**implemented 2026-05-08/09**, retained as design rationale and format spec:
  extends the `nematic.knot` note format from frontmatter-plus-markdown to a
  polyglot composition where every other `nematic.*` protocol's blocks embed
  fenced-code-block-style and round-trip back to the source protocol's syntax.)
  (**planned**: collapse the three separate passes — `expand_fenced_blocks`,
  `resolve_transclusions`, `evaluate_blocks`, each with its own dispatch and
  trust handling — into one registry that resolves *any* fenced block by its
  tag, and the new resolver kinds that makes pluggable: graph/eidetic query
  blocks, diagram DSLs, sandboxed wasm blocks.)

## verso_docs/ — rendering surfaces and the engine flip

It describes the engine flip, which is the `flip` module of `crates/inker/inker`
(`api`, `orchestrator`, `scry`, and the `genet-donor` feature). The docs keep
their own root because the charter is a separate argument.

### verso_docs/technical_architecture/

- [compatibility_view_charter](verso_docs/technical_architecture/2026-06-10_compatibility_view_charter.md)
  (**charter decision (Mark, 2026-06-10)**, pre-implementation: verso reborn as
  the engine-flip / compatibility-view seam — portable view-state carriers and
  the one-hop invariant, minted at the first genet→scrying flip.)
