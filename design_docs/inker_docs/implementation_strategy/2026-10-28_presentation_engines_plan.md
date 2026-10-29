# Presentation engines: routing, the flip, shared receiving and the block resolver

**Status:** 2026-10-28: cut from `2026-06-15_engine_picker_and_pluggability_plan.md`, `2026-06-23_genet_scrying_flipcarrier_plan.md`, `2026-10-22_shared_surface_receiving_plan.md` and `2026-06-13_polyglot_block_resolver_plan.md`, archived in `archive_docs/2026-10-28/`. The library halves are built: `EngineRoutePolicy` and `is_surface_engine` in `inker::routing`, the Graft and Weld `SurfaceEngine` impls, `inker::flip` behind the `genet-donor` feature, the page capture contract at the Inker boundary, and bounded Genet, Pelt and Graphshell shared receiving. Open: every host-facing picker phase (no host has carried them since meerkat left), a host that fires the flip, a real flip-back, concrete native browser receiving with its platform qualification, and the polyglot block resolver, which is plan-only.

**For:** Deciding which engine presents a piece of content and what survives a change of engine. It holds the picker's unbuilt phases, the surface-engine tier (Scry, Weld, Graft), the flip between a document engine and a surface engine, the shared receiving surface through which hosts take controls and frames from either tier, and the unified resolver that would make knot blocks polyglot. The rulings that opened it: Mark's 2026-06-15 activation decision (a global default plus a per-session override), the compatibility-view charter's split (picking is inker, carrying state is the flip), and the 2026-10-22 decision that `wgpu-cue` owns neutral receiving while browser-specific host extensions stay explicit.
Not here:
- Turnstone's concrete Servo consumer, its pixel qualification and the complete Turnstone product UI migration: `turnstone/design_docs/2026-08-25_browser_surface_implementation_plan.md`, in the Turnstone repository.
- The native session store's durable, partitioned and script-integration cookie follow-ons and its flip-back SESSION import: the ranged fetch plan (`2026-09-20_ranged_fetch_plan.md`) receives the session store's open items. The import cannot start before F4 here.
- Wasmtime currency under the resolver's wasm seam (B3 sits on wasmtime 45): the workspace hygiene plan's dependency currency lanes.

Receipts from this plan's runs go to `archive_docs/<date>/`, never into this
plan (DOC_POLICY §8). Evidence that matters is a test in the repository or a
line in Findings.

## Constraints

From the engine picker plan (`archive_docs/2026-10-28/2026-06-15_engine_picker_and_pluggability_plan.md`):
- Picking an engine is inker routing; texture plumbing is scry, graft and weld with netrender compose and the constellation actors; carrying state across a swap is the flip. In-product words stay plain ("compatibility view", "flip", "open in <engine>"); `verso` and `inker` are crate names, not UI words (:44-51).
- Availability is one predicate, `is_available(id) = registry.contains(id) && enabled(id)`. An engine not in the build is not registered; one in the build but deactivated is registered, never routed to, and spawns no actor or producer (:76-80).
- Activation scope is a global default plus a per-session override. Graph scope is reserved, not built (:81).
- Two tiers: tier 1 (`Engine`, portable `EngineDocument`, wasm32-safe, always in) and tier 2 (`SurfaceEngine`, GPU `SurfaceFrame`, native only, one cargo feature per surface-engine crate) (:59-66, 82).
- An unhandled scheme keeps `host.external-protocol` as its fallback and becomes legible rather than changed. A `file://` address feeds the same routing through a content sniffer; local files are not special (:83-84).
- Activation reaping goes through the capability-gated action bus when that lands; until then it is a direct host operation with a diagnostic (:187).

From the flip plan (`archive_docs/2026-10-28/2026-06-23_genet_scrying_flipcarrier_plan.md`):
- One hop: no secondary to secondary. Primaries (genet, nematic) implement `FlipDonor`; secondaries (scry, weld, graft) implement `FlipBack` and never `FlipDonor`; every engine receives (:96-100, 171-177, 212).
- Every state layer degrades and never blocks (:104). Session is carried once, at flip time; a continuous session mirror is rejected as a tarpit (:213). The ceiling is the same page, session and place, never the same running program (:150-151, 213-214).
- Downloads stay outside the flip (:43-45).

