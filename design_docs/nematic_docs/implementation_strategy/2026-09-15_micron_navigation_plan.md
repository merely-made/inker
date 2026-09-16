# Micron Navigation Plan — anchors and collapsible sections

**Status (2026-09-16):** accepted; C1 and C1b landed, N1 next. Lane 2 of the
[smolweb fidelity plan](2026-07-01_smolweb_fidelity_plan.md) ("Document
navigation"). Lane 3 (forms) closed on 2026-09-13 with headed receipts; this
lane is the next user-visible conformance gap.

## Objective

Make Micron's in-document navigation real in both consumers: anchor links
scroll the current page, heading and explicit anchors resolve, collapsible
sections open and close under session control, and none of it issues a
transport request. Source bytes stay authoritative; a construct the plan does
not qualify keeps its source and a diagnostic, as today.

## What exists (verified 2026-09-15)

- **Syntax already retains the facts.** `nematic::micron::syntax` parses
  `LineKind::Heading { depth, initially_open, anchor }`, the anchor declaration
  `Span::Anchor { name, active }`, and `section_depth` per line. Heading slugs are
  derived (lowercase, non-alphanumeric runs to one hyphen), and every declaration,
  heading or explicit, competes for its name with the first winning: an explicit
  `` `:name `` after a heading with the same slug is recorded as an inactive
  duplicate, not a rename (`syntax.rs` ~711–731). `LinkEffect::Anchor` is the
  cross-page `anchor=` link modifier; an in-page `#name` or `#` link is an
  ordinary link whose target starts with `#`, which `resolve_target` rejects.
  Anchors inside table cells reserve their name but are not retained, and a `<`
  line parses as ordinary text at unchanged depth. *(Corrected 2026-09-16 by the
  N1 design pass; the earlier text had the rename and link-effect facts wrong.)*
- **Rendering drops them with a diagnostic.** `render.rs` lowers headings to
  `Block::Heading`, reports collapsible sections as "retained in syntax; shown
  expanded", anchor declarations as "native anchor scrolling is not
  implemented", and in-page links under a third diagnostic it shares with request
  links ("Micron request or anchor link retained…", `render.rs` ~271–277), which
  N1 has to split;
  `resolve_target(base, "#name")` returns `None`, so an anchor link currently
  becomes plain text with an unresolved diagnostic.
- **Consumers have no anchor path.** Turnstone's `nomadnet.rs` refuses
  unresolved aliases and activates only full native targets; Knot's preview
  renders Inker blocks through `blocks()` in `scroll_site.rs`. Knot's
  `document_folding.rs` folds *Knot* source outlines, not Micron sections, and
  must not be duplicated for Micron.
- **Stock semantics are captured for the ordinary cases**
  (`crates/nematic/nematic/tests/fixtures/micron/nomadnet-1.4.2/REFERENCE.md`,
  "Captured Guide syntax"; its original `C:/t` capture tree no longer exists):
  `>`-runs set depth; `` `+> `` / `` `-> `` headings fold to the next heading of
  equal or shallower depth and toggle on Enter, Space or mouse; explicit
  anchors are zero-width with ASCII letters, digits, `_`, `-`; every heading is
  an anchor by slug, first declaration wins; `` `[label`#name] `` scrolls the
  current page and `` `[label`#] `` jumps to the next heading; an external link
  may carry `anchor=name`. Fixture: `fixtures/guide-structure.mu`.
- **Captured since (C1 and C1b, 2026-09-16).** The spellings the 2026-09-12
  capture left open (duplicate and missing anchors, `#` past the last heading,
  links into closed sections, nested fold state, `<`, key toggling, in-page
  transport and anchor history) now have a stock UI receipt at
  `crates/nematic/nematic/tests/fixtures/micron/nomadnet-1.4.2/navigation/NAVIGATION_RECEIPT.md`.
  Its C1b section adds unnamed sections inside folds and before a `#` jump,
  `<<` and `< text` inside a fold, an explicit anchor on the line after a
  heading, two explicit anchors with the same name, a target inside two closed
  sections, and `anchor=` links to another page (present, missing, and inside a
  closed section). The C1 and C1b Findings below summarise both. The receipt
  still keeps source and diagnostic for the depth `<` and `<<` reach, for what
  `< text` is, and for which depth an unnamed line is compared with.
- **Instrument rebuilt (2026-09-16).** The 2026-09-13 `C:/t` scratch family was
  deleted, so C1 rebuilt stock `nomadnet` 1.4.2, `rns` 1.5.3 and `lxmf` 1.1.1 in
  a WSL-native venv under `/var/tmp`; the package metadata digest matches the
  2026-09-12 reference. `/var/tmp` is volatile, so the recipe and wheel digests
  are recorded in the receipt, and durable captures live under
  `Code/testing/mere/micron-navigation-20260916/`.

## Phases and done-conditions

### C1. Stock captures for the open spellings

**Status (2026-09-16): landed.** Stock NomadNet 1.4.2 (package metadata
identical to the 2026-09-12 reference) served and browsed 15 probe pages on a
TCP loopback pair. The observation table, the four spellings that remain "keep
source and diagnostic", and what was not captured are in
`crates/nematic/nematic/tests/fixtures/micron/nomadnet-1.4.2/navigation/NAVIGATION_RECEIPT.md`,
with page digests in `CAPTURE_MANIFEST.md` beside it. ANSI captures and node
logs are in `Code/testing/mere/micron-navigation-20260916/`. Micron tests still
pass offline (28 passed).

Controlled pages served by the stock daemon and rendered in the stock TUI
client, retained as ANSI captures plus a written observation table, black-box
throughout. Probes: two headings with the same slug; an explicit anchor that
duplicates a slug; a link to a missing anchor; `#` next-heading from the last
heading; a link whose target lies inside an initially closed section; a
closed section containing another collapsible heading; `<` at line start
inside a nested section; Enter/Space on a focused heading; whether Back after
an anchor jump leaves the page or restores the previous scroll.

Done when each probe has a capture and one sentence of observed behaviour in
`nematic/tests/fixtures/micron/nomadnet-1.4.2/`, and any spelling still
ambiguous after capture is listed as "keep source and diagnostic", not
interpreted.

### N1. Document model: fold extents and anchor resolution

In Nematic (owner of source interpretation), in syntax line space so it is
testable without Inker: each collapsible heading's fold extent runs to the next
heading of equal or shallower depth or to a `<` line, whichever comes first
(decision 5); anchor declarations are collected in order with duplicates marked
inactive, first declaration winning; `resolve_anchor(name)` returns the winning
declaration or nothing; `next_heading(from)` counts from the link's own line
(decision 6) and returns nothing past the last heading.

Lowering carries these as typed facts without changing any block (decision 7):
the lowered `EngineDocument` gains a navigation table listing anchor
declarations and fold extents as half-open ranges of top-level block indices,
plus the block count they were computed against so a consumer can detect stale
indices. In-page links lower to a new inline kind carrying the resolved target
block and fragment, kept apart from network links so `outgoing_links`, link
statements, clipping, gemtext link lines and `resolve_target` never see them.
Micron lowers one wrapped block per source line, so indices survive the
presentation wrappers; the unsupported-construct badge inserted at index 0 is
accounted for. HTML export renders the new inline as label text only
(decision 9). A `<` line gains its source-plus-diagnostic treatment, which it
lacks today. Missing anchors and `#` past the last heading stay silent no-ops,
as stock does.

Done when the model tests over `guide-structure.mu` and the C1 and C1b probe
pages assert extents, anchors, first-declaration-wins, the `<` stop and the
next-heading origin; the lowering tests assert the table and the in-page targets
by label, that `outgoing_links` excludes in-page links, that the three old
diagnostics are gone while request links still raise theirs, and that a serde
round trip keeps the table; and the Inker-family crates treat the new inline as
inert label text. No IO, no consumer change beyond the one-line default each
engine needs.

### P1. Shared presentation and session state

In Inker/document-canvas and document-lanes (owners of reusable presentation
and the retained viewport): a fold is a session-owned open/closed flag keyed
by the document identity plus the heading's line index; layout omits a closed
extent and marks the heading with its state; the heading is a hit target and
a keyboard target (Enter/Space) that toggles; an in-page link activation asks
the session to scroll the viewport to a block, with the target's closed
ancestors opened first, which C1 probe 05 confirmed stock does. Fold state survives
relayout and resize and is discarded when the source bytes change.

Done when document-lanes tests cover toggle by pointer and by keyboard,
scroll-to-anchor after reflow, state reset on source change, and the `smolweb`
streaming test still passes. Nothing here knows about NomadNet addresses.

### A1. Consumers

Turnstone: an in-page anchor link scrolls the focused page with **zero**
transport requests (asserted against the recorded fetch log), is a history
entry exactly when it changes the displayed address (decision 1), and a native
link carrying `anchor=name` fetches then scrolls. Knot: the Micron preview shows fold state
and anchor targets through the same shared lowering; toggling in the preview
is preview state, not document state, and never writes source. Both apps keep
their existing alias refusal and diagnostics for unqualified spellings.

Done when Turnstone's `nomadnet` and app tests and Knot's site and desktop
tests cover anchor scroll, next-heading jump, fold toggle and the zero-request
assertion, and the plan docs of both apps record the commands.

### R1. Receipts

Headed, both apps, against the stock daemon's pages: open a page with nested
closed sections, toggle by pointer and keyboard, follow `#name` and `#`, follow
an `anchor=` link from another page, resize, go Back. Captures compared side by
side with the stock TUI rendering of the same pages.

Done when the receipt lists each C1 probe with the stock observation beside
both apps' observations, and every difference is either fixed or written down
as a deliberate deviation.

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

## Out of scope

Forms and partial refresh, inline form widgets, media and directives (lane 4),
multi-segment Resource responses, section-exit `<` and the escape glyph beyond
capturing them, and any change to how source bytes are stored.

## Findings

- 2026-09-15: heading slug derivation and explicit-anchor rebinding are already
  in `syntax.rs`; the gap is entirely in lowering, session state and consumers.
- 2026-09-16: the `C:/t` scratch family for this workstream was deleted between
  2026-09-13 and 2026-09-16, taking the headed artifact tree, the stock daemon,
  the RNS virtualenvs and the shared isolated Cargo home. Nothing committed was
  lost and every cited test still runs, but artifact paths in the 2026-09-13
  receipts are now dead references, and any plan whose commands name
  `CARGO_HOME=C:/t/smolweb-next-20260913/cargo-home` needs that home refetched.
- 2026-09-16 (C1 receipt): in the stock client, a duplicate slug or a heading
  and explicit anchor sharing a name resolves to the earlier declaration; a
  missing anchor, and `#` below the last heading, are silent no-ops; a link into
  a closed section opens that section and scrolls to the target; opening a
  closed section shows nested headings in their authored state, and a reader's
  toggle survives closing and reopening the ancestor but not a reload.
- 2026-09-16 (C1 receipt): an in-page anchor jump issued zero requests, matching
  A1's assertion. The stock client also serves URL reopens and Back/Forward from
  a page cache with zero requests, so a zero count is not by itself proof that
  nothing was re-rendered. The jump added no fragment to the stock address row,
  and Back left the page; decision 1 differs from this on purpose.
- 2026-09-16 (C1 receipt): a `<` line changed stock rendering in a way N1's
  extent rule does not describe. With the collapsible heading closed, the line
  after `<` stayed visible, from depth one and from depth two inside a closed
  depth-one fold. The receipt keeps `<` as source plus diagnostic; N1 has to
  decide how a fold extent treats that line without promoting `<` to a rule.
- 2026-09-16 (C1b receipt): rules N1 needs beyond C1. **Unnamed sections:** a
  bare `>` run ends a closed fold only when it is shallower than the fold's
  heading; at equal or greater depth the lines after it stay inside, unlike a
  named heading at equal depth (C1 probe 6a), and `#` skips it for the next named
  heading. `syntax.rs` (~227–236) parses a bare run as `LineKind::Heading` with
  no anchor, so N1's extent and `next_heading` must not treat it as a heading.
  **Extent stops:** `<`, `<<` and `< text` each end the extent before their own
  line, whether first in the fold or after body; `< text` renders its text as a
  body row outside the fold, the other two make no row. **Cross-page
  `anchor=`:** one request for the path without the field, a landing identical
  to the target page's own in-page jump, and a closed section around the target
  opened. A missing target loads at the top with `Unknown anchor: #name` in the
  status row, unlike the silent in-page case. Back from a target returned from
  cache, but Forward onto a page reached by `anchor=` re-requested it, where
  C1's plain-page Forward came from cache. The stock
  address row shows the field spelling (`` …mu`anchor=name ``), which decision 1
  replaces with `#name`. **Anchor precedence:** an explicit anchor on the line
  after a heading never renames it. With the slug's name, the heading row wins
  as the first declaration; with another name, both resolve (the slug to the
  heading row, the explicit name to the next row). Duplicate explicit anchors
  resolve to the first, and a target inside two closed folds opens both.

## Progress

- 2026-09-15: plan drafted after the forms lane closed; nothing implemented.
- 2026-09-16: four decisions settled with Mark; the history rule is
  address-driven rather than gesture-driven. C1 started.
- 2026-09-16: C1 landed. Instrument rebuilt under WSL (`/var/tmp`, pinned
  nomadnet 1.4.2 / rns 1.5.3 / lxmf 1.1.1, metadata digest matches the
  reference); all ten probes captured with the stock daemon's own request log.
  Probes 06 and 07 were split mid-capture when the first pages proved
  non-discriminating. Committed `navigation/` fixtures, receipt and manifest.
- 2026-09-16: decisions 5 (`<` ends a fold extent) and 6 (`#` counts from the
  link's line) settled with Mark from the C1 results. N1 starts with a read-only
  design pass on how typed facts enter Inker's shared blocks.
- 2026-09-16: N1 design pass (read-only, throwaway build over every fixture)
  chose a navigation table plus an in-page inline kind, corrected three
  statements in this plan, and flagged for P1 that streaming re-lowers every
  received prefix, so fold state cannot simply be discarded whenever the source
  bytes change. Decisions 7–9 settled with Mark; C1b capture batch next.
- 2026-09-16: C1b landed. C1's instrument was reused and re-verified (metadata
  digest matches) with fresh profiles on a new loopback port. Probes 11 to 17
  were captured with the node's own request log, and all 17 requests are
  accounted for. Probe 11c was added mid-capture to test the unnamed-section
  rule at depths one and two. Committed 14 probe pages, the receipt's C1b
  section and the manifest digests; Micron tests still pass offline (28 passed).
  N1 next.
