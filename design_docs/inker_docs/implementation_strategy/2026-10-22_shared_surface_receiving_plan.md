# Shared surface receiving

**Status (2026-10-23): in progress; published-dependency and merged-main checks running.** Shared controls and retained Genet receiving
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
- 2026-10-23: final published-dependency, merged-main and independent-registry
  checks are running. This is not a native-window, physical IME, human AT or
  physical GPU-reset qualification. The D2 audit retains its baseline snapshot
  digest mismatch and 39 missing records; this plan has a supplemental judgment.
