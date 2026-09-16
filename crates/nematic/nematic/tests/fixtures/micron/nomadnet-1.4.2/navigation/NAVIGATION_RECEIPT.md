# Stock-client navigation receipt

Captured 2026-09-16 with stock NomadNet 1.4.2 serving and browsing the probe
pages in this directory on an isolated Reticulum TCP loopback pair. This is a
runtime/output capture, not a reading of NomadNet, RNS, or LXMF implementation
source. It answers phase C1 of the
[Micron navigation plan](../../../../../../../../design_docs/nematic_docs/implementation_strategy/2026-09-15_micron_navigation_plan.md).

## Runtime and isolation

- Rebuilt instrument: a WSL-native virtualenv at `/var/tmp/micron-c1-20260916/venv`
  with pinned `nomadnet==1.4.2`, `rns==1.5.3`, `lxmf==1.1.1` from PyPI.
- Runtime-reported version: `Nomad Network Client 1.4.2`.
- Package-metadata SHA-256 (`nomadnet-1.4.2.dist-info/METADATA`):
  `0916bffb68b508b13b92aec79f9660c35b74a9daa8c9e1e7c7b405ad35d584d2`. This
  **matches** the 2026-09-12 [reference capture](../REFERENCE.md), so the
  package is the same artifact. Installed metadata reports `rns 1.5.3` and
  `lxmf 1.1.1`.
- The `bin/nomadnet` launcher digest differs from the reference
  (`91f64c887c4e8bf591b5f9f18bb22a9cd4ab1f9f28bd71ec36dcd50800457257` here).
  The launcher is pip's generated entry-point stub and its first line names the
  virtualenv's own interpreter path, so its digest follows the install location
  rather than the package build. Wheel digests are in the
  [manifest](CAPTURE_MANIFEST.md) for future rebuilds.
- Node and client each had a fresh profile under `/var/tmp/micron-c1-20260916`,
  RNS transport disabled, instance sharing disabled, and one interface: the node
  a `TCPServerInterface` on `127.0.0.1:45450`, the client a `TCPClientInterface`
  to it. Every process ran with `HOME` pointed at `/var/tmp/micron-c1-20260916/home`.
  No public, RF, or third-party destination was used.
- The node destination `923706ddc70d389bd3719258c41f6592` was derived from the
  node's task-local identity with the public RNS `Destination` API and matched
  the daemon's own "ready for incoming connections" line.
- **Plain pages are servable on a WSL-native path.** The probe pages were
  installed as mode 644 files and the daemon logged `Serving page:` for each;
  no executable-page workaround was used.
- Client display settings changed from the generated profile: `glyphs = unicode`
  (the generated value was `nerdfont`). The browser pane ran fullscreen
  (`C-g`) in a 120x45 terminal, which shows 37 page rows.

## Instruments

- **Rendering and interaction:** the stock text client in a task-owned tmux
  session. Captures are `tmux capture-pane` screens, kept as ANSI (`.ansi`) and
  plain text (`.txt`). Input was keyboard through `tmux send-keys`, plus one
  terminal mouse click (an SGR mouse sequence) in probe 07c.
- **Transport:** the stock daemon's own console log at `loglevel = 7`, which
  writes `Handling request <id> for: <path>` with a wall-clock second for every
  request. No owned RNS request handler was needed. Each gesture was bracketed
  by recording the log's line count before it and printing every line after it.
- **Positive controls in the same run:** every first load of a page logged one
  request, `C-r` (Reload) logged one request, and the probe 09 same-node link
  logged one request.
- **Client cache:** reopening an already-visited page through the URL dialog,
  and Back/Forward, showed `Done (cached)` in the browser status row and logged
  no request. A zero-request result is therefore only evidence against transport,
  not evidence that no reload happened; the status row is recorded beside it.
- **Focus visibility:** link focus shows only as `Link to <target>` in the
  browser status row; the link text itself does not change. Focus on a
  collapsible heading changes no byte on screen.

## Observations

Captures are named `p<probe>-<step>-<what>`; the table lists the decisive ones.
All sit under the durable receipt directory named in the manifest.

