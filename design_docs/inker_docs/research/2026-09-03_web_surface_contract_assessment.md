# Web Surface Contract Assessment

**Status (2026-10-05):** assessment complete; the neutral browser-control
extraction remains open. `mere-surface-api` is the separately landed
application discovery/settings contract, not the browser protocol proposed
here. Ordered Scry/Weld completions and owned native-frame delivery have
advanced; the local B0 capability corrections pass their focused tests.
The [2026-10-22 cues](#four-producers-and-two-frame-directions-2026-10-22)
add native Genet as a fourth producer and record that Inker's
`SurfaceProducer` and Cambium's `TextureProducer` move frames in opposite
directions.

The original assessment sections below describe the September 3 source
snapshot. Dated findings and migration progress record later repairs and
remaining gates; historical mismatches are not claims about today's source.

## Verdict

Extracting a neutral contract is useful, but not before the contract is made
fully honest about asynchronous browser operations and resource ownership. The
right order is:

1. keep the current publish train unblocked;
2. repair capability honesty and sync/async mismatches in the existing crates;
3. extract a small neutral surface-contract crate only after the event and
   request model is stable enough to be a public semver surface.

The assessment should not block `grafting` 0.6.0, `scrying` 0.7.0, or
`welding` 0.14.0. It should block any stronger public claim that these crates
already form a complete browser replacement platform.

## Architectural ruling

There are three distinct layers, and combining them would recreate the present
translation problem in a public crate:

1. **Native image custody and import.** Graft owns OS/GPU resource lifetime,
   synchronization, and adoption by the host's wgpu device. Its owned transport
   types eventually need a wgpu-free package/module boundary so a neutral
   surface protocol can use them without selecting the host's wgpu major.
2. **One browser surface protocol.** A small neutral contract owns
   non-blocking frame acquisition, input/focus, correlated commands, one ordered
   event stream, settings outcomes, and runtime capability evidence.
3. **Host orchestration.** Inker owns engine registration and choice, profile
   binding, fallback, document/graph integration, and application routing.

Only layer 2 is an extraction candidate. Graft stays below it, including
authority over any later transport-only package factored from its workspace.
Inker stays above it. This is also the dependency direction: the neutral types
must not depend on Mere, Inker registries, or a specific wgpu version.

## What already looks like a neutral contract

Mere/Inker has the closest thing to the shared browser-surface shape:

- `mere/crates/inker/inker/src/surface_engine.rs:306` defines `SurfaceFrame` as
  raw native texture handle plus sync, size, format, and resource epoch. It is
  deliberately wgpu-free; the host imports on its own device.
- `mere/crates/inker/inker/src/surface_engine.rs:698` defines
  `WebSurfaceCapabilities` with frame transport, cookies, script, pointer,
  document controls, PDF, downloads, devtools, popups, drag/drop, IME,
  accessibility, and degradation reasons.
- `mere/crates/inker/inker/src/surface_engine.rs:870` documents
  `SurfaceProducer` as not `Send`, matching STA-bound WebView2 and
  main-thread-only WKWebView reality.
- `mere/crates/inker/inker/src/surface_engine.rs:942` defines `WebSurface` as
  the web-specific control plane layered over raw surface transport, with one
  ordered `poll_web_event` stream.

That is enough shape to avoid three unrelated browser-surface APIs if the
details are corrected.

## What must not be extracted

These pieces are Mere/Inker host policy, not neutral browser-surface API:

- `SurfaceEngineRegistry`, route decisions, and engine IDs.
- `EngineProfileBinding` as a persona/graph storage decision.
- fallback routing between static documents, Nematic, Scry, Graft, and Weld.
- document graph ingestion, projection, content custody, and app-level
  navigation history.

The neutral contract should describe how a host drives one browser surface and
imports its frame. It should not decide which engine the host chooses or how a
Mere graph stores the result.

## Mismatches to fix first

**Synchronous browser commands.** `WebSurface` currently has synchronous
`get_cookies_for_url` and `execute_script_with_result` methods at
`mere/crates/inker/inker/src/surface_engine.rs:1003` and `:1035`. That does not
fit CEF or WebKit well. Weld already exposes `request_script_result` /
`poll_script_result` and `request_cookies` / `poll_cookies` in
`welding/src/surface.rs:930` and `:958`; the neutral contract should follow
that request-id plus completion-event shape.

**Event order.** Inker's doc says filtered convenience pollers can discard
intervening events and are not part of the contract, but the Scry and Graft
adapters still synthesize the ordered stream by polling navigation before web
messages. A public contract needs one actual queue, not ordered-by-adapter
guesswork.

**Capability defaults.** `WebSurface::capabilities` currently defaults to
`WebSurfaceCapabilities::default()`. That is useful for tests but dangerous as
a public implementor path. Public producers should be required to return a
complete capability struct, with unsupported fields carrying explicit reasons.

**Dropped frame modes.** `scrying-engine` drops CPU RGBA, PNG snapshot, and
overlay-only frames at `mere/crates/inker/engines/scrying-engine/src/translation.rs:63`.
That is acceptable for today's composited-texture route, but it is not a
complete neutral surface contract until CPU and overlay outputs have named host
behavior.

**Borrowed versus transferred handles.** Inker already has
`FrameHandleOwnership`, but Graft's current safe Vulkan descriptor still carries
copyable consumed fds. The neutral contract must depend on Graft's corrected
owned/raw split, or it will preserve the bug at a higher layer.

**Feature-conditioned capability truth.** Scry's WPE capability struct says
unsupported while describing a working feature-enabled WPE path. Weld's static
probe must account for `cef-runtime`. A neutral contract that copies those
answers would become a shared lie.

## Recommended public shape

A future neutral surface-contract crate should be wgpu-free and
producer-neutral. The 2026-10-22 naming decision chooses `wgpu-cue` for the
project and wgpu-facing crate, with `webcue` for a separate wgpu-free core if
needed (see the dated cues below).
It should contain:

- `SurfaceFrame` as an owned transport frame from Graft's wgpu-free boundary,
  plus an explicit CPU frame and an explicit native-overlay outcome. It should
  not reproduce Inker's integer handles plus `Borrowed`/`Transferred` tag as
  its safe front door.
- `SurfaceProducer` as a single-owner, non-`Send` trait with non-blocking frame
  acquisition.
- `WebSurface` as an extension trait for navigation, focus, input, settings,
  cookies, script, capture, find, permissions, authentication, downloads, and
  devtools.
- `WebSurfaceEvent` as the single ordered event stream, including correlated
  completions for async commands.
- `WebSurfaceCapabilities` with no guessed defaults in production
  implementations.
- Correlation IDs minted by the caller and carried on both accepted commands
  and their completion/error events. Acceptance failure returns synchronously;
  accepted work settles exactly once in event order.
- Settings application results that say which fields were applied, rejected, or
  degraded. A blob-shaped `apply_settings` result cannot otherwise keep
  capability truth aligned with actual instance state.

It should not expose `wgpu::Texture`. Graft remains the native import crate;
Scry, Weld, and Servo adapters produce native frame descriptors and the host
imports them on its own `wgpu` device.

Today `grafting` aliases a feature-selected wgpu/wgpu-hal pair throughout its
public API. Depending on that crate directly would therefore make the neutral
contract choose the host's wgpu major. Before extraction, factor the owned
native-frame and synchronization vocabulary into a wgpu-free boundary owned by
the Graft repository, then let both `grafting` and the neutral contract depend
on it. Do not create a second, subtly different raw-handle vocabulary in Mere.

## Design constraints before extraction

- A public capability report is an instance observation, not an operating-system
  guess. Compile-time availability, construction success, selected transport,
  and runtime-observed acceleration are distinct facts.
- The default trait implementation for a supported operation must not block or
  fabricate support. Test stubs can use helpers, but production implementors
  must return a complete report.
- One queue owns web event order. Adapters may translate event values but may not
  create ordering by polling several backend-specific queues in priority order.
- A frame is either immediately available or it is not. Waiting for first paint
  is an explicit, bounded host operation outside the ordinary render-loop poll.
- Resource ownership must be structural. An integer plus a
  `Borrowed`/`Transferred` tag is useful at an FFI edge, but is not the safe
  public custody model for a descriptor that can be consumed or leaked.
- CPU snapshots and native child overlays need explicit host outcomes. If the
  v1 contract does not carry them, capabilities must reject them before a host
  begins acquiring frames.

## Migration path

1. In Mere/Inker, change result-bearing script and cookie reads to
   request/correlation/event APIs while keeping temporary compatibility helpers
   behind adapter-local blocking wrappers.
2. In Scry and Weld, expose complete capability structs that map one-to-one
   into Inker without default guessing.
3. In Graft, land the owned/raw resource API so all higher-level contracts can
   name transferred versus borrowed handles correctly.
4. Factor Graft's owned frame/sync vocabulary behind a wgpu-free package or
   module boundary without changing its ownership authority.
5. Extract the proven browser-control types and traits into the neutral crate;
   do not extract Inker's current file wholesale.
6. Make `scrying-engine`, `weld-engine`, and `graft-engine` depend on the new
   API crate instead of re-owning the bridge vocabulary.
7. Only after one external consumer uses Scry and Weld through the extracted
   contract should Scry/Weld/Graft consider implementing it directly.

## Done conditions

- `cargo test -p inker -p scrying-engine -p weld-engine -p graft-engine` passes
  with the async event contract.
- Adapter tests prove a script-result completion and a cookie-read completion
  cannot overtake earlier navigation or page-message events.
- Capability tests fail if any backend inherits a default capability answer.
- A fresh consumer can drive at least two engines through the extracted crate
  without depending on Mere.
- The DX12, Metal, and Vulkan release consumer still imports browser frames
  through Graft after the extraction.

## Migration progress

- 2026-09-04: Inker added caller-minted `WebRequestId` values,
  `request_script_result` / `request_cookies_for_url` acceptance methods, and
  correlated `ScriptCompleted` / `CookiesCompleted` variants on the single
  `WebSurfaceEvent` queue. A focused ordering test proves completions remain
  behind an earlier navigation event and retain the caller's ids.
- 2026-09-04: The blocking script/cookie methods are gone from Inker and its
  Graft, Scry, and Weld adapter traits. Pelt's only caller was a WebView
  readiness probe; its fixture now posts `pelt.surface.ready` through the
  page-to-host message channel and Pelt drains that from the ordered event
  stream.
- 2026-09-04: `scrying` 0.7.1 added one native ordered event stream for
  navigation, page messages, script completions, and cookie completions on
  WebView2, WKWebView, and WPE. `scrying-engine` now forwards caller ids without
  reminting them, consumes only that queue, maps completion payloads back into
  Inker, and reports backend-specific cookie-read limits instead of a hopeful
  shared default. The registry-backed adapter suite passed all 18 tests against
  the crates.io package. The registry-backed wider receipt passed 99 Inker
  tests, 24 adapter tests, and six Pelt tests with
  `cargo test -p inker -p scrying-engine -p weld-engine -p graft-engine -p pelt-core -j 1 --locked`.
- 2026-09-04: Direct Weld binding was assessed against Welding 0.14.1 and
  Turnstone's live factory. It must start in wgpu-weld: the public producer
  trait has separate navigation/message/result queues, and only Windows exposes
  a native-frame take path. Mere cannot reconstruct native callback order or
  honest Metal/Vulkan frame custody above that boundary. `WeldProducerFactory`
  remains the right host seam for CEF subprocess bootstrap, runtime lifetime,
  profile policy, and host import caches. After Weld exposes ordered events and
  cross-platform owned native frames, a version-pinned opt-in adapter can own
  the mechanical translation without absorbing those host policies.
- 2026-09-04: Welding 0.15 now satisfies those prerequisites. The opt-in
  `welding-0-15` adapter is pinned to Weld commit
  `4784d07c4064195c33136b9b91c8231913f08e06`, consumes its single ordered
  event queue, preserves caller ids across the full `u64` range, and carries
  the complete owned `NativeFrame` through Inker's type-erased payload until
  the host importer consumes it. The adapter requires the exact
  `CefSurfaceConfig` used by the host so configured permission, auth, download,
  background, and actual frame-mode facts are not replaced by build-time
  guesses. CEF bootstrap, runtime lifetime, profiles, producer construction,
  and import caches remain in `WeldProducerFactory`. Focused receipts passed
  six feature-enabled tests, three feature-disabled tests, and target-only
  clippy with warnings denied.
- Pelt's Windows consumer now recovers Scry's owned native-frame payload and
  imports it with Scry's host-device importer and the same shared fence
  synchronizer given to WebView2. The mixed headed receipt passed at 1280x800
  and 960x640 with three owned imports, three fence waits, host composition,
  and the page-message readiness event (artifact digest `85d1b0ba8a86778f`).
- Focused tests passed for Inker, Graft/Scry/Weld adapters, and Pelt core at
  this checkpoint. Graft subsequently published its wgpu-free `grafting-frame`
  0.1.0 custody package, recorded in
  `wgpu-graft/design_docs/2026-09-03_wgpu_triplet_release_plan.md`. Producer
  adoption and full importer conversion remain open, along with browser-control
  extraction and a combined demo with real Graft and Weld factories.
- 2026-10-05: **B0 source audit and bounded capability correction.**
  `crates/system/surface-api/{lib.rs,surface.rs,settings.rs}` owns application
  settings and provider/surface discovery. Browser frame/input/command/event
  types still live in `crates/inker/inker/src/surface_engine.rs`; the September
  status line incorrectly equated the two extractions and is corrected above.
  `scrying-engine/src/producer.rs` forwards navigation, legacy settings,
  pointer/keyboard/focus, cookie writes/reads, result-bearing scripts, and the
  ordered backend event stream. It does not override document find, typed
  `set_page_zoom`, permission answers, or authentication answers. Its previous
  capability projection nevertheless described find/zoom/PDF/permissions as
  partial and authentication/context menus as universally supported.
  `scrying-engine/src/translation.rs` now explicitly refuses find, typed zoom,
  PDF, permission/authentication decisions, IME geometry observability, and
  accessibility-tree projection at the Inker boundary. Legacy zoom and
  DevTools settings forwarding remain separate from the typed zoom outcome.
  Downloads and popup requests retain implemented partial event paths, with
  missing control/lifecycle or widget-surface projection stated explicitly and
  backend refusals/degradation retained. Context-menu target events are partial
  only for the verified WebView2/WKWebView sources; other backends report an
  explicit refusal. Script/cookie and input forwarding are unchanged.
  Focused regressions exercise default command refusals through the actual
  `ScryingProducer`, projected request payloads, and preserved backend limits.
  Root qualification passed all 21 library tests with
  `cargo test -p scrying-engine --lib --locked --offline --target-dir C:/t/cargo-targets/mere -j 2`.
  This source correction adds no new browser operation or hardware receipt.

## Remaining consumer gates (2026-10-05)

- Implement and prove actual commands before promoting an adapter capability:
  retained find results, typed zoom outcomes, capture correlation, permission
  and authentication decisions, download lifecycle, and inspection channels.
- Keep Scry's CPU/PNG/overlay frame-mode handling as an explicit unresolved
  transport seam: `map_frame` still drops those modes. B0's operation-capability
  correction does not implement their host outcome.
- Adopt the direct Weld adapter in a real host with matching package identity,
  owned import, exact instance config, and host-specific limitations. Turnstone's
  consumer work is tracked in
  `turnstone/design_docs/2026-08-03_user_agent_taxonomy_plan.md`.
- Prove a real Servo composite host and native event order; Graft's current
  host seam still polls navigation and messages separately.
- Require fresh two-engine external-consumer proof before browser-control
  extraction or a unified public browser-contract claim. Preserve the completed
  registry triplet release as its own, narrower qualification.

## Four producers and two frame directions (2026-10-22)

Source read at Mere `7136592c`, Genet `729414db`, Scry `36bac85`, Weld
`776ed44`, Graft `574ebc4` and Turnstone `cfb2904`. These are cues: what is
wrong and what fixing it would improve. They do not set an order.

**Native Genet is already a producer.** Graphshell's `ContentProducer`
(`ports/graphshell/src/retained_content/producer.rs:20`) paints a retained
Pelt document on the embedding host's device and exposes its semantics and
actions, and Pelt core has an embeddable controller and routed content API.
What Genet lacks is the browser-control face that hosts use for Scry, Weld
and Servo: navigation, one ordered event stream, correlated requests and a
complete capability report. A packaged Genet producer (a "wgpu Genet") would
mostly assemble pieces that exist: the document session, the render host and
the Graphshell producer. Because Mere and Genet own both sides of it, it is
the one producer where the contract's hard parts can be proven without a
supplier in the way: the full IME lifecycle, accessibility semantics and
actions, and a frame that is genuinely non-blocking.

**Two frame directions.** The contracts move frames opposite ways.

- Inker `SurfaceProducer` (`crates/inker/inker/src/surface_engine.rs:985`):
  the producer renders on its own device or process and hands over a
  `SurfaceFrame` that the host imports through Graft. Scry, Weld and Servo
  work this way.
- Cambium `TextureProducer`
  (`crates/cambium/cambium-rootstock/src/producer.rs:123`): the host lends its
  `wgpu::Device`, queue and `RenderCore`, and the producer renders a
  `ProducedTexture` on them. It also carries `portable_scene`, `semantics`,
  `act`, `suspend` and `retire`. Native Genet works this way.

Sending Genet through native export and import would introduce an
unnecessary custody and import boundary, with its synchronization
requirements, and could add copying depending on the transport; its existing
host-device path should remain directly representable. External engines
cannot render on a lent device. A
four-producer contract therefore needs a transport that names both: an owned
native frame under Graft's `grafting-frame` custody, or rendering into a
host-lent device. One control plane can sit over both. Pelt already does this
inside one application: `PeltContent` (`ports/pelt/core/README.md`) puts a
document lane and a live web-engine surface lane behind one command, input,
frame and routing API. It is the closest existing precedent for a front that
spans both directions, although it is Pelt's API rather than a neutral one. The lent-device path
names a wgpu major by construction, so it belongs in a wgpu-specific adapter
while the neutral types stay wgpu-free, as the recommended shape above
requires.

Accessibility has two producer-facing forms with different semantics.
`TextureProducer` describes the slot's current state through `semantics` and
carries actions through `act`, with no snapshot or transaction guarantee
between reads. `SurfaceProducer` (`surface_engine.rs:1083`) has activation
returning a root tree identity, ordered tree updates via
`poll_accessibility_update`, resynchronization that retires the old root,
and typed actions the host validates against tree, node, generation and
advertised actions. Turnstone's Servo producer implements all four and
refuses unadvertised actions; `scrying-engine` and `weld-engine` keep the
trait defaults. The gap is overlapping channels with different identity,
update and lifecycle rules, not a missing one. A shared contract that keeps
the stronger guarantees and makes unwired backends explicit would let one
host accessibility path serve every producer. Browser controls exist only
on `WebSurface`.

**Five overlapping contracts.**

| Contract | Home | Shape | Overlap |
| --- | --- | --- | --- |
| `ShellEngine` / `ShellEngineCapabilities` | Genet `components/genet-host-api` | Five booleans (JavaScript, WebDriver, DevTools, WebGPU, WebXR) for diagnostic Pelt | A coarse capability report beside `WebSurfaceCapabilities` |
| `document-session-api` | Genet `components/shared/document-session-api` | Retained sessions; `SessionIme` and `SessionInput` (`session_engine.rs:421`, `:439`); `DocumentCapabilities`; accessibility; capture | `DocumentCapabilities` is already shared: Inker embeds it as `WebSurfaceCapabilities.document` |
| `SurfaceProducer` / `WebSurface` | Mere `crates/inker/inker/src/surface_engine.rs` | Import-direction frames, browser controls, ordered events, ordered accessibility tree updates, `WebSurfaceCapabilities` | The de facto shared browser contract |
| `TextureProducer` | Mere `crates/cambium/cambium-rootstock` | Host-device frames, scene export, semantics and actions | A second accessibility form, read as current state rather than an ordered stream |
| Supplier traits | `scrying` `WebSurfaceProducer`; `welding` `CefSurfaceProducer` | Each has its own `NativeFrame`, events and capability struct | Translated by `scrying-engine` and `weld-engine`. Graft has no web-control trait; Servo's lives in Turnstone `src/shell/servo.rs` |

What the overlap costs:

- Three capability vocabularies (`ShellEngineCapabilities`,
  `WebSurfaceCapabilities` and each supplier's struct) can disagree, and only
  the middle one reaches Turnstone's inspector.
- Scry, Weld and Graft each define a native-frame type beside the published
  `grafting-frame`.
- IME is split four ways. Genet's `SessionIme` carries preedit with selection
  and commit. Weld exposes composition set/commit/finish/cancel and geometry.
  Scry only observes editable focus and caret. Inker's surface keyboard
  envelope carries no composition, and `scrying-engine` turns Scry's
  text-input events into diagnostics without projecting composition or caret
  geometry. Genet's `SessionInput` (key, keyboard-produced text, independent
  text, IME) is the richest of the four and the natural source for shared
  input and IME types.
- Two earlier mismatches still hold. Scry's default `try_acquire_frame` calls
  the blocking `acquire_frame` (`scrying/src/lib.rs:1549`), against "a frame
  is either immediately available or it is not". `WebSurface::capabilities`
  still has a default, and Turnstone's Servo report fills its unstated fields
  from it (`turnstone/src/shell/servo.rs:744`).

**Naming (2026-10-22, with Mark).** `wgpu-cue` is the chosen project and
wgpu-facing crate name. It places the shared embedding contract beside
`wgpu-graft`, `wgpu-scry` and `wgpu-weld`, with "cue" covering the inputs,
commands, events and readiness exchanged between producer and host. If the
dependency boundary needs a separately packaged wgpu-free core, its name is
`webcue`. That core would carry the neutral control vocabulary without
selecting a wgpu major; the wgpu-facing package would carry GPU-specific
presentation integration. The earlier `surface-engine-api` and
`web-surface-api` names were descriptive placeholders. This naming decision
does not settle the repository home or require a separate core package.

**Where a neutral crate could live.** The constraints above still hold: the
crate is wgpu-free, does not depend on Mere or Inker registries, and sits
above Graft. Licensing does not decide it, because Mere has been MPL-2.0
like the rest of the family since `e9aadb4ac` (2026-08-27).

- *Genet, as a dependency-free component beside `genet-host-api`.* Genet
  already owns `DocumentCapabilities` and the richest input vocabulary, and
  Inker depends on `document-session-api` today. The cost is that Scry, Weld
  and Servo consumers would depend on a Genet-repository crate, making Genet
  the owner of the external engines' contract.
- *The Graft repository, beside `grafting-frame`.* Graft already owns custody,
  and Scry and Weld already depend on it. The cost is that browser control is
  not Graft's subject; the architectural ruling above keeps Graft below the
  protocol.
- *A repository of its own.* It is neutral for all four producers. The cost
  is one more release train in a family that releases in one breath.
- *Inker, as today.* The dependency-direction ruling rules this out for the
  neutral types, and an external host would have to pull Mere.

Any home other than Inker changes this document's ruling that Inker owns
`WebSurface`, and the ownership note in Genet's
`docs/2026-08-14_web_platform_host_contract_plan.md` with it.

**Capability reports as the browser taxonomy.** A report describes a live
producer configuration, not an engine name. A useful one distinguishes
backend identity and version; platform and enabled build features; the
selected frame direction and transport; supported, partial and unavailable
operations with reasons; and runtime observations that confirm or degrade
the initial probe. Scry needs this most, because one name covers WebView2,
WKWebView, WebKitGTK and WPE. CEF's statically plausible acceleration is not
observed acceleration. Turnstone's inspector
(`turnstone/src/inspector_controls.rs:80`) shows the four
`DocumentCapabilities` controls out of roughly twenty
`WebSurfaceCapabilities` fields, so the full report is not yet the taxonomy
users see.

## Recorded choices and remaining cues

- **The name and home are settled.** `wgpu-cue` lives in its own repository;
  `webcue` is its published wgpu-free core. Genet and Inker supply adapters.
  Keeping these choices visible avoids reopening package placement as a missing
  architecture decision.
- **Both frame directions are in the contract.** `TextureSource` renders on the
  host's device; `FrameSource` hands over move-owned native or CPU frames.
  The host's presenter checks scope, extent, device identity and revisions.
  CPU RGBA is implemented; native child overlays still need an explicit host
  lifecycle rather than an invented compositable texture.
- **Synchronous conveniences still need a deliberate home.** Moving blocking
  capture/import helpers into an adapter keeps a shared UI call from silently
  acquiring a device wait. Any convenience API that blocks needs to say so.
- **Reports describe the installed binding.** The full supplier API, the
  shared subset and native extensions can differ. A complete live report makes
  those limits inspectable without treating an engine name as a feature promise.

## Shared-contract receiving (2026-10-23)

- **The retained session stays with its owner.** Inker's `SessionSurface`
  adapts the canonical document session already held by Pelt. Genet keeps its
  admitted resources, fonts and document state; acquisition, routing and history
  stay with the host. Another static HTML session would have lost those owners.
- **Retained input has one dispatch and observation mapping.** Webcue supplies
  the canonical dispatch plus ordered input-result, cursor, navigation and form
  observations. Inker and the standalone Genet adapter use those functions.
  Input results preserve capture and editability; dedicated navigation/form
  events carry host policy once. Unrepresentable modifiers are refused.
- **Both frame directions reach Pelt's shared-content API.** Its source is a
  native frame reader or a host-device texture producer behind the same control
  object. Inker's browser binding requires an explicit owned frame conversion
  from the engine/host pair. Existing native browser extensions remain explicit;
  the shared subset reports unsupported operations rather than inheriting a
  supplier's theoretical capabilities.
- **Private quality state makes the host device shareable.** Genet and
  Graphshell use a private retained renderer on the host's existing device.
  Producers and the shell cannot consume one another's quality results. This
  adds renderer cache cost, while removing the synchronous checked raster wait
  from the migrated producer path. Content revisions stay with the retained
  receiver; allocation epochs survive renderer reconstruction.
- **A replacement has a final stream.** Pelt reserves bounded space for the old
  scope's accepted-request cancellations before replacing its session. A failed
  reservation retains the actual receiver. Semantic mutations invalidate old
  publication keys before another frame, and qualified GPU frames advance the
  capability report from constructed to observed.
- **Publication and qualification have separate bounds.** `webcue` and
  `wgpu-cue` 0.1.1 are published; `wgpu-cue-netrender` is a source adapter pinned
  to Cue `94ba59574838a70c8283d3f5d3cf3ddbb838cfc5`. Retained Genet input/IME,
  semantic invalidation and Metal pixels are exercised through Pelt and
  Graphshell. Native candidate-window placement, human OS AT, physical device
  loss and CEF/native-WebView frame bindings retain their own qualification.
  Concrete Servo and Genet have since passed the finite headed and offscreen
  Metal receiver on one host device; see the receiving plan and Turnstone's
  `docs/receipts/servo_cue_20261023/`. That receiver is separate from complete
  Turnstone product adoption and Linux/Vulkan qualification.

- **The placement pool can lend its retained receiver to Cue.** Pelt's shared
  renderer now borrows the controller already admitted by the pool. Routing,
  host loading, history, independent Reader appearances and semantic admission
  retain their owners. Rendering no longer requires consuming that controller
  into a second content container; private renderer cache cost remains visible.
- **Visibility belongs to the shared input boundary too.** Pool focus loss and
  pointer cancellation precede hiding. Hidden retained surfaces reject late
  shared input, while reopening the placement preserves the session and gives
  the appearance a new admission. A renderer cache never owns that lifetime.

- **A pending frame must not consume a later mutation's redraw.** The retained
  pool exposes a visual revision distinct from receiver lifetime and placement
  admission. Input, mutable resource access, runnable work and exact due timers
  can require a follow-up frame after an older submission completes. Idle
  layout preparation does not continually invalidate that submission.
