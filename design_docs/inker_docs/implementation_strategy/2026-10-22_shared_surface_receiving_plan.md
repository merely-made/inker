# Shared surface receiving

**Status (2026-10-23): bounded Genet/Pelt/Graphshell receiving landed; concrete native browser integration and platform qualification remain open.** Shared controls and retained Genet receiving
in Inker, Pelt and Graphshell. The neutral owner is `wgpu-cue`; `webcue` is its
renderer-free core. Existing browser-specific host extensions remain explicit.

## Targets and validation

### Retained control receiving

Inker adapts the already-owned canonical document session to Webcue. Pelt keeps
its acquisition, resource delivery, history and engine routing. Scope changes
on successful session replacement; rejected replacement retains the old life.
Input preserves capture, cursor, editable state, navigation and form effects.
Commands complete once in the ordered bounded stream. Done when retained Genet
receiving tests cover input/IME, capabilities, request correlation, retirement
and semantic publication/action invalidation through the shared API.

### Host-device receiving

Pelt and Graphshell expose that control object beside host-device textures.
Render polling must not wait for GPU completion. Private quality observations
must not be consumed by the embedding host or another producer. Done when a
real device receiving test proves pixels, pending/retry, two producers, input
redraw, resize and device/scope replacement; compile and portable scene tests
remain separate from native window and human assistive technology acceptance.

### Native browser receiving

Inker's platform engine adapters retain their richer browser extensions while
common controls move to the neutral surface. No capabilities may claim an
operation merely because its supplier provides it. Raw borrowed native handles
are not promoted to move-owned shared frames without a custody proof. Done
when existing browser tests and focused shared receiving tests pass without
losing advanced browser operations or changing the host's import obligations.

## Findings

- 2026-10-22: Pelt's document lane already owns the retained Genet session,
  admitted fonts/resources and host history. Constructing another static HTML
  adapter would discard those features. `ports/pelt/core/src/lib.rs` is the
  receiving owner; `ports/graphshell/src/retained_content/producer.rs` supplies
  the host-device texture and portable scene extension.
- 2026-10-22: Graphshell's existing texture producer calls the synchronous
  checked raster path. Genet's initial Cue adapter assumes exclusive quality
  polling on its RenderCore. Neither assumption qualifies multiple producers
  sharing the shell's render core. A renderer with private quality state on
  the same host device avoids cross-consumption without a new GPU device.
- 2026-10-22: The published shared core and Mere's Git Genet dependencies must
  resolve the same `document-session-api` source. A root registry patch is
  needed for type identity, alongside a receiving compile.

## Progress

- 2026-10-22: fetched Mere and Cue; clean primary checkouts preserved. Isolated
  implementation worktrees created. Scope and validation inventory recorded.

## Receiving continuation (2026-10-23)

The public core is `webcue = 0.1.1` and `wgpu-cue = 0.1.1`. The retained
renderer adapter is a Git supplier at Cue
`94ba59574838a70c8283d3f5d3cf3ddbb838cfc5`. Its dependencies name the registry
contracts, so a Git adapter cannot introduce a second copy of their traits.
Mere patches the registry document API to its existing Genet Git source;
`cargo metadata` confirms one `document-session-api`, one Webcue and one WgpuCue.
Git Genet receiving workspaces need the equivalent root patch; dependency patch
tables are not inherited.

Webcue's common dispatch and event mapping preserve canonical input results and
produce dedicated navigation/form observations. Consumers apply those effects
once. The browser binding retains its richer serialized platform callbacks and
refuses common operations that it cannot represent. It does not silently
upgrade native accessibility or composition support.

Pelt's `PeltSharedContent` presents either `Source::Native` or
`Source::HostDevice`. Its retained texture source can rebind renderer state
without replacing the session. Allocation epochs are monotonic across private
renderer reconstruction; content revisions belong to the retained session.
Graphshell keeps its portable scene extension, selected-content ownership and
native view callbacks while its texture producer uses the shared rasterizer.

- 2026-10-23: Inker, Pelt and the three engine-adapter suites passed 272 tests
  with the two GPU receiving tests explicitly excluded from the ordinary run.
  The explicit Pelt GPU run passed both tests, including two producers sharing
  one device, independent shell quality polling and same-scope renderer rebind.
- 2026-10-23: Graphshell's retained suite passed 15 tests, with its GPU test
  excluded; the explicit Genet/Graphshell GPU receiving test passed separately.
- 2026-10-23: the shared core passed 11 tests and both real-Metal presentation
  tests. Strict Clippy passed for the four changed Cue packages. The standalone
  Genet GPU fixture passed IME commit, stale semantic actions, straight-alpha
  pixels and 128 accepted navigation completions.
- 2026-10-23: the final published-dependency receiving run, after rebasing onto
  Mere `6dc420602`, passed 273 Inker/Pelt/native-adapter tests, both explicit
  Pelt GPU tests and all 16 Graphshell retained tests including its GPU case.
  The final browser regression proves an existing callback backlog cannot starve
  an accepted host command while callback order is preserved.
