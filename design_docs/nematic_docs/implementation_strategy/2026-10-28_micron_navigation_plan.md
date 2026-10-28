# Micron navigation: Turnstone's anchor and fold half, and the headed receipts

**Status:** 2026-10-28: cut from `2026-09-15_micron_navigation_plan.md`, archived in `archive_docs/2026-10-28/`. Built on 2026-09-16: C1 and C1b (stock captures), N1 (the document model: fold extents and anchor resolution), P1 (shared presentation and session state), P1b (shared style tokens), P1c (the `Glyph(String)` adornment), Cambium's public scroll request, and A1's Knot half. Open: A1's Turnstone half, which has not started (Turnstone's main still respawns a session for every Micron prefix), and R1, the headed receipts in both apps.

**For:** Making Micron's in-document navigation real in both consumers: anchor links scroll the current page, headings and explicit anchors resolve, collapsible sections open and close under session control, and none of it issues a transport request. Source bytes stay authoritative; a construct this plan does not qualify keeps its source and a diagnostic. It is lane 2 of the smolweb fidelity plan ("Document navigation"), settled by the nineteen decisions of 2026-09-16 below.
Not here: forms and partial refresh, inline form widgets, media and directives (lane 4) and multi-segment Resource responses belong to `2026-10-28_smolweb_fidelity_plan.md`. Out of scope as the source set it, with no owner: section-exit `<` and the escape glyph beyond capturing them, and any change to how source bytes are stored.

Receipts from this plan's runs go to `archive_docs/<date>/`, never into this
plan (DOC_POLICY §8). Evidence that matters is a test in the repository or a
line in Findings.

## Constraints

From the Micron navigation plan (`archive_docs/2026-10-28/2026-09-15_micron_navigation_plan.md`):
- The Decisions section below binds, in particular: an anchor jump is a history entry exactly when it changes the displayed address, and the fragment is display and history state only, stripped before any request (1); fold state is session-only and keyed by the heading's source line and occurrence (2, 10, 11); navigation facts travel in a table on the lowered document, not on blocks (7); keyboard focus reaches every interactive region with a visible, configurable indicator, a deliberate deviation from stock (12).
- Opening a local anchor issues zero transport requests (:12-18, 313-314, 349-352).
- Fixes the Mere bump breaks land in the consuming app; a fix that would edit another session's still-active Mere lane is reported to Mark (decision 16, :437-440).
- The stock TUI is not the authority for our address bar (decision 1, :380-381).

## Phases

### A1. Turnstone's half of the consumers
Turnstone switches Micron streaming from respawning a session per prefix to `replace_document`. This compiles either way, so a missed switch fails silently and must be asserted by a test. It also drains the in-page queue after a `Handled` click; routes keys through the session before the scroll bindings, where Space is bound to page down and so would never toggle a heading; scrolls an in-page anchor link on the focused page with zero transport requests, asserted against the recorded fetch log; makes the jump a history entry exactly when it changes the displayed address; loads a native link carrying `anchor=name` with one fetch, opens any closed sections around the target and scrolls, or loads at the top with a visible notice when the anchor is missing, as stock does; and teaches `parse_address` and `is_micron_address` a trailing fragment. Turnstone never calls `replace_document`, `take_in_page_navigations` or `reveal_anchor` today. Its pins already carry newer Mere and Knot revisions than A1's own, so the pinning order the plan set is overtaken. The Turnstone files are `src/shell/effects.rs` (the respawn), `src/shell/input.rs` and `src/shell/lens.rs` (the `Handled` arms), `src/shell/keys.rs` (`deliver_content_key`) and `src/nomadnet.rs`.
- Done when: Turnstone's `nomadnet` and app tests and Knot's site and desktop tests cover anchor scroll, next-heading jump, fold toggle, the streaming switch and the zero-request assertion, and both apps' plan docs record the commands.
- Needs: P1's session API, which is built: `SmolwebDocumentSession::replace_document` and `take_in_page_navigations`; `SmolwebDocument::reveal_anchor` (false for a missing anchor, which is where A1 shows the notice), `reveal_block`, `toggle_fold`, `folds` and `focused_interaction`; focus and Enter or Space through `input()` as `SessionInput::FocusMove` and `SessionInput::Key`. A replacement drops a reveal requested before layout, so an `anchor=` landing calls `reveal_anchor` again once a streamed prefix contains the target.
- Rulings: decisions 1, 2, 10, 11, 12 and 16 below.
- From: `archive_docs/2026-10-28/2026-09-15_micron_navigation_plan.md:304-326, 349-352, 334-347, 660-661`; `archive_docs/2026-10-28/2026-09-15_micron_navigation_plan.md:146-147, 375-378`