From shared surface receiving (`archive_docs/2026-10-28/2026-10-22_shared_surface_receiving_plan.md`):
- No capability claims an operation merely because its supplier provides it. The browser binding refuses common operations it cannot represent and never upgrades native accessibility or composition support; engine and factory accessibility stays Opaque until its owner qualifies and declares a concrete capability (:28-35, 69-73; and `archive_docs/2026-10-28/2026-06-15_engine_picker_and_pluggability_plan.md:325-326`).
- Raw borrowed native handles are not promoted to move-owned shared frames without a custody proof (:31-33).
- Render polling must not wait for GPU completion; private quality observations are not consumed by the embedding host or another producer (:21-24).
- Pelt keeps acquisition, resource delivery, history and engine routing; Graphshell keeps its portable scene extension and selected-content ownership (:11-13, 75-80).
- Receiving consumers resolve one `document-session-api` source. Patch tables are not inherited, so every Git Genet receiving workspace needs the root patch (:49-51, 64-67).

From the polyglot resolver plan (`archive_docs/2026-10-28/2026-06-13_polyglot_block_resolver_plan.md`):
- Own notes resolve per setting; a received knot renders every resolvable block as inert source until explicit consent, through one gate in the registry (:172-174).
- Determinism stays for policy, not content. Scripts and queries generate content and never make trust decisions; acceptance and brokering policy stays declarative data evaluated by Rust (:175-178).
- Every spliced block carries its provenance and is never silent (:179-181).
- No native binaries for diagrams (the browser-reach rule); the wasm kind is native and desktop only, and the browser build reports it unavailable (:127-130, 157-161, 167-168).
- Shell surfacing stays gated; each kind demos in a probe first (:17-20, 190-192).
- Out of scope: a general graph query language, browser wasm (a no-JIT interpreter is separate research), shelling to native diagram binaries, the consent UI, and policy-as-script (:194-199).

## Phases

### R1. Per-host override editor and per-session toggle (picker Phase 2b)
The engine manager shipped only a global toggle. This phase is the `per_host_overrides` editor (a host text field and an engine choice) and a per-session toggle. An explicit "in build / absent" row is cosmetic. `per_host_overrides` and `route_filtered` survive in `inker::routing`; pandect keeps the `disabled_engines` settings field, which nothing outside pandect reads.
- Done when: the source states none; the deferred scope is the editor and the per-session toggle.
- Needs: a host to draw the engine manager (open question 1); session manifest wiring before per-session state persists.
- Rulings: picker decisions 1 and 2.
- From: `archive_docs/2026-10-28/2026-06-15_engine_picker_and_pluggability_plan.md:140-141, 150-152, 187`; `archive_docs/2026-10-28/2026-06-15_engine_picker_and_pluggability_plan.md:4-20`

### R2. No-handler state and local files (picker Phase 4)
Surface `engine.route_degraded` in the picker, add an "open externally" affordance, and put a `file://` content sniffer (extension and magic bytes) in front of `EngineRouteRequest.content_type`. The default policy already routes `text/html` and `application/xhtml+xml` to `genet.web` whatever the scheme, so the HTML half arrived with the Phase 0 reconcile.
- Done when: opening a local `.md` routes to nematic.markdown, a local `.html` to genet, and an unhandled scheme shows the explicit "no engine / open externally" state.
- Needs: a host (open question 1).
- Rulings: picker decisions 4 and 5.
- From: `archive_docs/2026-10-28/2026-06-15_engine_picker_and_pluggability_plan.md:172-174`; `archive_docs/2026-10-28/2026-06-15_engine_picker_and_pluggability_plan.md:83-84, 109-113`