| Probe | Page | Observed behaviour | Instrument | Captures |
| --- | --- | --- | --- | --- |
| 1. Duplicate derived slugs | `probe-nav-01-duplicate-heading.mu` | `#nav-probe-one` scrolled the **first** `Nav Probe One` heading to the top of the viewport, with `MARKER FIRST` visible and the second heading off-screen. | client; node log (0 requests) | `p01-b-link-focused`, `p01-c-after-jump` |
| 2. Explicit anchor duplicates a slug | `probe-nav-02a-heading-first.mu`, `probe-nav-02b-anchor-first.mu` | In both declaration orders the earlier binding won: with the heading first the jump reached the heading (02a), and with `` `:other-name `` first it reached the anchored line (02b). | client; node log (0 requests each) | `p02a-c-after-jump`, `p02b-c-after-jump` |
| 3. Missing anchor | `probe-nav-03-missing-anchor.mu` | Activating `#no-such-anchor-here` left the screen byte-identical (no scroll, no message, no request), while the present-anchor link on the same page scrolled. | client (`cmp` of captures); node log | `p03-b-missing-link-focused`, `p03-c-after-missing`, `p03-e-after-control` |
| 4. `#` below the last heading | `probe-nav-04-next-heading-tail.mu` | Activating `` `[label`#] `` from below `Last Heading` left the screen byte-identical with no request, while the same spelling at the top jumped to `First Heading`. | client (`cmp`); node log | `p04-c-after-top-jump`, `p04-i-tail-link-focused`, `p04-j-after-tail-jump` |
| 5. Target inside a closed section | `probe-nav-05-closed-target.mu` | Following a link to a heading or an explicit anchor inside `` `->Closed Outer `` opened that section (`▸` became `▾`) and scrolled the target to the top of the viewport, with no request. | client; node log | `p05-b-closed-rendering`, `p05-d-after-hidden-heading-jump`, `p05-e-outer-state-after-jump`, `p05-i-after-explicit-jump` |
| 6. Nested collapsible state | `probe-nav-06b-nested-collapsible.mu` (control `probe-nav-06a-same-depth-collapsible.mu`) | Opening the closed outer showed its depth-two collapsible headings in their authored states, and after the reader closed one of them, closing and reopening the outer kept it closed. | client | `p06b-a-loaded`, `p06b-b-outer-opened`, `p06b-c-down2-enter`, `p06b-e-outer-reopened`, `p06a-c-outer-opened` |
| 7. Leading `<` in a nested section | `probe-nav-07a-section-exit.mu`, `probe-nav-07b-section-exit-deep.mu`, `probe-nav-07c-section-exit-fold.mu` | A line of `<` or `<<` produced no row, the following body rendered flush left whether it came from depth 2, 3 or 4, and with the enclosing collapsible heading closed that following line stayed visible. | client | `p07a-a-loaded`, `p07b-a-loaded`, `p07c-a-loaded-closed`, `p07c-c-b-opened-by-click` |
| 8. Enter and Space | `probe-nav-08-toggle-keys.mu` | Enter and Space each toggled the focused collapsible heading open and then closed again, and focusing the heading changed nothing on screen. | client (`cmp` of focused and unfocused captures) | `p08-b-enter-target-focused`, `p08-g-space-1`, `p08-h-space-2`, `p08-i-enter-1`, `p08-j-enter-2` |
| 9. Transport on an anchor jump | `probe-nav-09-transport.mu`, `probe-nav-09-control.mu` | Two in-page anchor jumps scrolled the page and wrote no node log line of any kind, while the same-node link in the same run logged exactly one request. | node log; client status row | `p09-c-after-anchor-jump-1`, `p09-d-after-anchor-jump-2`, `p09-e-after-control-link`; logs `p09-*-node-log.txt` |
| 10. Location and Back | `probe-nav-10-location-back.mu` | The address row stayed `923706ddc70d389bd3719258c41f6592:/page/probe-nav-10-location-back.mu` with no fragment after the jump; Back then left for the previously loaded page and Forward returned to probe 10 at its top, both from cache with no request. | client; node log | `p10-a-loaded`, `p10-b-after-anchor-jump`, `p10-c-after-back`, `p10-d-after-forward` |

Supporting detail, each read directly off the captures:

- 1: the heading row sits at viewport row 1 after the jump; `pad b001` follows.
- 3, 4: `cmp` reported the ANSI captures before and after activation identical.
- 5: `p05-f-after-reload` and `p05-g-reload-closed-check` show `C-r` restoring
  `▸ Closed Outer`, so fold state did not survive a reload.
- 6: 06a shows that a depth-one `` `-> `` fold ends at the next depth-one
  collapsible heading: opening `Outer Closed` revealed only `MARKER OUTER`.
  In 06b the inner headings rendered two columns in with a different heading
  shade.
