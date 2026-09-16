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