### R3. SurfaceEngine registry fold-in and the scrying tile tails (picker Phase 5)
Phase 0 routed the pin through the policy, but surface construction still binds the concrete `PlatformWebSurfaceProducer` and so bypasses the `SurfaceEngine` registry, dropping the WebView2 handle-handoff protocol that the type-erased lane omits. `ScryingTileEngine` over a host-supplied `ProducerFactory` is the Scry tier-2 engine, and the Graft and Weld engines already implement `SurfaceEngine`. The archived scrying tile plan's tails travel with the fold-in: X2's chrome round-trip, X3's durable pin (pandect's `compat_mode` as source of truth), X4 on macOS and Linux, overlay scrollbars, the frame-arrival wake and `content_generation`, and the modular integration plan's S6 (web content through scrying). Pelt is the nearest live host; it hosts scrying surfaces.
- Done when: the source states none; it names the registry construction replacing the concrete producer bind as the Phase 5 companion to the graft and weld engines.
- Needs: a host (open question 1).
- Rulings: S50 (the picker stays active and may take the fold-in).
- From: `archive_docs/2026-10-28/2026-06-15_engine_picker_and_pluggability_plan.md:176-178, 248, 254-256`; `archive_docs/2026-10-28/2026-07-03_archived_plan_tails_plan.md:949-961`

### R4. Build tier
Feature-gate the tier-2 engine crates and prove a tier-1-only build compiles and runs, routing tier-2 schemes to the fallback. This is the wasm and PWA shape proven on the desktop build.
- Done when: a build without scry, weld and graft compiles and runs, with tier-2 schemes reaching the fallback.
- Needs: -
- Rulings: picker decision 3.
- From: `archive_docs/2026-10-28/2026-06-15_engine_picker_and_pluggability_plan.md:180, 82`

### R5. Engine profile wiring and per-session escalation
The path-resolution primitive in pandect's `engine_profile_store.rs` is called by nothing outside pandect; v0b, per-engine profile wiring, is unbuilt. The multi-graph activation plan's per-session engine-profile escalation waited on v0b, and the picker's activation scope mirrors the persona, session and graph tiering of that boundary.
- Done when: the source states none.
- Needs: open question 3.
- Rulings: picker decision 2; S54 (the engine profile boundary plan was retired and v0b parked here).
- From: `archive_docs/2026-10-28/2026-07-03_archived_plan_tails_plan.md:573-575, 1076-1079`; `archive_docs/2026-10-28/2026-06-15_engine_picker_and_pluggability_plan.md:31, 68, 81`

### R6. register-viewer harvest
The `register-viewer` declarations (accessibility, security, storage and history capabilities, not the live mime table) were found latent and unwired. The plan set them apart as a small follow-up to the routing reconcile.
- Done when: the source states none.
- Needs: a host that routes more than one content engine.
- Rulings: -
- From: `archive_docs/2026-10-28/2026-06-15_engine_picker_and_pluggability_plan.md:33, 116-118`

### R7. First engine declares page capture
Page capture P1 landed at the Inker boundary: host-minted request ids, viewport-only requests, typed PNG outputs, explicit unknown CSS facts, and a correlated hosted completion event. No engine declares capture support yet, and no existing capability was promoted.
- Done when: the source states none.
- Needs: -
- Rulings: -
- From: `archive_docs/2026-10-28/2026-06-15_engine_picker_and_pluggability_plan.md:12-15, 229-240`