- 7: `< TEXT AFTER LESS-THAN` rendered as ` TEXT AFTER LESS-THAN`, flush left
  with its leading space. In 07b, depth-four body text sits six columns in and
  the line after a single `<` sits at column zero, not four. In 07c both folds
  were closed and both `MARKER ... AFTER <` lines were visible; opening them
  (`p07c-b-a-opened`, `p07c-c-b-opened-by-click`) placed each body above its
  `AFTER <` line.
- 8: a mouse click on `▸ Closed One B` in 07c also toggled it.
- 9: the status row kept the page's original load figures
  (`Done ▤ 1.39KB ↓1.39KB in 0.07s`) after each jump rather than `Done (cached)`.
  Five housekeeping lines (`Cleaning known destinations`) appeared between the
  two jumps, outside both brackets.
- 10: the panel title stayed `<923706ddc70d389bd3719258c41f6592>`. The stock
  text UI has an address row, so the location is observable; it does not change
  on an anchor jump, and Back behaves as if the jump were not a history step.
  Decision 1 of the navigation plan deliberately differs.

Incidental, not probes:

- A line holding only `` `:name `` rendered no row. A jump to it put the
  following line at the top of the viewport (02b, 03, 05).
- Heading background varies by depth: depth one, depth two, and depths three
  and four (which share one) are three different shades. Body text is indented
  two columns per level beyond one, so depth-zero and depth-one body text are
  both flush left.
- After a reload, Enter with no prior key toggled nothing
  (`superseded/p06a-d-enter-no-move`, taken against the pre-rename 06a page
  with the same heading lines), and a single Down followed by Enter or Space
  toggled the first collapsible heading (`p06a-c-outer-opened`, `p08-g-space-1`).

## Still ambiguous after capture

Each of these keeps its source and a diagnostic; none is a rule.

- **`<` section exit depth.** The line after `<` renders flush left from depths
  2, 3 and 4, and escapes a closed depth-one fold from depth two. Depth zero and
  depth one look identical in this UI, so the depth `<` sets is not separable
  here. Keep source and diagnostic.
- **`<<` versus `<`.** Both rendered the same in every case captured. Keep
  source and diagnostic.
- **`<` followed by text on the same line.** The remainder rendered as body
  text with its leading space. Whether that is an exit plus a body line or
  something else is not separable. Keep source and diagnostic.
- **The reference point of `#`.** From the top the link, the focus and the
  viewport were all above `First Heading`; from the tail they were all below
  `Last Heading`. Which of them "next" is measured from is not separated. Keep
  source and diagnostic.

## Not captured

- An explicit `` `:name `` on the line immediately after a heading (the
  rebinding form `syntax.rs` models); the collisions here placed the anchor 60
  lines away.
- Two explicit anchors with the same name; a target inside a closed section
  nested in another closed section; `anchor=` on a link to another page.
- Which element each keyboard Down focuses between collapsible headings. It is
  not visible on screen, so 06b and 08 name the heading a key toggled, not the
  steps taken. `p08-d-after-down` then `p08-e-after-space` changed nothing, and
  this receipt does not claim why.

The first two items were captured in [C1b](#c1b-2026-09-16) below.

## Commands

```text
nomadnet --daemon --console \
  --config /var/tmp/micron-c1-20260916/node/nomadnet \
  --rnsconfig /var/tmp/micron-c1-20260916/node/rns

TERM=xterm-256color nomadnet --textui \
  --config /var/tmp/micron-c1-20260916/client/nomadnet \
  --rnsconfig /var/tmp/micron-c1-20260916/client/rns