### R1. Headed receipts in both apps
Headed, both apps, against the stock daemon's pages: open a page with nested closed sections, toggle by pointer and keyboard, follow `#name` and `#`, follow an `anchor=` link from another page, resize, and go Back, with captures compared side by side against the stock TUI. Knot's outline paint and scroll have not been checked in a real window; they belong here. The stock daemon was rebuilt for C1 in a WSL venv under `/var/tmp`, which is volatile; the recipe and wheel digests are recorded in the checked-in receipt.
- Done when: the receipt lists each C1 probe with the stock observation beside both apps' observations, and every difference is either fixed or written down as a deliberate deviation.
- Needs: A1.
- Rulings: decisions 1, 12 and 14 below.
- From: `archive_docs/2026-10-28/2026-09-15_micron_navigation_plan.md:354-363, 297-298, 68-73`

## Open questions

1. Three spellings stay source plus diagnostic because no capture separates them: how far `<` and `<<` reach in depth, what `< text` is, and which depth an unnamed line is compared with (N1 compares with the fold heading's depth, recorded as a choice, not an observation). Capture them, or keep them unqualified? From: `archive_docs/2026-10-28/2026-09-15_micron_navigation_plan.md:66-67, 119-127`
2. Four items are deferred by the decisions, each with its trigger: per-page fold persistence (only if someone asks, decision 2), a viewport- or focus-relative `#` jump (a possible later session-layer refinement, decision 6), a live session UxTree projection (until an app consumes one, decision 13), and HTML export of `href="#name"` and heading ids (they arrive with the consumers, decision 9). Which of them are now wanted? From: `archive_docs/2026-10-28/2026-09-15_micron_navigation_plan.md:382-383, 396-397, 411-412, 429-430, 224-226`

## Decisions (settled 2026-09-16)

1. **An anchor jump is a history entry when it changes the displayed address.**
   The address is the authority, not the gesture. Following `#name` appends the
   fragment to the displayed address, so it is a history step and Back returns
   to the page without the fragment, at the scroll it had. Following `#` appends
   the *resolved* heading's anchor rather than the bare instruction, so the
   address names where the reader is. A native link carrying `anchor=name` lands
   on `dest:/page/other.mu#name`, one history step for the navigation itself.
   Toggling a fold changes no address and is never a history entry.
   Consequences to implement: Turnstone's `parse_address` and
   `is_micron_address` accept a trailing fragment; the fragment is display and
   history state only and is stripped before any request, which the
   zero-transport done-condition already asserts; `resolve_target` keeps
   returning `None` for an in-page target so the network resolver never sees
   one. C1 records what the stock client shows in its own address display, but
   the stock TUI is not the authority for our address bar.
2. **Fold state is session-only**, discarded when the source bytes change and
   when the page closes. No per-page persistence until someone asks for it.
3. **Knot's preview toggles fully**, with state held in the preview the way the
   form editor holds field values, never writing source. Extent computation
   lives once in Nematic rather than beside Knot's own source folding.
4. **C1 first and alone**, since N1's duplicate, missing and closed-section
   rules depend on it; then N1, P1, A1, R1, one repo at a time.

5. **A `<` line ends a fold extent** (settled 2026-09-16, after C1). This matches
   what stock renders in probe 07c: content after `<` stays visible when the
   fold above it closes. The extent stops at the `<` line, while `<` itself keeps
   its source and a diagnostic and acquires no depth meaning.
6. **`#` counts "next heading" from the link's own line** (settled 2026-09-16,
   after C1). Deterministic and testable in the model without UI state, and it
   agrees with every capture where the origin was observable. A viewport- or
   focus-relative jump stays a possible later session-layer refinement.

7. **Navigation facts travel in a table on the lowered document, not on
   blocks** (settled 2026-09-16, replacing the earlier fields-on-heading text).
   Fields on the heading block cannot hold explicit anchors on plain lines or in
   table cells and cost about 42 edits; a wrapper beside the presentation
   wrapper compiles but silently slips through four catch-all arms, including
   Turnstone's alias refusal and Knot's preview. The accepted risk is index
   staleness if a later step inserts or removes top-level blocks, which
   transclusion must clear or remap as it already does for provenance.
8. **A short capture batch (C1b) precedes N1 implementation** (settled
   2026-09-16): unnamed sections, `<<` and `< text` inside a fold, `anchor=`
   links to another page, an explicit anchor after a heading, duplicate explicit
   anchors, and a target inside two closed sections.