### R8. Weld and Graft adapter gates
The adapters are library-qualified. What remains is consumer adoption and the supplier gates the source keeps apart: the Weld input follow-up consumer condition; supplementary Unicode input (pinned Welding's `cef_input::send_key` casts a Rust scalar to `u16`, so native UTF-16 delivery and OS IME need supplier or host work); the upstream Servo native consumer, process-owned Servo host gates, supplier export and action support, Servo GPU synchronisation, and native Windows, macOS and Linux assistive-technology walks; the CEF-enabled Mere adapter and the combined Turnstone executable; and Windows native pixels and foreign accessibility as distinct gates. Weld and Graft producer build-out stays "tracked separately" from the picker.
- Done when: immutable adapter source adoption with one Inker type identity passes Turnstone's real two-page input, find, zoom, permission and teardown scenarios before Turnstone removes its host bridge.
- Needs: Turnstone's lane; the selected consumer graph verified after the Knot, Redshank and Turnstone repins, in that order; broader Mere and Genet repins stay a separate decision.
- Rulings: -
- From: `archive_docs/2026-10-28/2026-06-15_engine_picker_and_pluggability_plan.md:186, 218-227, 274-276, 290-291, 306-316, 333-338`

### R9. Scry compositing tails
Three tails of the archived native surface compositing plan: make `start_capture`'s roughly 500 ms settle non-blocking in wgpu-scry (it blocks the UI thread on a stall restart; needs runtime verification on a demo, not a blind edit); batch the cache-flush submit per tile; and make an explicit D3D12 fence failure warn and fail the tile instead of silently falling back to implicit sync.
- Done when: the source states none.
- Needs: a host to verify against (open question 1).
- Rulings: -
- From: `archive_docs/2026-10-28/2026-07-03_archived_plan_tails_plan.md:63-68`

### F1. A host that fires the flip
`inker::flip` holds the traits, `flip_forward`, `flip_back`, the Scry receiver and the Genet donor, but no host calls it; only `crates/system/fetch/src/cookies_flip.rs` references it. The trigger the plan meant to intercept (the genet to `scrying.web` pin switch) left with meerkat. The forward flip carried URL, scroll and session before the host was retired.
- Done when: a host intercepts the genet to `scrying.web` transition, captures the donor and injects into the receiver, so the switch becomes a flip.
- Needs: open question 2.
- Rulings: -
- From: `archive_docs/2026-10-28/2026-06-23_genet_scrying_flipcarrier_plan.md:188-207`; `archive_docs/2026-10-28/2026-06-23_genet_scrying_flipcarrier_plan.md:9-21`

### F2. The FORM layer
Form values degrade today. Capturing them needs a synchronous genet-DOM read, and the full `verso-genet` donor capture (DOM and forms) needs an off-thread genet document.
- Done when: the source states none; the layer must degrade and never block.
- Needs: F1.
- Rulings: -
- From: `archive_docs/2026-10-28/2026-06-23_genet_scrying_flipcarrier_plan.md:19-20, 271-274, 288`

### F3. The visual cross-fade
Layer 5: hold the donor's last frame as the receiver's first frame, and swap the tile's backing texture when the first post-navigation captured frame arrives. Not built.
- Done when: the source states none.
- Needs: F1.
- Rulings: -
- From: `archive_docs/2026-10-28/2026-06-23_genet_scrying_flipcarrier_plan.md:110, 131-132, 19-20`

### F4. Flip-back to genet
Only mock `FlipBack` impls exist. A real one extracts the lean locator (`BackState`: URL, scroll, optional form, cookies) from the scry WebView through its API and `execute_script`, then genet re-fetches the URL from the lossless root and renders it live, applies scroll and a best-effort form refill, and returns the cookies to the jar. The WebView cookie API has no read path yet (`set_cookie` is write-only), and the reverse cookie flow into the jar is unbuilt.
- Done when: the source states none beyond the re-root path above.
- Needs: F1.
- Rulings: -
- From: `archive_docs/2026-10-28/2026-06-23_genet_scrying_flipcarrier_plan.md:135-169, 208, 55-58, 146-147`

### F5. Weld and Graft flip adapters
The layering names `verso-weld` and `verso-graft` adapters, one per secondary engine. There is no record of either being built.
- Done when: the source states none.
- Needs: F1; R8.
- Rulings: -
- From: `archive_docs/2026-10-28/2026-06-23_genet_scrying_flipcarrier_plan.md:75, 81-82`

### Q1. Concrete native browser receiving
The owning native-browser phase is open although a Turnstone Servo and Metal finite receiver is claimed. Common controls move to the neutral surface while the browser adapters keep their richer extensions. Servo's capability report stays `Constructed`, and the host-import observation lives only in the external receipt, not fed back into that report. A concrete Servo zoom implementation belongs to Turnstone; the Graft seam default refuses.
- Done when: existing browser tests and focused shared receiving tests pass without losing advanced browser operations or changing the host's import obligations.
- Needs: Turnstone's browser surface plan for the consumer; R8.
- Rulings: -
- From: `archive_docs/2026-10-28/2026-10-22_shared_surface_receiving_plan.md:28-35, 106-107, 137-139, 143-144, 172-174`

### Q2. Host-device receiving beyond the claimed passes
Compile, portable and real-Metal offscreen and headed passes are claimed. Native window acceptance, physical IME, human assistive technology and physical GPU-reset qualification are not done, and the complete Turnstone product UI, Linux and Vulkan, and physical input remain separate boundaries.
- Done when: a real device receiving test proves pixels, pending and retry, two producers, input redraw, resize and device or scope replacement, with compile and portable scene tests kept separate from native window and human assistive technology acceptance.
- Needs: Q1.
- Rulings: -
- From: `archive_docs/2026-10-28/2026-10-22_shared_surface_receiving_plan.md:19-26, 102-103, 133-134, 181-182`

### Q3. Receiving upkeep
Three leftovers: the D2 audit baseline snapshot digest mismatch with 39 missing records (a supplemental judgment stands in); existing workspace Clippy warnings (the new adapter modules add none); and the root `document-session-api` patch that Git Genet receiving workspaces still need.
- Done when: the source states none.
- Needs: -
- Rulings: -
- From: `archive_docs/2026-10-28/2026-10-22_shared_surface_receiving_plan.md:66, 103-105`

### B0. The unified block-resolver registry (polyglot P0)
Plan-only: no `BlockHandler` and no `resolve_blocks` exist, and the transclusion and evaluation passes still keep separate `BlockProvenanceMap`s. This is the spine: one `resolve_blocks(doc, registry, policy)` walk in which a tag maps to a handler of kind Render, Fetch, Eval, Query, Diagram or Wasm, with one `BlockPolicy`, uniform provenance, and one place to cap recursion depth and total work across kinds. The existing passes become handlers.
- Done when: the three existing passes run through one registry with one policy, the K1 and K2 tests pass unchanged behind the handlers, and a knot mixing a transclude fence, an eval fence and a render fence resolves in one walk with correct per-block provenance.
- Needs: -
- Rulings: -
- From: `archive_docs/2026-10-28/2026-06-13_polyglot_block_resolver_plan.md:47-83`; `archive_docs/2026-10-28/2026-06-13_polyglot_block_resolver_plan.md:4-10`

### B1. Graph and eidetic query blocks (polyglot P1)
Plan-only. `graph`, `recall` and `trail` tags resolve through a read-only `QueryHandler` holding handles to the kernel graph and the eidetic index, with a small set of named, bounded queries (`neighbors-of`, `recall`, `co-occurring-with`, `visits-in`). A richer query DSL is a later refinement and stays data-shaped.
- Done when: a `recall` block renders ranked hits from the eidetic index inline; a `graph neighbors-of` block renders the node's neighbours as links; queries are read-only and bounded; a received knot's query blocks stay inert until consent.
- Needs: B0.
- Rulings: -
- From: `archive_docs/2026-10-28/2026-06-13_polyglot_block_resolver_plan.md:85-109, 99-101`

### B2. Diagram blocks (polyglot P2)
Plan-only. `dot`, `mermaid` and `abc` tags route through a `DiagramHandler` to a renderer producing a platen swatch rather than text blocks, so the knot's output needs a `Block::Canvas`-shaped hole or a swatch-embedding seam, decided with platen. Renderers are pure Rust or wasm-deliverable. Layout is deterministic, so a swatch can cache as an engram.
- Done when: a `dot` block renders its graph as a swatch inline; the swatch embeds in the knot's rendered output through the platen seam; the renderer is wasm or pure Rust, with no native graphviz.
- Needs: B0; the platen swatch seam; open question 8.
- Rulings: -
- From: `archive_docs/2026-10-28/2026-06-13_polyglot_block_resolver_plan.md:111-136`

### B3. Wasm module blocks (polyglot P3)
Plan-only. A `wasm` tag (a named module, or an inline `wat` body) runs through a `WasmHandler` with a narrow host interface (text in, text or blocks out) and no ambient capability. The seam is now `registry::mod_loader` (`WasmModRuntime`), whose only implementation is `crates/script/document-host/src/runtime.rs` on wasmtime 45. The earlier names (`register-mod-loader`, extism) no longer exist.
- Done when: a `wasm` block runs a named module (text in, blocks out) on native, sandboxed with no ambient capability; the browser build reports the kind unavailable rather than failing.
- Needs: B0; the `registry::mod_loader` seam named above.
- Rulings: -
- From: `archive_docs/2026-10-28/2026-06-13_polyglot_block_resolver_plan.md:138-168`; `archive_docs/2026-10-28/2026-06-13_polyglot_block_resolver_plan.md:151-156`

## Open questions

1. Which host carries the picker's surfaces (the engine manager, the per-node picker, the no-handler state) and the registry fold-in, now that meerkat has gone? Pelt consumes routing and hosts scrying surfaces, and Turnstone consumes routing; the source names neither as the picker host. From: `archive_docs/2026-10-28/2026-06-15_engine_picker_and_pluggability_plan.md:16-19`; `archive_docs/2026-10-28/2026-07-03_archived_plan_tails_plan.md:960-961`
2. Which host fires the flip? The source's options: re-home the host wiring to Turnstone or another current host, or record `inker::flip` as library-only until a host asks for it. From: `archive_docs/2026-10-28/2026-06-23_genet_scrying_flipcarrier_plan.md:204-207`
3. Two `EngineProfileBinding` types exist, pandect's scope enum and inker's `user_data_dir` struct. Rename one, declare inker's the v0b seam, or merge them? From: `archive_docs/2026-10-28/2026-07-03_archived_plan_tails_plan.md:1080-1082`
4. Two picker progress entries, the shared Weld input follow-up (2026-10-06) and the current upstream and coordinated consumer family (2026-10-07), are still marked in progress with no closing entry. Are they closed? From: `archive_docs/2026-10-28/2026-06-15_engine_picker_and_pluggability_plan.md:204, 293`
5. The render ladder tails (keyboard dispatch, interactive-region refinement, routing article `main_text` into the eidetic corpus, and a `pump()` before the scripted extract) wait on the rethink Mark called for at S50 ("render ladder needs rethinking"). Do they stay with this plan, and after what rethink? From: `archive_docs/2026-10-28/2026-07-03_archived_plan_tails_plan.md:967-973`
6. Graph activation scope is reserved, not built, and later extension classes (protocols, scripting) are meant to reuse `is_available` and the build, session and active levels. Neither has a named home. From: `archive_docs/2026-10-28/2026-06-15_engine_picker_and_pluggability_plan.md:81, 85`
7. Should the source-marker badge on spliced blocks default visible (provenance shown) or off (clean reading)? It is a setting either way, no such setting exists in nematic, and it waits for a consumer. From: `archive_docs/2026-10-28/2026-06-13_polyglot_block_resolver_plan.md:201-209`
8. The eval menu is Rhai only: the piccolo Lua backend never landed, and the plan does not say whether it is wanted. Separately, B2 justifies its visual hole by the smolweb fidelity plan's non-box escape hatch (bespoke regime B), which that plan has since superseded with one render path through `EngineDocument` (`Block::Menu` for gopher). Does B2 still need a `Block::Canvas`-shaped seam, and on what ground? From: `archive_docs/2026-10-28/2026-06-13_polyglot_block_resolver_plan.md:22-26, 120-126`
9. Should K3 caching generalise to query results and diagram swatches (LocalOnly engrams with a staleness mark, so an offline render shows cached content honestly), and when? From: `archive_docs/2026-10-28/2026-06-13_polyglot_block_resolver_plan.md:182-185`

## Findings

- 2026-10-28: the root lock named `netrender_device` at `d9d6e778` although Cargo loaded the path member from its parent Netrender checkout at `66b2f72e`; `--locked` metadata alone did not reject the disagreement. Both device source trees are identical. Mere `db8983943` aligns the root record without a supplier update. The isolated web event-receiving repair passed a full build before upstream `8996e935a` supplied that edge with a newer cohort; that newer web lock is preserved.
- 2026-06-15: routing decides by pinned engine, then content type, then per-host override, then scheme, then fallback; `route_filtered(is_available)` walks past an absent or deactivated engine. Routing is two-altitude: a host knows the scheme and pin at navigation time and learns the content type only after the fetch, so both passes must consult one policy. From: `archive_docs/2026-10-28/2026-06-15_engine_picker_and_pluggability_plan.md:57, 200`
- 2026-10-06: surviving routing code is consumed by Pelt (`ports/pelt/core/src/workspace.rs`), `crates/fleece/src/clip.rs`, the `mere::routing` facade (`crates/mere/src/routing.rs`) and Turnstone. The Graft and Weld engines are `crates/inker/engines/graft-engine/src/engine.rs` and `weld-engine/src/engine.rs`. From: `archive_docs/2026-10-28/2026-06-15_engine_picker_and_pluggability_plan.md:161-170, 250-256`
- 2026-10-06: Weld native lowercase input needs the character code in CHAR's `windows_key_code`; virtual A (65) inserts uppercase A even when the supplied text is lowercase a. From: `archive_docs/2026-10-28/2026-06-15_engine_picker_and_pluggability_plan.md:191-196`
- 2026-10-22: Pelt's document lane already owns the retained Genet session, admitted fonts and resources, and host history, so a second static HTML adapter would discard them. `ports/pelt/core/src/lib.rs` is the receiving owner and `ports/graphshell/src/retained_content/producer.rs` supplies the host-device texture and portable scene extension. Graphshell's texture producer uses the synchronous checked raster path and Genet's first Cue adapter assumes exclusive quality polling on its render core; neither qualifies several producers sharing one render core. From: `archive_docs/2026-10-28/2026-10-22_shared_surface_receiving_plan.md:39-48`
- 2026-10-06: `inker::flip` lives in `crates/inker/inker/src/flip/`; the forward flip carried URL, scroll and session before its host retired. From: `archive_docs/2026-10-28/2026-06-23_genet_scrying_flipcarrier_plan.md:9-21, 196-202`
- 2026-10-06: the resolver's wasm seam moved: `registry::mod_loader` (`crates/system/registry/src/mod_loader.rs`) on wasmtime 45; `RhaiEvaluator` (`crates/script/rhai/src/lib.rs`) is the only `BlockEvaluator`. From: `archive_docs/2026-10-28/2026-06-13_polyglot_block_resolver_plan.md:22-26, 151-156`

## Progress

- 2026-10-28: root device lock precision is published in `db8983943`; its queued native library/viewer build passed with 721 source-aware identities, while the full default web build failed in `spareval 0.2.7` on mixed Digest trait versions. Mere `4559b860a` takes latest stable `spareval 0.2.9`, its seven exact companions and a coherent SHA-2 edge in both locks. Native query/test and full web compile-only checks pass; full tests/linking remain queued behind the storage guard. The existing web `genet-taffy` source-record split is preserved separately, with identical source trees. The [bounded receipt](../../../support/receipts/netrender-lock-20261028/receipt.json) binds each source snapshot and result.
- 2026-10-28: cut from the engine picker, flip carrier, shared surface receiving and polyglot block resolver plans; nothing new landed.

## Received from the backlog holder, 2026-10-28

Items the archived-plan-tails plan held with no owner, given this one when the active plans were cut; each is as the holder recorded it, not re-verified.

- Persisted resource settings (Pelt's `SurfaceResourcePolicy` has no serde); background-work and texture-residency budgets; independent appearances for engines other than Reader; native frame performance and background CPU unmeasured. From: `archive_docs/2026-10-28/2026-07-03_archived_plan_tails_plan.md:516-520`
- Mechanism A, the genet-side host-driven transform ask, un-parked only on a performance or correctness need; the unified document host's condition 1. From: `archive_docs/2026-10-28/2026-07-03_archived_plan_tails_plan.md:1018-1022`