```

Both ran inside `tmux -S /var/tmp/micron-c1-20260916/tmux.sock -f /dev/null`.
Pages were opened through the client's `C-u` URL dialog as
`923706ddc70d389bd3719258c41f6592:/page/<page>`. The node, client, tmux server
and WSL keepalive were stopped after capture. The scripts that generated the
pages and drove the client are kept with the durable receipts.

## C1b (2026-09-16)

A follow-up batch run the same way, answering decision 8 of the navigation plan:
unnamed sections, `<<` and `< text` inside a fold, `anchor=` links to another
page, an explicit anchor after a heading, duplicate explicit anchors, and a
target inside two closed sections. Probes are numbered 11 to 17. The C1
observations above are unchanged; only C1's "Not captured" list gained a
pointer here.

### Runtime and isolation

- **Instrument reused and re-verified.** C1's virtualenv at
  `/var/tmp/micron-c1-20260916/venv` still existed and was used read-only.
  Before any capture it reported `Nomad Network Client 1.4.2`, its
  `nomadnet-1.4.2.dist-info/METADATA` digest **matched**
  `0916bffb68b508b13b92aec79f9660c35b74a9daa8c9e1e7c7b405ad35d584d2`, installed
  metadata reported `rns 1.5.3` and `lxmf 1.1.1`, the launcher digest was C1's
  `91f64c88…`, and C1's cached wheels matched the digests in the
  [manifest](CAPTURE_MANIFEST.md).
- **Fresh profiles.** Node, client, `HOME` and the tmux socket were new, under
  `/var/tmp/micron-c1b-20260916`, with RNS transport and instance sharing
  disabled. The node had a `TCPServerInterface` on `127.0.0.1:45550` and the
  client a `TCPClientInterface` to it, a port C1 did not use. Node
  `loglevel = 7`, client `glyphs = unicode`, browser fullscreen in 120x45 as in
  C1. The real WSL home's top-level listing was identical before and after.
- **Node destination** `5a77246c9fcc780468b44cff22c1ec51`, derived with the
  public RNS API and matched to the daemon's ready line. Probe 14d's link spells
  it out, so that single link names this run's node.
- **Client cache.** The client profile started empty, and every transport
  probe followed a link to a page path not visited before. The only repeat
  visits were deliberate and are recorded: page A of probe 14 by Back, 14a's
  target by Forward, and probe 17 by `C-r`. The node logged 17 requests in
  total, each accounted for in the table.
- **One page added mid-capture.** After 11a, `probe-nav-11c-unnamed-fold-depths.mu`
  was added to test the same rule at depths one and two. Every other page was
  proven byte-identical before the node was restarted once (10:24:43); the
  client kept running and reconnected.

### Observations

Captures are named `c1b-p<probe>-<step>-<what>` in the durable directory.
Request counts come from the node's own log, bracketed as in C1.

| Probe | Page | Observed behaviour | Instrument | Captures |
| --- | --- | --- | --- | --- |
| 11a. Unnamed section inside a closed fold | `probe-nav-11a-unnamed-fold.mu` | With the folds closed, the lines after a bare `>>>>` in a depth-one fold (A) and after a bare `>>>>` in a depth-four fold (B) were hidden, while the lines after a bare `>` in a depth-two fold (C) stayed visible. | client; node log (1 request, the load) | `c1b-p11a-a-loaded-closed`, `c1b-p11a-b-a-opened`, `c1b-p11a-c-b-opened`, `c1b-p11a-d-c-opened` |
| 11c. Unnamed section at other depths | `probe-nav-11c-unnamed-fold-depths.mu` | With the folds closed, the line after an equal-depth bare `>` in a depth-one fold (D) and an equal-depth bare `>>` in a depth-two fold (E) was hidden, and the line after a bare `>>` in a depth-four fold (F) stayed visible. | client; node log (1 request, the load) | `c1b-p11c-a-loaded-closed`, `c1b-p11c-b-d-opened`, `c1b-p11c-c-e-opened`, `c1b-p11c-d-f-opened` |
| 11b. `#` and an unnamed section | `probe-nav-11b-next-heading-unnamed.mu` | `` `[label`#] `` at the top of the page skipped the bare `>>>>` line and scrolled `Named After Unnamed`, sixty lines further on, to the top of the viewport. | client; node log (0 requests) | `c1b-p11b-b-link-focused`, `c1b-p11b-c-after-next-heading-jump` |
| 12. `<<` first inside a closed fold | `probe-nav-12-double-less-than-fold.mu` | With each fold closed, both lines after `<<` as the fold's first line stayed visible, as they did after `<` in the same position and after `<<` following a body line. | client | `c1b-p12-a-loaded-closed`, `c1b-p12-b-a-opened`, `c1b-p12-c-b-opened`, `c1b-p12-d-c-opened` |
| 13. `< text` inside a closed fold | `probe-nav-13-less-than-text-fold.mu` | With the fold closed, the text rendered as ` TEXT ON THE LESS-THAN LINE` (a body row, leading space kept) directly under the closed heading, followed by both later lines. | client | `c1b-p13-a-loaded-closed`, `c1b-p13-b-opened` |
| 14a. Cross-page `anchor=` | `probe-nav-14-source.mu` to `probe-nav-14a-target.mu` | Following `` `[…`:/page/probe-nav-14a-target.mu`anchor=cross-target] `` requested the target path without the field once, scrolled `Cross Target` to the top, and the address row read `` …:/page/probe-nav-14a-target.mu`anchor=cross-target ``. | client; node log (1 request) | `c1b-p14a-a-link-focused`, `c1b-p14a-b-after-follow` |
| 14a, Back and Forward | same | Back returned to page A at its top from cache (`Done (cached)`) with no request; Forward then requested the target again once and landed on `Cross Target` again. | client; node log (0, then 1) | `c1b-p14a-c-after-back`, `c1b-p14a-d-after-forward` |
| 14a, in-page control | `probe-nav-14a-target.mu` | B's own `#cross-target` link gave page rows identical to the cross-page landing. | client (`diff` of rows 5–41); node log (0 requests) | `c1b-p14a-e-b-top`, `c1b-p14a-f-after-inpage-control` |
| 14b. Cross-page missing anchor | `probe-nav-14b-target-missing.mu` | The target loaded once at its top, and the status row read `Unknown anchor: #no-such-cross-anchor`. | client; node log (1 request) | `c1b-p14b-b-after-follow` |
| 14c. Cross-page anchor inside a closed section | `probe-nav-14c-target-closed.mu` | The target loaded once with `▾ Closed Cross Outer` opened and its hidden `Closed Cross Target` heading at the top of the viewport. | client; node log (1 request) | `c1b-p14c-b-after-follow`, `c1b-p14c-d-outer-heading` |
| 14d. Cross-page anchor, full address | `probe-nav-14d-target-full-address.mu` | The same link spelled `` 5a77…:/page/…`anchor=cross-target `` behaved exactly as 14a: one request, `Cross Target` at the top, the field in the address row. | client; node log (1 request) | `c1b-p14d-b-after-follow` |
| 15a. Explicit anchor after a heading, same name | `probe-nav-15a-heading-anchor-same.mu` | With `>Setup` followed by `` `:setup ``, `#setup` scrolled the `Setup` heading row to the top, not the body row after the explicit anchor. | client; node log (0 requests) | `c1b-p15a-b-after-setup-jump` |
| 15b. Explicit anchor after a heading, other name | `probe-nav-15b-heading-anchor-other.mu` | With `>Setup` followed by `` `:install ``, `#setup` scrolled the `Setup` heading row to the top and `#install` scrolled the next row, `MARKER 15B BODY`, to the top. | client; node log (0 requests each) | `c1b-p15b-c-after-setup-jump`, `c1b-p15b-e-after-install-jump` |
| 16. Duplicate explicit anchors | `probe-nav-16-duplicate-explicit.mu` | `#dup` scrolled `MARKER DUP FIRST`, the row after the first of two `` `:dup `` declarations, to the top. | client; node log (0 requests) | `c1b-p16-c-after-jump` |
| 17. Target inside two closed sections | `probe-nav-17-nested-closed-target.mu` | Following `#nested-deep` opened both `Outer Closed` and the `Inner Closed` fold inside it (both `▾`) and scrolled the target row to the top. | client; node log (0 requests) | `c1b-p17-c-after-jump`, `c1b-p17-d-fold-states-after-jump`, `c1b-p17-e-closed-after-reload` |

