# Micron Navigation Plan — anchors and collapsible sections

**Status (2026-09-16):** accepted; C1 in progress. Lane 2 of the
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
  `LineKind::Heading { depth, initially_open, anchor }`, `Span::Anchor { name, active }`,
  `LinkEffect::Anchor(name)` and `section_depth` per line. Heading slugs are
  derived (lowercase, non-alphanumeric runs to one hyphen) and an explicit
  `` `:name `` on the following line rebinds the heading
  (`syntax.rs` ~231–300, tests ~714–760).
- **Rendering drops them with a diagnostic.** `render.rs` lowers headings to
  `Block::Heading`, reports collapsible sections as "retained in syntax; shown
  expanded" and anchors as "native anchor scrolling is not implemented";
  `resolve_target(base, "#name")` returns `None`, so an anchor link currently
  becomes plain text with an unresolved diagnostic.
- **Consumers have no anchor path.** Turnstone's `nomadnet.rs` refuses
  unresolved aliases and activates only full native targets; Knot's preview
  renders Inker blocks through `blocks()` in `scroll_site.rs`. Knot's
  `document_folding.rs` folds *Knot* source outlines, not Micron sections, and
  must not be duplicated for Micron.
- **Stock semantics are captured for the ordinary cases**
  (`C:\t\micron-reference-20260912\REFERENCE.md`, "Captured Guide syntax"):
  `>`-runs set depth; `` `+> `` / `` `-> `` headings fold to the next heading of
  equal or shallower depth and toggle on Enter, Space or mouse; explicit
  anchors are zero-width with ASCII letters, digits, `_`, `-`; every heading is
  an anchor by slug, first declaration wins; `` `[label`#name] `` scrolls the
  current page and `` `[label`#] `` jumps to the next heading; an external link
  may carry `anchor=name`. Fixture: `fixtures/guide-structure.mu`.
- **Still uncaptured** (same file, "next probes"): duplicate and missing anchor
  behaviour, a link into a closed section, the `<` section-exit token, the
  escape glyph, and whether an anchor jump is a history step.
- **Instruments must be rebuilt (2026-09-16).** The whole 2026-09-13 scratch
  family under `C:/t` is gone: the headed artifact tree, the stock `nomadnet`
  1.4.2 daemon under WSL, both RNS virtualenvs, the earlier reference-capture
  tree and the isolated Cargo home. The durable reference capture survives in
  this repo at `crates/nematic/nematic/tests/fixtures/micron/nomadnet-1.4.2/`,
  but its ANSI TUI capture does not. Turnstone still self-drives through
  `TURNSTONE_SCENARIO`; Knot's guarded input script was in the deleted tree.
  C1 therefore begins by standing a stock daemon and a capture client back up.

## Phases and done-conditions

### C1. Stock captures for the open spellings

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

In Nematic (owner of source interpretation): compute each collapsible
heading's fold extent (line range to the next heading of equal or shallower
depth), expose `anchors()` as an ordered map from name to first line index
with duplicates recorded, and `resolve_anchor(name)` / `next_heading(from)`
following the captured rules. Lower these into the shared presentation as
typed facts rather than diagnostics: `Block::Heading` gains an anchor id and
an optional `Collapsible { initially_open, extent }`, and `Span::Anchor` /
`LinkEffect::Anchor` lower to an in-page link kind that Inker keeps distinct
from network links. `resolve_target` keeps returning `None` for `#name`;
in-page targets never enter the network resolver.

Done when the guide-structure fixture and the C1 fixtures round-trip through
parse → lower with extents and anchors asserted, duplicate/missing cases follow
the captured behaviour, and the two existing diagnostics for anchors and
collapsibles are replaced by the typed facts. No IO, no consumer change.

### P1. Shared presentation and session state

In Inker/document-canvas and document-lanes (owners of reusable presentation
and the retained viewport): a fold is a session-owned open/closed flag keyed
by the document identity plus the heading's line index; layout omits a closed
extent and marks the heading with its state; the heading is a hit target and
a keyboard target (Enter/Space) that toggles; an in-page link activation asks
the session to scroll the viewport to a block, with the target's closed
ancestors opened first when C1 shows stock does so. Fold state survives
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

## Progress

- 2026-09-15: plan drafted after the forms lane closed; nothing implemented.
- 2026-09-16: four decisions settled with Mark; the history rule is
  address-driven rather than gesture-driven. C1 started.