9. **HTML export renders in-page links as label text in N1** (settled
   2026-09-16). `href="#name"` and heading ids arrive with the consumers.

10. **A heading's fold state is keyed by its source text and occurrence**
    (settled 2026-09-16, refining decision 2). Streaming re-lowers the page for
    every received prefix and Knot re-lowers on every edit, so discarding all
    fold state whenever the source bytes change would reset folds constantly.
    State is keyed by the heading's source text plus which occurrence of that
    text it is: it survives streaming and edits elsewhere on the page, and a
    heading's own state drops only when that heading is edited. It remains
    session-only and is discarded when the page closes.

11. **The fold key uses the heading's source line** (settled 2026-09-16),
    reading decision 10 literally: any edit to that heading, including restyling
    it or flipping its `+`/`-` marker, drops its state.
12. **Keyboard focus reaches every interactive region with a visible,
    configurable indicator** (settled 2026-09-16). Stock NomadNet shows no focus
    indicator; this is a deliberate deviation.
13. **P1 carries accessibility facts in UxTree only** (settled 2026-09-16);
    a live session projection waits until an app consumes one.
14. **Resize after an anchor jump keeps pixel scroll** (settled 2026-09-16);
    stock was not captured for this.

15. **In-page links get their own adornment token** (settled 2026-09-16),
    defaulting to the same arrow as other links, so nothing changes on screen
    now but a theme can distinguish an in-page jump from leaving the page.
16. **A1 bumps Turnstone and Knot to current Mere main and fixes whatever the
    bump breaks** (settled 2026-09-16), even beyond P1's own edits. Fixes land in
    the consuming app; a fix that would edit another session's still-active Mere
    lane is reported to Mark instead.
17. **Fold marker defaults move into Inker** (settled 2026-09-16) as a shared
    style token read by document-canvas and by Knot's DOM preview, so the glyph
    choice has one source.

18. **Cambium gains a public scroll request** (settled 2026-09-16). Knot's
    Cambium host exposes no way for an app to scroll its own view (the rootstock
    scroll setters are crate-private), so an in-page link in Knot's preview could
    not reach its target. A public request to bring an element into view is added
    to Cambium in Mere, for every Cambium app, rather than making Knot's preview
    an app-local scroll container.
19. **Knot gets one visible focus rule for all its controls** (settled
    2026-09-16). Knot had no focus styling anywhere, so decision 12 could not be
    met in its preview alone; the fix covers every focusable control, reaching
    beyond the Micron lane on purpose.

## Findings

- 2026-09-16: the stock client resolves a duplicate slug, or a heading and explicit anchor sharing a name, to the earlier declaration; a missing in-page anchor and `#` below the last heading are silent no-ops; a link into a closed section opens it and scrolls; a reader's toggle survives closing and reopening the ancestor but not a reload. From: `archive_docs/2026-10-28/2026-09-15_micron_navigation_plan.md:472-477`
- 2026-09-16: the stock client serves URL reopens and Back or Forward from a page cache with zero requests, so a zero request count is not by itself proof that nothing was re-rendered. It adds no fragment to its address row and Back left the page after an anchor jump; decision 1 differs on purpose. A missing cross-page anchor loads at the top with `Unknown anchor: #name` in the status row, and Forward onto a page reached by `anchor=` re-requested it. From: `archive_docs/2026-10-28/2026-09-15_micron_navigation_plan.md:478-482, 488-508`
- 2026-09-16: Knot's in-page link opens the closed folds around its target, then asks Cambium to scroll the target's top-level block to the window top (`AppCtx::scroll_into_view` with `ScrollAlign::Start` or `Nearest`); a link without a target is inert, and no jump writes source, the address or the manifest. From: `archive_docs/2026-10-28/2026-09-15_micron_navigation_plan.md:285-298, 645-657`
- 2026-09-16: navigation indices go stale if blocks are inserted or removed after lowering; `resolve_transclusions` and `evaluate_blocks` clear the table when they splice, and blocks copied into another document stay inert. From: `archive_docs/2026-10-28/2026-09-15_micron_navigation_plan.md:399-406, 529-532`
- The stock receipts are checked in under `crates/nematic/nematic/tests/fixtures/micron/nomadnet-1.4.2/navigation/` (`NAVIGATION_RECEIPT.md`, with page digests in `CAPTURE_MANIFEST.md`); the ANSI captures are under `Code/testing/mere/micron-navigation-20260916/`. From: `archive_docs/2026-10-28/2026-09-15_micron_navigation_plan.md:56-73, 79-86`

## Progress

- 2026-10-28: cut from the Micron navigation plan; nothing new landed.