Supporting detail, each read directly off the captures:

- 11a, 11c: no bare `>` run produced a row of its own. Opening each fold showed
  where the following lines sit. In A and B they follow the fold's body at six
  columns (depth four); in D and E they follow it at the fold's own
  indentation. In C they render flush left below the depth-two body, and in F
  two columns in below the depth-four body, so a shallower unnamed line set the
  depth of what followed it.
- 11b: the jump's origin was the link row at the top, so decision 6's reference
  point is not in question here.
- 12: opening the `<<`-first and `<`-first folds changed only `▸` to `▾` and
  added no row, so each fold's extent was empty; opening the third inserted only
  `MARKER 12C BODY`. Neither `<` nor `<<` produced a row.
- 13: the text row has body colour and no heading background. Opening the fold
  inserted only `MARKER 13 BODY`, above the text row, so the `<` line itself is
  outside the extent.
- 14: the status row showed each link's field before activation
  (``Link to :/page/probe-nav-14a-target.mu`anchor=cross-target``). Every Back
  to page A logged no request and showed `Done (cached)`. The Forward in 14a
  showed fresh load figures rather than `Done (cached)`.
- 14c: `c1b-p14c-c-outer-state` shows the outer body line above the target, and
  one more line up shows `▾ Closed Cross Outer`.
- 15a, 15b: the line holding only the explicit anchor produced no row in either
  page, so the heading row and the body row are adjacent and a one-row
  difference separates the two landings.
- 17: before the jump, one page down showed `▸ Outer Closed` directly above
  `Sentinel After`; after `C-r` (one request) the same view returned
  (`c1b-p17-e-closed-after-reload`), so the opened state did not survive a
  reload, as in probe 5.

Incidental, not a probe: a click on the non-collapsible `Sentinel E` heading
changed nothing (recorded in `c1b-client-keys.log`).

### Narrowed from C1's ambiguities

- **`<<` versus `<`**: both end a fold extent, both as a fold's first line and
  after a body line, and neither produces a row. They still render identically.
- **`<` followed by text**: the text is a body row outside the fold it follows;
  the extent ends before the `<` line.

### Still ambiguous after C1b

Each of these keeps its source and a diagnostic; none is a rule.

- **Depth set by `<` and `<<`.** Every C1b case sat directly under a depth-one
  heading, where depth zero and depth one render alike, so the depth each reaches
  is still not separable. Keep source and diagnostic.
- **`< text` as a construct.** The remainder renders as body text outside the
  fold, but whether stock reads it as a section exit followed by a text line or
  as something else is not separable. Keep source and diagnostic.
- **The depth an unnamed line is compared with.** Every captured case is
  consistent with "a bare `>` run ends a fold when it is shallower than the fold's
  heading". No case was shallower than the depth just before it without also being
  shallower than the heading, for example `>>` after `>>>` inside a depth-one
  fold, so a comparison with the preceding depth is not ruled out. Keep source
  and diagnostic for that arrangement.

Behaviour that is not a spelling, recorded but not separated: why Forward
re-requested a page reached by an `anchor=` link when C1's plain-page Forward
did not, and why a missing anchor shows `Unknown anchor:` when reached across
pages but nothing when followed in-page (probe 3).

### Not captured in C1b

- An explicit anchor on the heading line itself (`` >Setup`:install ``).
- `anchor=` combined with other link fields, or on a link to a different node.
- Keyboard focus order across the new pages; links were focused by stepping
  Down until the status row named them, as in C1.

### Commands

```text
HOME=/var/tmp/micron-c1b-20260916/home \
  /var/tmp/micron-c1-20260916/venv/bin/nomadnet --daemon --console \
  --config /var/tmp/micron-c1b-20260916/node/nomadnet \
  --rnsconfig /var/tmp/micron-c1b-20260916/node/rns

HOME=/var/tmp/micron-c1b-20260916/home TERM=xterm-256color \
  /var/tmp/micron-c1-20260916/venv/bin/nomadnet --textui \
  --config /var/tmp/micron-c1b-20260916/client/nomadnet \
  --rnsconfig /var/tmp/micron-c1b-20260916/client/rns
```

Both ran inside `tmux -S /var/tmp/micron-c1b-20260916/tmux.sock -f /dev/null`.
Pages were opened through the client's `C-u` URL dialog as
`5a77246c9fcc780468b44cff22c1ec51:/page/<page>`. The node, client, tmux server
and WSL keepalive were stopped after capture and no task process remained. The
`scripts/c1b-*` files in the durable directory generated the pages and drove the
client; `c1b-README.txt` there lists the receipts.
