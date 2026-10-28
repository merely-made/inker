# Smolweb fidelity: trust, Micron conformance, Turnstone stages and fetch actor adoption

**Status:** 2026-10-28: cut from `2026-07-01_smolweb_fidelity_plan.md`, archived in `archive_docs/2026-10-28/`. Built: the one render path (WS4, R0 to R6, done 2026-10-06, which superseded WS3's bespoke rendering); WS1's feed fields, guid and enclosures, the Spartan prompt, gopher's raw item type, the `8`/`T` fix and CSO; Turnstone's S4; Micron's reading slice with both consumer integrations; Micron lane 3 (forms, headed acceptance 2026-09-13); and the fetch actor's admission and joined shutdown, qualified in Mere (2026-10-27). Open: WS2 trust (not started), the WS1 residue, Micron lanes 1 and 4, Turnstone S1 to S3, and Turnstone's adoption of the fetch actor.

**For:** Getting every smolweb protocol (gemini, gopher, Spartan, nex, finger, feeds, Misfin, Micron) onto the one `EngineDocument` render path with its protocol semantics intact and its trust posture visible. It was opened by Mark's three principles of 2026-07-01, by rulings S66 and S70 to S77 of the stack seams plan (one render path), and by Turnstone's U3, which gave this plan the Mere halves of its stages S1 to S4 (Mark chose "Mere's smolweb fidelity lane").
Not here:
- Micron lane 2, document navigation (anchors and collapsible sections), runs in `2026-10-28_micron_navigation_plan.md`, which holds A1's Turnstone half and R1's headed receipts.
- The Mere half of Turnstone's S8, `accessibility_projection` for smolweb and Micron sessions: Turnstone's unusual-protocols lane (its U16).
- Djinn's persistent serving of Micron pages and dynamic page hosting: `2026-10-28_site_publication_plan.md` (resident services section 13).
- Tabard's reader theme consuming shared styling roles: `2026-10-28_appearance_plan.md`.
- Routed client early refusal and the explicit standalone-client queue: `smolweb/design_docs/implementation_strategy/2026-10-22_client_limits_plan.md`, in the smolweb repository. Retinue's Resource bounds stay separate.
- knot-editor's desktop preview wildcard arm and `..` in its `FeedEntry` pattern at its next repin: the knot-editor lane (S77).

Receipts from this plan's runs go to `archive_docs/<date>/`, never into this
plan (DOC_POLICY §8). Evidence that matters is a test in the repository or a
line in Findings.

## Constraints

From the smolweb fidelity plan (`archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md`):
- The three principles (Mark, 2026-07-01): share synonymous parts through the box substrate; go bespoke only where a format's line model is not box-shaped; enrich the parse ASTs and manage trust in the native lane (:127-138).
- The `EngineDocument` lane is the single smolweb render path. Every host draws through document-canvas, and no per-format views return (S66, S70 to S77) (:803-808).
- Trust originates at the transport and is keyed by carrier, not scheme: gopher over a Reticulum link is not Insecure and gemini over it is not Tofu. It surfaces as tile chrome, not document body, and both lanes converge on one trust type (:660-686).
- Spec-accurate grammars follow the wire crates into the smolweb workspace; WS2 stays on the implementation side in Mere (:91-95).
- Stop rules: publishing gopher-protocol or any crate waits for Mark's sign-off; a change needed in knot-editor, Turnstone or genet stops and comes to Mark (:886-893).
- Micron clean-room: public prose and black-box captures only, never NomadNet implementation source. A construct stays raw source or an explicit unsupported format until a fixture and an independent capture establish it; bracket links, cache directives, terminal styles and form syntax are not inferred from examples (:146-157).
- Source bytes stay authoritative. An unsupported feature keeps its source and a diagnostic, and parsing and saving source stay inert (:239, 407-411).
- Forms: rendering, focus and preview never submit; cancellation or a newer navigation stops a stale reply replacing a page; field count, submitted bytes and request time are bounded (:340-344).
- `nomadnet-rs` is experimental and not a runtime dependency; the Go client is a test peer, not a shipped-stack requirement (:427-436).
- Fetch actor: errand keeps wire and native deadlines, Mere's per-operation byte, time and Submit policy is unchanged, the original `spawn_fetcher*` signatures and exhaustive command and update enums are preserved, dropping a handle stays nonblocking, and a started write without a receipt reports unknown delivery rather than implying a safe retry (:21-40, 78-80).

## Phases

### T1. Trust in the native lane (WS2), with Turnstone S3
Errand returns a trust descriptor beside the bytes, produced by the transport that carried them and keyed on the carrier. `EngineDocument.trust` is then populated for both lanes (every Nematic engine sets `Unknown` and document-lanes never sets it, so the descriptor must also reach the lowering side the card lane uses), and host chrome shows it. Misfin's signer is an extra field, not a new ladder. The TCP/TLS mapping (matched pin to Trusted or Tofu, mismatch to Broken, gopher, finger, nex and Spartan to Insecure, Misfin to Trusted with a signer) has no entry yet for Reticulum, where NomadNet and Micron pages already travel. A durable TOFU store is part of the work: Mere installs only an in-memory store and `fetch::install_smolweb_tofu` accepts a host-owned one. Turnstone's S3 is this phase's forcing consumer: per-carrier trust fills `EngineDocument.trust` (TLS TOFU state, unauthenticated TCP, a proven Reticulum destination).
- Done when: a Spartan tile reads Insecure, a gemini tile reads its TOFU pin state, a Misfin tile reads its signer, and the same posture shows identically on the card and the focused tile.
- Needs: open questions 1 to 3; Turnstone's S3 opens with failing tests.
- Rulings: S66; the 2026-08-04 correction (carrier, not scheme).
- From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:660-751, 795-797`; `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:108-125, 518-546, 687-714`

### T2. Content type before scheme (Turnstone S1)
document-lanes chooses its lowering by content type before scheme for `text/plain` and `text/markdown`, and an XML body that sniffs as RSS or Atom reaches the feed engine. Turnstone's accessibility-first ruling (its U2) puts S1 and S2 first.
- Done when: the source gives none beyond Turnstone's rule that each stage opens with failing tests.
- Needs: Turnstone's S1 tests.
- Rulings: -
- From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:108-122`

### T3. Find, zoom and selection in smolweb sessions (Turnstone S2)
Smolweb sessions gain find, per-node page zoom, and text selection with copy.
- Done when: the source gives none beyond Turnstone's rule that each stage opens with failing tests.
- Needs: Turnstone's S2 tests; T2.
- Rulings: -
- From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:108-122`

### F1. Fetch actor adoption in Turnstone
The bounded companion (`FetchActor`: active and pending counts and an independent queue-wait deadline, defaults 16 active, 64 pending and 30 seconds) is qualified in Mere. Turnstone's shell embeds `ActorHandle<FetchCommand>` and matches commands and updates exhaustively, and its existing ingress is not bounded; strict pre-mailbox admission needs the companion. Output-channel backpressure, aggregate retained-body memory and HTTP connection-pool ownership are not covered by the operation-count limits and stay separate. The clean worktree `Code/worktrees/mere-fetch-actor` is removed after verified publication and integration.
- Done when: Turnstone adopts the bounded handle and passes its own native and headed qualification; Mere's side was held to saturation and refusal before contact, finite queue waiting, exact-request cancellation and supersession, unrelated requests continuing, one terminal result, permit release after task destruction, and joined cleanup for page, binary, favicon and explicit Submit.
- Needs: Turnstone's lane.
- Rulings: -
- From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:19-89`

### W1. WS1 residue
The feed channel `ttl` (the poll-interval hint) and `image` fields; Atom links with no `rel` or `alternate` still resolve first-wins; Atom `type="xhtml"` content is not captured; enclosures are carried but not drawn; nex still lowers the old way, so the `FIXED_WIDTH_MENU_TYPES` typeface rule stays; uxtree projects no paragraph-level Submit; and gemini-protocol's `GemLine` has no prompt variant (the Spartan prompt landed as a Spartan-grammar variant instead; open question 4).
- Done when: WS1's condition holds for the feed, gopher and Spartan parts already built; the items above are what the source leaves.
- Needs: any gemini-protocol or gopher-protocol change goes through a crate release and a Mere repin, on Mark's sign-off.
- Rulings: -
- From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:10-14, 494-507, 596-598, 643-646, 963-968`

### M1. Micron lane 1: reading fidelity qualification
The reading slice (colour, underline, alignment and section indentation in Inker's typed `Presented` wrappers) is built in both consumers; what is open is qualification. Nematic owns source interpretation, Inker and document-canvas own reusable presentation, and document-lanes own the retained viewport. Also open: syntax compatibility for malformed input, escape spelling, section exit and version differences, each needing stock captures; an arbitrary file acquires no native destination and bare or relative native targets are unqualified; a persisted reader-appearance setting (`SourcePresentation::Reader`) in Knot and Turnstone is not qualified; and a runtime UI appearance receipt and full interactive conformance are separate gates.
- Done when: the original combined-style and nested-section fixtures have independent stock-client comparisons and headed receipts in both apps, including keyboard focus, selection, contrast overrides and narrow widths, and horizontal scrolling exposes the full wide-table packet and preserves clipping and link hit coordinates after scrolling and resizing.
- Needs: -
- Rulings: -
- From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:271-282`; `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:186-188, 206-210, 217, 236-239`

### M2. Micron lane 4: refresh, media and directives
Partial replacement, refresh timing, image targets and page or cache directives, qualified separately and reusing the host's subresource and request lifecycle with configurable cadence, fan-out, byte and time bounds. Lane 3's acceptance left these open beside them: inline form widgets, partial refresh, outgoing request Resources (larger than packet-sized maps are unsupported, and `StaticNode` refuses dynamic submissions), multi-segment responses and authentication.
- Done when: stock behaviour is documented and each enabled feature has bounded fetching, cancellation, stale-completion, offline and failure, and both-consumer receipts. Full conformance requires this matrix, not successful parsing.
- Needs: Retinue's request and response transport limits for the Resource and multi-segment items.
- Rulings: -
- From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:404-411`; `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:303-305, 334-336, 385-387`

### H1. Mere host integration and interactive eyeballing
The archived native smolweb rendering plan's last tail: Mere's own host integration of the smolweb lane and interactive eyeballing of it, received here under S66. The per-format views in `cambium::nematic` that the same tail told to retire went in R0.
- Done when: the source gives none.
- Needs: T1 to T3 for the host chrome the integration would show.
- Rulings: S61, S66.
- From: `archive_docs/2026-10-28/2026-07-03_archived_plan_tails_plan.md:1265-1269`; `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:97-106`

## Open questions

1. What is WS2's scope now that two lanes carry one document? Options: drop "`SmolwebDocument` gains `trust`" and restate WS2 as the transport descriptor, then a populated `EngineDocument.trust`, then chrome, with a separate native-view field for the Misfin signer. From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:699-703`
2. Where does the shared trust type live? Default: errand defines a minimal `TransportTrust` and the host maps it to `DocumentTrustState`, with no new shared crate until a second consumer wants one. From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:715-719`
3. What settles the TOFU store for Mere? Options: Turnstone's file-backed `GeminiTrustStore` (`gemini_trust.json`) settles the pattern and Mere copies it or shares it through `fetch`; the question stays open for Mere (a file beside the profile, or eidetic engrams); or Turnstone's store is promoted into `fetch` as the stack's shared durable store. From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:729-751`
4. Does the Spartan prompt's landed shape (`SpartanLine::Prompt`, lowered with `push_submit`) meet WS1? Options: accept it as a deviation and mark the Spartan condition met, or reopen it, for example if the prompt is wanted in gemini-protocol. From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:643-646`
5. Wide-table navigation is contradictory: the Smolweb session scrolls vertically only and horizontal navigation of unusually wide tables is open, yet document-lanes is also said to retain horizontal viewport movement. Which holds, and does lane 1 close it? From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:206-210, 235-236`
6. Where does the Micron conformance programme live? The S14 pass offered extracting lanes 1, 3 and 4 to a dedicated plan, keeping them here, or extracting only the open lanes 1 and 4. This successor holds the open lanes 1 and 4, with lane 3 closed history in the archive; whether they move again is unanswered. From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:413-418`
7. Where do the peer-compatibility investigations live? The Go client's incompressible multipart failure, the Rust peer server's large-request question, and the older server API and stock browser defects remain separate; three unsent upstream issue drafts and a Resource-handler API note sit in the Retinue receipt. From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:425-426, 438-463`
8. smolweb's local main held the published gopher-protocol 0.2.0 commit unpushed as of 2026-10-06. Is it pushed? From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:981`

## Findings

- 2026-10-06: `SmolwebDocument` holds `document: Arc<EngineDocument>`, Nematic's lowering (`crates/system/document-lanes/src/smolweb.rs`), so both lanes carry `EngineDocument.trust`. Every Nematic engine sets `DocumentTrustState::Unknown` (`gemtext.rs`, `gopher.rs`, `feed.rs`, `micron.rs`, `nex.rs`, `spartan.rs`). From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:528-537`
- 2026-10-06: the in-memory TOFU install is in `crates/system/document-lanes/src/remote.rs`; Turnstone's file-backed store implements `fetch::SmolwebTofuStore` and is installed through `fetch::install_smolweb_tofu`. No durable store exists in the Mere workspace. From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:739-745`
- 2026-10-06: gopher rows take their marker from `raw_type`, so `g` and `I`, or `8` and `T`, stay distinct, and CSO lowers as `Other`; gopher-protocol 0.2.0 is published and errand is at 0.4.0. From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:972-981`
- 2026-10-26: Armillary already provides `ResidentTasks::close_and_join` and `ActorHandle::join`; the existing fetch code retained only page tasks and detached the other task classes, which is why a compatible companion was needed rather than a change to the public enums. From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:57-62`
- 2026-09-13: Retinue's `StringMapRequest` bounds entry count and encoded bytes, and raw requests over the link packet capacity are refused before sending. Stock captures and the activation receipt are checked in under `crates/nematic/nematic/tests/fixtures/micron/nomadnet-1.4.2/`. From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:176-182, 303-309`
- 2026-09-16: the `C:/t` Micron scratch artifacts of 2026-09-13 were deleted, so paths into them are dead references; the acceptance stands on the tests and fixtures. Receipt artifacts now live under `Code/testing/<repo>/`. From: `archive_docs/2026-10-28/2026-07-01_smolweb_fidelity_plan.md:375-384, 931-941`

## Progress

- 2026-10-28: cut from the smolweb fidelity plan; nothing new landed.
