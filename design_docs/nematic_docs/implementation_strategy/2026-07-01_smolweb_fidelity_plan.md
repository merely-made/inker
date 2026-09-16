# Smolweb Fidelity Plan — enrich the ASTs, trust in the native lane, bespoke only where boxes fail

**Date**: 2026-07-01
**Status**: planning (with Mark). Extends the
[native smolweb rendering plan](2026-06-27_native_smolweb_rendering_plan.md)
(that effort shipped: transport → parse → native themed render → scene → window,
with scroll, link nav, per-site/app theming). This one recovers the
spec-faithfulness the flavour-neutral pipeline collapses, and closes the security
posture the native lane currently drops.

> **Home refinement, 2026-08-03**: WS1's AST enrichment lands wherever the
> grammar lives at the time, per the
> smolweb home decision (`smolweb/design_docs/technical_architecture/2026-08-03_smolweb_home_decision.md`)
> (spec-accurate grammars follow the wire crates into the smolweb workspace;
> WS2/WS3 are implementation-side and stay in genet/cambium-nematic).

**The three principles** (Mark's call, 2026-07-01):

1. **Share the synonymous parts through the box substrate.** A gemtext paragraph
   and an HTML paragraph are the same box. Render them the same way and inherit
   typography, wrapping, selection, focus order, and a11y once, from genet-layout,
   instead of re-deriving them per format.
2. **Go bespoke (regime B) only where a format's line model genuinely is not
   box-shaped.** Gopher's fixed-width typed column is the first real case. A
   paragraph is never a reason to go bespoke, because it is the synonymous case.
3. **Enrich the parse ASTs, and manage trust in the native lane.** Most of what we
   collapse is lost at parse time, not paint time. And the native genet lane
   bypasses `Block`, so it drops the trust posture the card lane models.

---

## 1. What we collapse today (audit, verified against code)

### Micron grammar gate (2026-09-11)

At the initial 2026-09-11 gate, Micron was a planned Nematic engine. The public
Reticulum manual establishes that `.mu` pages and Markdown-to-Micron conversion
exist, but it does not define the line, link, inline-control, cache, or form
grammar. The household clean-room rule permits public prose and black-box
captures, while excluding NomadNet implementation source. Until a fixture
corpus records each interpreted construct and an independent capture confirms
its output, Nematic must retain Micron as raw source or an explicit unsupported
format. Do not infer bracket links, `#!c=` cache directives, terminal styles, or
form syntax from examples whose semantics are not independently established.
Any future parser belongs under `crates/nematic/nematic/src/`, with source
provenance and diagnostics for every unsupported construct; NomadNet transport
and dynamic page behavior remain separate adapter work.

**2026-09-11 partial preview receipt.** `MicronSubsetEngine` now lowers only
literal LF fixtures checked in beside its tests: `> Heading` as a level-one
heading, `---` as a divider, unmarked text, and the two captured same-line
inline forms for bold and italic. It attaches a partial-preview diagnostic.
Mixed controls, candidate links and tables, `#` lines, spacing variants,
CRLF, and empty physical lines remain inert `Preformatted` source; no link,
form, MIME, transport, or dynamic-page behavior is inferred. The fixtures
record stock Python and independent black-box renderer observations. Extending
this engine requires a new literal capture and a regression test first.

**2026-09-13 syntax and native-preview extension.** `MicronEngine`
(`nematic.micron`) now uses a separate, source-preserving syntax model.
`MicronSubsetEngine` remains available for legacy callers. `FileEngine` routes
`.mu` and `.micron` through the new engine; Knot and Turnstone use the same
engine for retained native previews. Native Micron files remain the author's
source, with no conversion on save.

The reference is the stock NomadNet **1.4.2** in-app Guide, accessed through the
isolated executable UI, plus original fixtures and opaque reference-client
outputs. See the checked-in
[capture manifest](../../../crates/nematic/nematic/tests/fixtures/micron/nomadnet-1.4.2/CAPTURE_MANIFEST.md)
and [activation receipt](../../../crates/nematic/nematic/tests/fixtures/micron/nomadnet-1.4.2/activation/ACTIVATION_RECEIPT.md).
The independent Go renderer's older behavior is not used to reject constructs
documented by the version-matched Guide.

| Layer | Implemented boundary | Still open |
| --- | --- | --- |
| Syntax | Persistent combined styles, three-hex colors, alignment, headings and section depth, collapse markers, explicit anchors, link components, field/partial source, headers, literal blocks, table blocks and image descriptors retain their source facts. | Full malformed-input compatibility, escape spelling, section-exit behavior and version differences need further stock-client captures. |
| Portable preview | Text, bold/italic, headings, literal text, dividers, ordinary native links and pipe-table cells lower to shared Inker blocks. Tables now have document-canvas geometry, wrapping and link hit regions. | Color, underline, alignment, indentation, folding, anchor scrolling, forms and partials require additional presentation or interaction support. Unsupported constructs have explicit diagnostics; images use an alt-text placeholder. |
| Link authority | Native same-node `:/page/...` resolves against a real destination. File-context aliases remain explicit for the host's active-site manifest to resolve. | An arbitrary file acquires no native destination. Bare and relative native targets remain unqualified. |
| Dynamic requests | Stock-client captures establish selected fields, empty and masked values, checkbox aggregation, edited radio state, fixed variables, Unicode and multiline text. A subsequent Retinue receipt verifies the native string-map value in both directions. | Retinue's bounded map API is the packet-sized transport prerequisite. Consumer field state, submission and partial refresh remain inert; outgoing request Resources are unsupported. |

This is broader syntax preservation and a usable native preview, **not full
Micron conformance**. Completing an interaction requires an independent
transaction receipt, typed request/state handling, native UI behavior and
tests in both consumers. Djinn's persistent serving and Tabard's theme exports
remain separate work.

Native table cells wrap within their allocated width. When the viewport cannot
fit even the minimum column widths, the packet reports its full overflow width;
the current Smolweb session still scrolls vertically only. Horizontal navigation
of those unusually wide tables remains open and is not covered by the layout
receipt.

Shared validation for this extension: 187 Nematic unit tests, 3 integration
tests and 58 document-canvas tests pass with Rust 1.97.1, `--locked --offline`,
an absolute workspace manifest and cwd `C:\`. The dependency tree resolves
Parley from Genet `3a7b50230d447f6fa7ed6921cba019f78347d932`; the shared checkout's
ancestor Cargo path overrides are not involved. A runtime UI appearance
receipt and full interactive conformance are separate gates.

The important finding: almost every semantic loss happens at the **flavour-neutral
parse ASTs**, before any view exists. The box rendering is mostly innocent. Switching
render regimes would recover none of it, because the data is already gone.

### Micron completion scope (2026-09-13)

**Status:** shared reading presentation and both consumer integrations implemented
and tested on 2026-09-13. The form lane's headed acceptance was taken the same
day against two independent instruments and is recorded in lane 3; reading
fidelity, navigation and refresh headed qualification remain. The remaining
behavior lanes below are scoped. Source interpretation, document presentation and session
interaction have separate evidence boundaries.

The first reading slice carries foreground/background RGB, underline, alignment
and section indentation in Inker's typed `Presented` wrappers. HTML export,
plain-text fallbacks, statement traversal and accessibility retain the wrapped
content. The canvas applies the style, and document-lanes retains horizontal
viewport movement and translated hit regions. `SourcePresentation::Reader`
suppresses source inline colors and underline through the shared style/session API;
alignment and indentation remain active. A persisted
Knot/Turnstone setting has not been qualified. Source bytes remain authoritative.
The consumer integration carries styled link children in Knot and traverses the
wrappers in Turnstone's NomadNet alias refusal before activation. Its application
checks are recorded in the respective consumer plans.
The final consumer code pins are Knot `cf3afe8` and Turnstone `a1d7834`, both
using Mere `dce5cc97` with Genet `101d9e9` and Netrender `3961aca`. Knot's desktop
library passes 54 tests with one existing ignored test; Turnstone passes the
five NomadNet and two smolweb input tests. These final gates use immutable
sources with `--locked --offline`, without development path redirects.

The consumer audit also carries wrapped text and links through clipping and
wrapped headings through Gloss and reader outlines. Outline order, heading levels
and existing source-index identities remain stable. Micron routing remains an
explicit host decision; clipping does not add a new default MIME route. Generic
wrapped media discovery and executable/transclusion expansion are outside this
reading slice; Micron does not acquire code execution through presentation.
The import library's 16 tests and Gloss's 6 tests pass against these traversal
changes. The clipping regression retains the native destination identity while
recovering styled text and links.
Document-lanes with `smolweb` enabled passes 23 unit tests and the streaming
integration test. Its viewport regression checks translation and activation of
a link visible before and after bounded scrolling, then verifies that scrolling
it fully out of view removes the host hit target.

Automated shared receipts: Nematic 190 tests plus 3 examples, document-canvas
61 tests, Inker 112 tests, document-lanes 7 tests and UxTree 8 tests pass with
`--offline --locked`. These include retained style, geometry, narrow-width horizontal
scroll and paint/hit-region translation. These are software receipts, not new
stock-NomadNet or headed two-app comparisons. Source table-width options still
produce a diagnostic and use reader geometry; folds, anchors, forms, refresh and
media remain the separate gates below.

1. **Reading fidelity first.** Nematic owns source interpretation and lowering;
   Inker/document-canvas own reusable presentation; document-lanes own the
   retained viewport. Carry captured color, underline, alignment and section
   depth through that boundary with an explicit reader override. Preserve Micron
   facts independently of the reader's chosen appearance. Extend the shared
   presentation contract only for concrete consumers, including its HTML export
   and plain-text fallback. Knot and Turnstone consume the same semantics.
   Done when the original combined-style and nested-section fixtures have
   independent stock-client comparisons and headed receipts in both apps,
   including keyboard focus, selection, contrast overrides and narrow widths.
   Horizontal scrolling must expose the full wide-table packet and preserve
   clipping and link hit coordinates after scrolling and resizing.
2. **Document navigation.** Preserve anchors and collapsible section boundaries
   through projection, then implement session-owned fold state and anchor
   activation. These actions are distinct from network navigation and submission.
   Done when duplicate/missing anchors, links into closed sections, keyboard
   activation, reflow and back navigation have explicit behavior and consumer
   receipts; opening a local anchor must issue zero transport requests. Further
   stock captures must establish ambiguous section-exit, escape, table and
   malformed-control behavior before the parser interprets those spellings.
3. **Forms: bounded consumers, headed acceptance taken.** The 2026-09-13
   [typed request receipt](../../../../retinue/design_docs/2026-09-13_nomadnet_go_resource_compression_receipt.md#typed-form-request-receipt)
   closes the packet-sized transport prerequisite: stock NomadNet and Retinue
   match native string maps in both directions for defaults, edited Unicode and
   multiline text, checkbox/radio state, selected fields and fixed variables.
   Retinue's `StringMapRequest` bounds entry count and encoded bytes; raw requests
   exceeding the link packet capacity are refused before sending. Larger request
   Resources remain unsupported. `StaticNode` still refuses dynamic submissions.
   This prerequisite is published as Retinue `2563202`; Knot and Turnstone now
   consume that map API. Six map tests, seven endpoint tests and three link tests pass,
   and the API builds with the alloc-only feature set.
   Retinue owns the interoperable request value; Mere owns field
   state and submission actions; apps own destination context and activation.
   `nematic::micron::forms` now supplies ephemeral text, masked, checkbox and
   radio state, named/wildcard selectors and fixed variables. Its prepared maps
   match the three independently captured stock-client maps. Source changes,
   ambiguous groups, unsupported table/partial controls and configured bounds
   are refused; debug output redacts values. This API performs no IO and leaves
   the ordinary renderer inert. Apps must additionally bind state to the page's
   identity and address, resolve targets and invalidate stale requests. The
   independent follow-up capture confirms that unchecked checkbox/radio groups
   are omitted while empty text remains present. The focused Micron suite has
   28 tests, including seven form tests; this is an
   automated preparation receipt, not a headed widget or submit receipt.
   Shared contracts are published at Mere `91c6238d`. Knot `fae329c` adds an
   editable preview-side form with reviewed, explicit remote sending; Turnstone
   `d710af9` adds **Fill Micron form** in the command palette. Both retain
   ephemeral field state, require native destination authority, enforce bounded
   request time/size, cancel local work and suppress stale replies. A reply is
   transient output, not an implicit navigation or authored-source replacement.
   Knot passes 16 desktop and 16 site tests; Turnstone passes six focused form
   tests and two existing smolweb input tests. Actual loopback tests exercise
   the typed request and a 4096-byte Resource reply. They exposed abrupt client
   shutdown dropping the queued proof; both clients now await Retinue's bounded
   graceful shutdown before releasing the interface. Turnstone also tests its
   response cap against that Resource. These bounds remain after Resource
   reassembly. Static publication still does not execute dynamic handlers;
   inline form widgets and partial refresh remain outside that automated
   receipt. The app plans hold the exact clean-Cargo commands.
   The done-condition — a stock client reaching our controlled handler and our
   client reaching a stock node with independently matched values, followed by
   headed edit/submit receipts in Knot and Turnstone — was met on 2026-09-13.
   Rendering, focus and preview must not submit; cancellation or a newer
   navigation must prevent stale results replacing a page. Bound field count,
   submitted bytes and request time explicitly. The existing page-byte cap is
   applied after Resource reassembly; it is not a form-size or early allocation
   limit.
   The headed acceptance is recorded at `C:\t\micron-headed-20260913`, whose
   `RECEIPT.md` indexes the captures, logs, scenarios and scripts. It used two
   independent instruments: a controlled public-RNS `nomadnetwork/node` request
   handler logging every received typed map with wall-clock timestamps
   (`handler/requests.jsonl`), and a real stock `nomadnet` 1.4.2 daemon under WSL
   whose stock executable page records the `field_*` environment it receives
   (`stock-node/submissions.jsonl`). Stock NomadNet stayed black-box. Both apps
   were built from `C:/t` with the isolated Cargo home, Rust 1.97.1 and absolute
   manifests: Knot desktop `fae329c` with `--locked --offline`, Turnstone
   `d710af9` with `--offline`. In both apps opening a page, opening the form and
   editing fields sent nothing beyond the ordinary page fetch; one explicit Send
   produced exactly one independently observed map with the edited values
   (`field_hd_text="edited café 雪"`, `field_hd_empty="filled"`,
   `field_hd_mask="secret"`, `field_hd_checks="red,blue"`,
   `field_hd_radio="blue"`), and the reply was visible in the app. Both apps also
   completed a send to the stock daemon, whose page script recorded the same map
   shape — Turnstone with edited values, Knot with defaults; the node passes the
   `str → str` map verbatim as `field_*` environment plus `link_id` and does no
   splitting or typing. Cancellation, navigating away, reload and timeout each
   produced one request and no retry, showed an explicit unknown-remote-outcome
   message, and suppressed the handler's late reply. Turnstone's
   `TURNSTONE_NOMADNET_TIMEOUT_SECS` and `TURNSTONE_NOMADNET_MAX_PAGE_BYTES`
   bounds were exercised directly; Knot's equivalents are compile-time defaults
   (30 s, 4 MiB) with no environment override. Knot additionally refused a
   malformed local target before sending and offered a stale-form discard after
   the source changed under a prepared review.
   As of 2026-09-16 that artifact tree is gone: `C:\t\micron-headed-20260913` was
   deleted on or before that date along with the rest of the `C:/t` Micron
   scratch family, so the paths above are dead references, kept as the record of
   where the work was done rather than as somewhere to look. Nothing committed
   was lost and the acceptance stands on the tests and fixtures named in this
   lane, which still run and pass. The shared isolated Cargo home went with it,
   so any plan command naming
   `CARGO_HOME=C:/t/smolweb-next-20260913/cargo-home` — here and in several
   sibling plans — needs that home refetched before it will run. Receipt
   artifacts now live under `Code/testing/<repo>/`, not `C:/t`.
   Still open after this acceptance: inline form widgets, partial refresh,
   outgoing request Resources, multi-segment responses, authentication and
   dynamic page hosting in Djinn. Follow-up from the receipt: Knot compares its
   4 MiB response cap only after unpacking, so a 5 MiB reply fails as
   `Remote Micron handler returned an invalid response` — Retinue's
   single-segment `MAX_SEGMENT_SIZE` (1,048,575 bytes) is hit first and the cap
   is unreachable, naming the wrong cause. Turnstone caps received bytes before
   decoding and refuses correctly for both 64 KiB and 5 MiB replies, so Knot
   should move its check ahead of unpacking; multi-segment responses remain a
   separate transport gate. Two further Knot cosmetics from the same run: the
   preview pane loses its scroll offset during submission redraws, making the
   cancel control hard to hit on long pages, and the previous reply text survives
   closing and reopening a form (values themselves reset).
4. **Refresh, media and directives.** Qualify partial replacement, refresh timing,
   image targets and page/cache directives separately. Reuse the host's existing
   subresource and request lifecycle with configurable cadence, fanout, byte and
   time bounds; parsing and saving
   source remain inert. Done when stock behavior is documented and each enabled
   feature has bounded fetching, cancellation, stale-completion, offline/failure
   and both-consumer receipts. An unsupported feature keeps its source and a
   diagnostic. Full conformance requires this matrix, not just successful parsing.

The evidence lane can proceed alongside reading fidelity. Forms do not gate
ordinary static serving. Djinn's persistent serving contract lives in
[resident services §13](../../mere_docs/implementation_strategy/2026-08-22_djinn_family_resident_services_plan.md#13-ordinary-site-serving-and-governed-replication-2026-09-11-scope).
Tabard's reader-theme work can consume shared styling roles without extending
Gemtext or turning author styling into a cross-client requirement. The Go client's
incompressible multipart failure against both servers remains a separate
transport compatibility investigation, with the current Python receipt retained.
The Rust peer candidate [`nomadnet-rs`](https://github.com/TeskesLab/nomadnet-rs)
0.3.1 remains experimental after the 2026-09-13 qualification. Its package is
MIT licensed; its RNS dependencies use the custom Reticulum License and were
tested as black boxes. Native Windows compilation failed. On Linux, Retinue
retrieved a small page exactly from both the stock server and raw PageCache API,
but multipart responses timed out. The browser API also failed its own server
self-control, so forward failures cannot qualify or disqualify Retinue.
See the [Retinue receipt](../../../../retinue/design_docs/2026-09-13_nomadnet_go_resource_compression_receipt.md)
for pinned inputs and the eleven-case matrix. It is not a runtime dependency.
The Go client likewise remains a test peer, not a shipped-stack requirement.

Follow-up diagnostics confirmed mismatched browser request IDs and an undecoded
MessagePack response wrapper. An isolated single-request remap plus decoding
retrieved small and both 128 KiB pages from unchanged Retinue exactly on WSL and
native Fedora. This qualifies that experimental direction only, not the stock
browser or a production-safe correlation fix. In reverse, the Rust server emitted
neither response nor resource advertisement for large requests before timeout.
The linked Retinue receipt records the traces and remaining server-side question.

The subsequent external-peer comparison narrows that question: upstream passing
tests use rns-net 0.7.0 and an explicit Resource-response API absent from 0.5.6.
The newer explicit server initially passed all three page fixtures with the
diagnostic Rust client but timed out with Retinue. The activation comparison
then confirmed that float32 RTT encoding left this peer inactive. Retinue now
emits float64 RTT encoding; all three page fixtures transfer exactly from both
the newer explicit Resource server and stock Python RNS 1.5.3 on native Fedora.
The old server API and stock browser defects remain separate. This is a Retinue
source fix. The 2026-09-13 adoption pins Mere's optional transport and Turnstone's
page client to Retinue `85e716c7f06dac0a5253effe5ef06d05b266f02f`. Knot's
`knot-site` adopts the same revision in `4d880910fd8b6ffa6e4227b65ea65278f3e8011b`,
which Djinn now selects. Knot's locked NomadNet saved-snapshot lifecycle test
passed, including small/128 KiB responses, replacement, and interface shutdown.
Locked dependency-tree checks show exactly that Retinue revision on both the
Turnstone page-client path and the Djinn → knot-site serving path. These checks
run from `C:\t` with an isolated Cargo home, outside local development overrides.
The linked Retinue receipt also preserves three unsent upstream issue drafts and
the separate Resource-handler API note.
Turnstone adoption `a596f18` passed five NomadNet and two smolweb routing tests
with `--locked --offline`. Logs and resolved lockfile snapshots are retained in
`C:\t\smolweb-next-20260913` (`turnstone-rtt-locked.log`,
`turnstone-rtt-smolweb.log`, `knot-rtt-locked.log`, and the `*-rtt-tree.log` files).
Djinn's library suite passed 74 tests with `--locked --offline` and default features disabled; this covers
the resident serving integration, not a new headed or external-client receipt.
Mere's optional backend also passed `cargo check --locked --offline -p
mere-transport --features reticulum`. The corresponding logs are
`djinn-rtt-locked.log` and `transport-rtt-check.log` in the same receipt directory.
That receipt directory is also no longer present: it was deleted on or before
2026-09-16 with the rest of the `C:/t` Micron scratch family, taking the logs,
the lockfile snapshots and the isolated Cargo home named above with it. The
adoption itself is unaffected — the tests and tree checks those logs recorded
still run — but the path is now a record of where the checks ran, not a place to
read them.

### Semantic collapses (parse layer — fix by enriching the AST)

| Protocol | Spec distinction | Where it is lost |
| --- | --- | --- |
| RSS/Atom | `<summary>` (abstract) vs `<content>` (full body) | merged into one `summary` ([feed.rs:212](../../../crates/system/errand/src/parse/feed.rs)); the article body is dropped |
| Atom | `published` vs `updated`; RSS `pubDate` | merged into one `date` ([feed.rs:207](../../../crates/system/errand/src/parse/feed.rs)) |
| RSS/Atom | `<enclosure>` (podcast audio/media) | no field on `FeedEntry`; podcasts lose their payload |
| RSS/Atom | `<guid>` / `<id>` (stable entry identity) | dropped; read-state and dedup fall back to link+title |
| Atom | multiple `<link rel="alternate\|self\|enclosure">` | first-wins, `rel` ignored ([feed.rs:174](../../../crates/system/errand/src/parse/feed.rs)) |
| Gopher | item-type family (RFC 1436 + gopher+) | `g`/`I`→Image, `9`→Binary, but `4`/`5`/`6`/`d`/`;`→Other; `8` (telnet)→Other while `T` (tn3270)→Telnet, an inversion ([gopher.rs:112-119](../../../crates/system/errand/src/parse/gopher.rs)). The fetch/handling hint the type char exists to carry is flattened |
| Gopher | type `7` = search server (append a query with a TAB) | rendered as a plain link; the query-input step is gone |
| Spartan | `=:` prompt-upload line (its defining feature) | becomes body text; `GemLine` has no prompt variant |
| **all** | **trust posture** (gemini TOFU / spartan-unauthenticated / misfin-signed) | carried by neither the ASTs nor the native views |

### Presentation collapses (box-substrate artifacts — the regime-B candidates)

- **Gemtext text runs** join consecutive lines with a space, so hard line breaks
  vanish (poems, addresses, deliberate non-`pre` layout). "Paragraph" is not a
  gemtext concept; each line is discrete.
- **Gopher menus** render as proportional `p.gopher-itemline` rows, so the
  fixed-width column alignment gopher clients traditionally give (and that the
  ASCII-art info `pre` blocks assume) is broken.

### The trust gap (architectural)

The parse ASTs carry no trust state (verified: nothing in `errand/src/parse/`). The
native lane goes errand-parse → `mere-document-lanes::SmolwebDocument`, **bypassing `Block`** and its
`DocumentTrustState`. So focused viewing via the genet lane surfaces no security
posture: a spartan page (unauthenticated by design), a gemini page (TOFU), and a
misfin message (signed sender) render with the same neutral chrome. The transport
already knows the outcome (Phase A installs `InMemoryTofu`; a pin mismatch fails the
load) and then discards it.

**Correction (2026-07-01 review): the card lane's trust is structurally present but
empty in practice.** Every nematic smolweb engine emits `trust:
DocumentTrustState::Unknown` today (verified in `gemtext.rs` / `gopher.rs` / `nex.rs`
/ `feed.rs`), so the `Block` lane *models* the posture without ever *populating* it.
WS2 therefore has to produce trust for **both** lanes from the one transport source,
not merely carry it into the native one; the done-condition ("same posture on card
and focused tile") already implies this, but the audit contrast above overstates the
card lane's current state.

---

## 2. The regime spectrum (design frame)

Three ways to get a format onto the screen. The plan keeps A as the default and
escalates one format at a time.

| | (A) element tree + CSS *(default)* | (B) own layout, shared shaper | (C) raw paint |
| --- | --- | --- | --- |
| Format idiom lives in | mapping + stylesheet | a bespoke line/layout tree | a bespoke layout + paint fn |
| Shaping | shared (genet) | shared (parley direct) | shared shaper |
| Line-break + stack | shared | you own it | you own it |
| Selection / find / a11y / zoom | free | re-earned per format | re-earned per format |
| Cross-format identity of a paragraph | guaranteed identical | drifts | drifts |
| Code per format | ~50 lines + CSS | ~300-800 (a mini typesetter) | ~800-2000 |

The rule that falls out: **share synonymous constructs through A; reach for B only
when the format's line model is genuinely not box-flow** (a fixed-width grid, hard
columns, terminal alignment). Lagrange is a real (B); the shipped native lane took
its philosophy and implemented it as (A) for the leverage.

---

## 3. Workstream 1 — enrich the parse ASTs (errand)

Recover the parse-layer losses. Each AST change pairs with the nematic lowering that
consumes it on the capture/`Block` side, so the two lanes stay in step.

**Feed** ([errand/src/parse/feed.rs](../../../crates/system/errand/src/parse/feed.rs)) —
illustrative signatures:

```rust
// illustrative, not compile-ready
pub struct FeedEntry {
    pub title: Option<String>,
    pub id: Option<String>,            // <guid> / atom:id — stable identity
    pub link: Option<String>,          // rel="alternate" (the article)
    pub enclosure: Option<Enclosure>,  // <enclosure> / rel="enclosure" (media)
    pub published: Option<String>,     // first authored
    pub updated: Option<String>,       // last changed
    pub summary: Option<String>,       // <summary> / RSS <description>
    pub content: Option<String>,       // <content> / <content:encoded> (full body)
    pub author: Option<String>,
    pub categories: Vec<String>,
}
pub struct Enclosure { pub url: String, pub mime: Option<String>, pub length: Option<u64> }
```

Channel level gains `ttl: Option<u32>` (the poll-interval hint the real Subscribe
feature needs) and `image: Option<String>`. `feed_view` keeps showing summary+date;
the new fields feed the article reader, the podcast affordance, and read-state.

**Two flags on this workstream (2026-07-01 review):**

- **Lockstep + publish timing.** These are public-struct field changes on errand,
  breaking nematic's lowerings and `feed_view` simultaneously: one coordinated
  cross-repo pass (errand → genet → mere) per the established push choreography.
  And errand's manifest is publish-shaped (crates.io metadata, keywords, readme), so
  the field set should settle through WS1 *before* any crates.io publish; churning
  public struct fields post-publish is a semver treadmill.
- **`content` is an HTML fragment — the article reader needs a lane decision.** Feed
  bodies (`<content>`/`<content:encoded>`) are HTML. Rendering them inside
  `feed_view` would pull HTML rendering into the smolweb document lane, against the two-family
  split; the alternative is a cross-lane handoff (the entry card opens the article
  via the HTML/document lane, with `content` as the offline body). Decide before the
  article reader is built; the field itself is lane-neutral and can land first.

**Gopher** ([errand/src/parse/gopher.rs](../../../crates/system/errand/src/parse/gopher.rs)):
add `raw_type: char` to `GopherItem` so the exact item type always survives even when
`kind` is coarse; fix the `8`/`T` inversion; add `Cso` (type 2, interactive query);
keep the coarse `kind` for the marker but let handling read `raw_type`. Type-7 search
stays `Search` here; the input affordance is Workstream 3.

**Gemtext / spartan**
([errand/src/parse/gemtext.rs](../../../crates/system/errand/src/parse/gemtext.rs)): add
`GemLine::Prompt { url, label }` and classify `=:` (benign for pure gemtext, which
never carries it). Spartan then renders it as an upload affordance instead of body
text.

**Cross-repo note:** struct-field additions are visible to mere/genet through the
gitignored `.cargo/config.toml` path override at build time, so the local edit loop
works. A clean or CI build needs the errand push (unlike the *feature*-resolution
wall the native plan hit, plain field additions do not need a feature gate).

---

## 4. Workstream 2 — trust in the native lane

Trust originates at the **transport** (errand already knows it) and should be
surfaced as **tile chrome**, not document-body content, exactly like a browser's
address-bar posture. Both lanes converge on one trust type so a capsule looks the
same whether shown as a card or a focused tile.

- **Produce at transport.** errand returns a trust descriptor beside the bytes. Reuse
  the `DocumentTrustState` shape (Trusted / Tofu / Insecure / Broken / Unknown) so the
  two lanes share one vocabulary; misfin's signed-sender identity is an extra field,
  not a new ladder. Mapping: gemini matched-pin → Trusted or Tofu, mismatch → Broken;
  gopher / finger / nex / spartan → Insecure (unauthenticated by design); misfin →
  Trusted + `signer` when the sender identity verifies.

  > **Correction, 2026-08-04: that mapping is keyed on the wrong thing.** It reads
  > posture off the *scheme*, and posture is a property of the **carrier**. Once a
  > protocol can run over more than one carrier (see
  > carrier independence (`smolweb/design_docs/research/2026-08-04_protocol_carrier_independence.md`)),
  > the table above ships a falsehood in both directions: gopher over a Reticulum
  > link is **not** Insecure, because the link is encrypted and the peer is proven
  > by its destination key; and gemini over that same link is **not** Tofu, because
  > there is no certificate and no pin to have a state about. The descriptor must be
  > produced by the transport that actually carried the bytes, with the protocol
  > contributing only what it adds on top (misfin's signed sender, gemini's client
  > certificate). The mapping above stays correct for the TCP/TLS carrier, which is
  > the only one wired today; it must not be read as a scheme lookup.
- **Carry through the view.** `SmolwebDocument` gains `trust: DocumentTrustState`
  (+ optional `signer`). The native view body does not change; the host reads the
  field.
- **Surface in the host.** The meerkat smolweb lane (`ensure_smolweb` in
  `content/handlers.rs`) captures the fetch trust and exposes it so the tile chrome
  shows the posture. This is the host-integration touchpoint; see
  smolweb host integration plan (`mere/design_docs/mere_docs/implementation_strategy/2026-06-28_smolweb_host_integration_plan.md`).
- **Home for the shared type.** Decide with Mark whether `DocumentTrustState` moves to
  a small shared crate both errand and inker depend on, or errand defines its own and
  the host maps between them at the lane boundary. Default: errand defines a minimal
  `TransportTrust`, the host maps it to `DocumentTrustState`, avoiding a new shared
  crate until a second consumer wants one.
- **Precondition to verify (2026-07-01 review): the host's TOFU store.** errand
  defaults to `PermissiveTofu` (accept-any) unless a store is installed; pelt's
  fetcher installs an `InMemoryTofu`, but whether **meerkat's** fetch path does has
  not been checked. If it does not, gemini in Mere is silently accept-any today and
  "a gemini tile reads its TOFU pin state" has nothing to read — installing (and
  eventually persisting) the host trust store is part of this workstream, not an
  assumption. The card lane needs the same feed: nematic engines emit
  `DocumentTrustState::Unknown` unconditionally (see the §1 correction), so WS2's
  transport descriptor must reach the `EngineInput`/lowering side too.
  *Re-checked 2026-09-02, with the knot evaluation/export plan's open question
  rehomed here:* meerkat is gone (deleted 2026-07-18), so the hosts to check
  are Turnstone and mere. `genet-documents` installs an `InMemoryTofu`; mere's
  `fetch` exposes `install_smolweb_tofu` for a host-owned store; **no durable
  `TofuStore` exists anywhere in the workspace** (smolweb ships only the
  in-memory and permissive ones). The location question the knot plan left
  open — a file beside the profile, or eidetic engrams — is therefore still
  open and belongs to this workstream: start file-backed, migrate when
  persona/keys land fully.

---

## 5. Workstream 3 — bespoke where boxes fail (gopher first)

Only the presentation collapses, and only where the line model is genuinely not
box-shaped.

- **Gopher monospace grid.** Start **B-lite**: render the menu in a monospace grid
  (a `pre`-context or a CSS grid with a monospace font) so type marker, display, and
  the info ASCII-art columns align, while links stay focusable. This recovers the
  alignment without a bespoke typesetter and is unit-testable against the element
  tree. Half the ground is already held: `gopher_view` folds info runs into one
  monospace `pre.gopher-info` today; B-lite extends that treatment to the item rows
  rather than starting over. Escalate to **B-full** (a bespoke fixed-width line layout emitting to the
  paint list) only if terminal-precise columns demand it. Gopher is the one format
  that plausibly earns B-full.
- **Type-7 search affordance.** Render a `Search` item as an inline query input that
  appends the entered text (TAB-joined) before navigating, instead of a bare link.
- **Spartan `=:` prompt.** Consume Workstream 1's `GemLine::Prompt` and render it as
  an input-link (a labelled field that uploads on submit), the affordance the native
  plan named but never modelled.
- **Gemtext hard breaks.** Offer preservation of hard line breaks in text runs as a
  setting (reflow vs preserve), per the configurability-over-defaults principle,
  rather than always joining with a space. Small view tweak, not full regime B.

---

## 6. Sequencing and done-conditions

Targets, not dates.

- **WS1 (AST enrichment) is foundational.** It carries the trust field's neighbours
  and the gopher `raw_type` WS3 reads. Done when: feed round-trips
  published/updated + summary/content + enclosure + id; gopher preserves `raw_type`
  and the `8`/`T` fix; spartan `=:` parses to `Prompt`; the nematic lowerings and
  their tests are green; errand pushed.
- **WS2 (trust) follows WS1.** Done when: a spartan tile reads Insecure, a gemini tile
  reads its TOFU pin state, a misfin tile reads its signer, and the same posture shows
  identically on the card and the focused tile.
- **WS3 (regime B) is independent, gopher-scoped.** Done when: a gopher menu's columns
  align, a type-7 item takes a query, a spartan `=:` uploads, and gemtext hard-break
  preservation is a setting. B-full is entered only if B-lite alignment proves
  insufficient, and that decision is logged here.

---

## Findings

- **The lossy layer is the parse ASTs, not the renderer** (verified against
  `errand/src/parse/*`). This inverts the original "are we foisting HTML" worry: the
  risk is protocol semantics normalized away at parse time, not HTML semantics
  imposed at paint time. The fix is richer ASTs, not a different render regime.
- **The native lane carries no trust** (nothing trust-shaped in `errand/src/parse/`;
  `SmolwebDocument` emits no posture). The `Block` lane has `DocumentTrustState`; the
  genet lane, which the host uses for focused tiles, drops it.
- **Gopher is the sole clear regime-B candidate.** Gemtext, feed, nex, finger,
  spartan, guppy, scroll, misfin are all box-flow-shaped; gopher's fixed-width typed
  column is the one line model the box substrate visibly distorts.

## Progress

- **2026-07-01**: Plan created from the fidelity audit with Mark (the DocumentBlock →
  Block terminology sweep opened into a substrate/spec-faithfulness review). Collapse
  inventory verified against `errand/src/parse/*`, the then-current `smolweb-views/src/lib.rs`, and the
  paint-list API. Three workstreams scoped; A-default / B-for-non-box regime rule set.
- **2026-09-13**: headed Micron form acceptance taken for both consumers against a
  controlled public-RNS request handler and a real stock `nomadnet` 1.4.2 daemon
  under WSL, artifacts at `C:\t\micron-headed-20260913`. Knot desktop `fae329c`
  and Turnstone `d710af9`; edit/send, cancellation, navigation, reload, timeout
  and size-cap outcomes independently observed. Lane 3's done-condition is met.
  Inline widgets, partial refresh, request Resources and multi-segment responses
  stay open, and Knot's response cap is compared after unpacking rather than
  before it.
- **2026-09-16**: the `C:/t` Micron scratch family was deleted on or before this
  date — `micron-headed-20260913`, `smolweb-next-20260913`,
  `micron-reference-20260912`, `micron-forms-20260913`,
  `nomadnet-rust-interop-20260913` and both RNS virtualenvs, gone from inside WSL
  too and not in the Recycle Bin. It was not a blanket wipe of `C:/t` — files
  from 2026-09-11 survive and other sessions were writing there on 2026-09-15 —
  and the cause is unknown. Nothing committed was lost and every test the
  2026-09-13 receipts cite still runs and passes, so what went is the artifact
  trail, not the result. Mark ruled the same day that the docs are corrected
  rather than the exercise re-run, and that receipt artifacts now live under
  `Code/testing/<repo>/`.

## Cross-references

- [native smolweb rendering plan](2026-06-27_native_smolweb_rendering_plan.md) — the
  shipped transport → parse → native render this extends; the two-family model and the
  crate/dependency direction it defines.
- smolweb host integration plan (`mere/design_docs/mere_docs/implementation_strategy/2026-06-28_smolweb_host_integration_plan.md`)
  — the meerkat genet lane; Workstream 2's trust surfacing lands against its P3/P4.
- TERMINOLOGY.md (`mere/design_docs/TERMINOLOGY.md`) — the trust ladder and the
  protocol-faithfulness rule this plan operationalizes.
- errand (sibling repo `mark-ik/errand`) — owns the parse ASTs Workstream 1 enriches
  and the transport Workstream 2 reads trust from.