- 2026-10-23: the separate registry-only consumer passed nine API tests and both
  explicit Metal transport tests. Both downloaded crate archives match their
  registry checksums and source `94ba595`. The
  [receiving receipt](../../../support/receipts/inker-cue-20261023/qualification.json)
  retains exact source/lock identities, test logs and preservation bounds.
- 2026-10-23: this is not native-window, physical IME, human AT or physical
  GPU-reset qualification. The D2 audit retains its baseline snapshot digest
  mismatch and 39 missing records; this plan has a supplemental judgment.
  Existing workspace Clippy warnings remain; the new adapter modules introduce
  none. The owning broader native-browser phase remains open; the bounded
  Genet receiving targets above are implemented and qualified.

- 2026-10-23: current-main continuation on `a6d44855b` preserves its native
  publication/AccessKit patch beside the Cue document-API patch. The combined
  source again passes 273 Inker/Pelt/engine-adapter tests, both Pelt GPU tests,
  all 16 Graphshell retained tests including GPU receiving, and Clippy with the
  existing workspace warnings. Type identity remains singular, including the
  patched AccessKit 0.25.1 cohort.

- 2026-10-23: final integration `7039ba386` includes current-main density/input
  changes from `f76758b6b`. All 274 Inker/Pelt/native-adapter tests pass, both
  explicit Pelt Metal tests pass, and all 16 Graphshell retained tests pass
  including real GPU receiving. The adapter source and dependency lock match
  the qualified source above; the receipt retains both qualification cohorts.


## Concrete browser reports — 2026-10-23

**Status (2026-10-23): shared prerequisites landed; real Metal offscreen and
headed receiving pass.** Turnstone's actual Servo host forces an
independent Stop/inline-HTML refusal: the old navigation capability cannot
express those operations separately. `BrowserSurface::new_with_report` accepts
an explicit complete report, refuses claims outside its implemented subset,
and gates shared input before native dispatch. `PeltSharedContent::from_browser`
keeps that already-bound producer's report and scope. Independent refusals
preserve request IDs, existing receiving suites pass, and Turnstone's real
Metal receiver consumes the installed report. The same qualified binary also
passes the visible-window gate; native physical IME and human AT remain separate.

- 2026-10-23: Servo's public `WebView::page_zoom` supplies a real effective-value
  readback. Graft's producer seam forwards typed zoom only when its host
  implements it; the default refuses rather than turning settings into an
  invented applied value. Turnstone owns the concrete Servo implementation.

- 2026-10-23: the concrete-report and typed-Graft-zoom prerequisites pass
  251 Inker/Pelt/Graft tests and both explicit Pelt Metal tests. The native
  Servo consumer and pixel qualification are tracked in
  `turnstone/design_docs/2026-08-25_browser_surface_implementation_plan.md`.

- 2026-10-23: a concrete process turn must progress without frame acquisition.
  The browser binding installs a bounded engine turn on shared `advance`,
  including while hidden; retirement refuses further turns. This lets the
  Servo process advance through the shared control API instead of coupling
  navigation and callbacks to visible paint acquisition.

- 2026-10-23: 127 Inker library tests pass with the hidden engine-turn
  regression; the earlier 251-test receiving cohort and two Pelt Metal cases
  remain the prerequisite evidence.

- 2026-10-23: the real two-engine Metal receiver caught retained input still
  being admitted while hidden. Both shared input and synchronous retained
  dispatch now refuse before touching the session, with retirement retaining
  precedence. Commands continue advancing while hidden. The real Genet
  regression checks unchanged editor contents, no input result on refusal,
  reload progress and resumed input after showing. The complete focused
  Inker/Pelt/Graft cohort and both explicit Pelt Metal cases pass after this
  correction; the receiving host retains the failing native run as evidence.

- 2026-10-23: Turnstone `681deff` publishes the concrete Servo/Metal binding
  and finite same-host receiver. Real Genet and Servo frames, editing, typed
  zoom, hidden navigation progress without frame acquisition, resize, scope
  replacement and teardown pass offscreen on one producer device. Native
  custody survives producer destruction; a foreign device/queue refuses import.
  The [receipt](https://github.com/merely-made/turnstone/blob/681deff/docs/receipts/servo_cue_20261023/README.md)
  retains source/binary identity, all 28 capability fields, pixels and failed
  runs. This does not migrate the complete Turnstone product UI. Servo's report
  stays `Constructed`; host-import observation currently lives in the external
  receipt rather than feeding back into that report.

- 2026-10-23: Turnstone `b6983aa` closes the finite headed Metal receiving gate
  after unlock: a visible, focused window presents 30 frames on one producer
  device using the exact offscreen-qualified binary. The
  [headed receipt](https://github.com/merely-made/turnstone/blob/b6983aa/docs/receipts/servo_cue_20261023/headed/qualification.json)
  retains the complete same-host control/custody sequence. The driver also
  corrects a bundled-Bash empty-array launch failure. The full product UI,
  Linux/Vulkan, physical input and human AT remain separate boundaries.
