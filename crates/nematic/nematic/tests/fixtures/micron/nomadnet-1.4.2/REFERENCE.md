# Micron runtime reference capture

Captured on 2026-09-12 from NomadNet's actual terminal Guide, using a fresh,
isolated profile. This is a runtime/output capture, not a reading of NomadNet,
RNS, or LXMF implementation source.

## Runtime and isolation

- Executable: `/mnt/c/t/nomadnet-venv-wsl/bin/nomadnet`.
- Runtime-reported version: `Nomad Network Client 1.4.2`.
- Executable SHA-256: `1ff3899a0abeac8dea4b79677f9214a842903940a146c3d80ab9f153fee22d16`.
- Package-metadata SHA-256: `0916bffb68b508b13b92aec79f9660c35b74a9daa8c9e1e7c7b405ad35d584d2`.
- Installed package metadata: `nomadnet 1.4.2`, author Mark Qvist,
  home page `https://github.com/markqvist/nomadnet`.
- Dependencies reported by package metadata: `rns >= 1.5.3`, `lxmf >= 1.1.1`.
  Installed task-local metadata identifies `rns 1.5.3` and `lxmf 1.1.1`.
- The GitHub release index observed during earlier web research presented 1.2.0
  as latest. This capture is deliberately labelled 1.4.2 and must not be
  represented as a 1.2.0 receipt.
- Command:

  ```text
  TERM=xterm-256color /mnt/c/t/nomadnet-venv-wsl/bin/nomadnet --textui \
    --config /mnt/c/t/micron-reference-20260912/fresh/nomadnet \
    --rnsconfig /mnt/c/t/micron-reference-20260912/fresh/rns
  ```

- `fresh/rns/config` has no interfaces, transport disabled, and instance
  sharing disabled. The first NomadNet run created only its configuration
  beneath this artifact directory. No public, RF, or third-party probe ran.

The initial reused profile had `hide_guide = no` confirmed in the actual Config
UI but did not show the Guide because it was past first-run state. A fresh
profile entered the Guide and exposed its `Micron Markup` topic.

## Captured Guide syntax

Backtick is the Micron control prefix. The Guide states all source is UTF-8.

| Construct | Guide-captured contract |
|---|---|
| Alignment | At line start: `` `c `` center, `` `l `` left, `` `r `` right, `` `a `` document default. Alignment persists until changed. |
| Inline style | `` `! `` toggles bold, `` `_ `` underline, and `` `* `` italics. They can combine and nest. |
| Full reset | `` `` `` (two consecutive backticks) immediately removes all previously specified formatting. |
| Foreground/background | `` `Fxyz `` and `` `Bxyz `` use three hexadecimal digits. `` `f `` and `` `b `` return foreground/background respectively to their defaults. |
| Sections | A run of one or more `>` at the start of a line sets the section depth. Following text is a heading; no following text makes an unnamed, indented section. Dividers inside a section use its indentation. |
| Collapsible headings | `` `+>Heading `` is initially open; `` `->Heading `` is initially closed. It folds up to the next heading at equal or shallower depth. Enter, Space, or mouse toggles it. `` #!fold [open] [closed] `` selects page indicators. |
| Page headers | Early-line `` #!bg=X `` sets page background and `` #!fg=X `` default text color. The Guide says a `` #!bg `` header follows a cache header when both occur. |
| Link | `` `[target] `` uses target text as label. `` `[label`target] `` uses an explicit label; activation loads the URL. |
| Explicit anchor | `` `:name `` is zero-width. Name chars are ASCII letters, digits, `_`, `-`; any other character terminates it. An empty line containing it binds that row. |
| Header anchor | Every heading is an anchor: lowercase, runs of non-alphanumeric characters become one `-`, then leading/trailing hyphens are removed. First declaration wins on collision with an explicit anchor. |
| Anchor links | `` `[label`#name] `` scrolls current page to `name`; `` `[label`#] `` jumps to the next `>` heading. External-page example: `` `[Conclusion`DEST:/page/document.mu`anchor=conclusion] ``. |
| Tables | Enclose Markdown-style table lines in a begin `` `t `` line and a closing `` `t `` line. Opening tag may add alignment and max width, for example `` `tc30 ``. |
| Images | Block-level, own line: `` `(alt text`w=n`a=c`:/media/demo.webp) ``. `w`/`h` take columns/rows, percentage, or `n` native; `a` is `l`, `r`, or `c`. The Guide says network images must be WebP and clients without image support show alt-text placeholder. |
| Request link | A third backtick component after target carries pipes: `` `[Submit Fields`:/page/fields.mu`*] `` submits all page fields; names select fields; `key=value` supplies fixed variables. Example: `` `[Query the System`:/page/fields.mu`username|auth_token|action=view|amount=64] ``. |
| Text field | `` `<name`initial value> ``. No initial value: `` `<name`> ``. Width: `` `<16|name`> ``. Multi-row: `` `<40x5|name`> ``. Masked: `` `<!|name`initial> ``. The Guide describes `xROWS` as appended to width. |
| Checkbox | Example: `` `<?|field_name|value`> Label Text ``. Checked values sharing a field name are comma-concatenated for submission. Append `|*` after the field value to precheck. |
| Radio | Example: `` `<^|color|Red`> Red ``. Identical field names form a mutually exclusive group. Append `|*` after the value to preselect. |
| Comments | A line starting with `#` is omitted from output. |
| Partial | `` `{target} `` loads asynchronously after the page. `` `{target`10} `` refreshes every 10 seconds; omitted or `0` disables refresh. Additional backtick fields carry variables; `pid` can target a partial update. |
| Literal block | A line consisting of `` `= `` toggles literal content, which is displayed without Micron interpretation. |

## Rendered examples and interaction semantics

The Guide visibly rendered bold, italic, underline, three-digit colours,
combined nested styles, indented section levels, a Markdown-like table,
text/masked/multi-row fields, checkbox/radio controls, and an image fallback.
It explicitly says submitted fields and session variables are made available to
node-side scripts/programs as environment variables.

The Guide describes link activation, anchor scrolling, collapsible focus
activation, field selection, and partial refresh. The subsequent
[activation receipt](activation/ACTIVATION_RECEIPT.md) measures selected field
callback data and same-node link navigation. Raw request serialization,
nonempty multiline values, and partial-update lifecycle remain unmeasured.

## Capture limitations and next probes

- The Guide sentence describing a one-character inline escape was rendered in
  this terminal without a visible glyph. Do not infer its byte spelling from
  this capture. Probe a controlled page with candidate escaped control
  sequences and retain exact bytes/rendered text.
- This 1.4.2 capture documents six constructs beyond the prior subset, but it
  does not certify 1.2.0 compatibility. Obtain the actual 1.2.0 wheel/runtime
  artifact before making a 1.2-specific claim.
- The controlled dynamic node now has all-fields, explicit-field, fixed-variable,
  duplicate-checkbox, radio-default, empty and masked-field callback receipts.
  Add nonempty multiline input and raw request-value captures before admitting
  form submission in Retinue consumers.
- Test invalid/unmatched tags, cross-line state, colour validation, table edge
  cases, anchor duplicate/missing behavior, cache-header ordering, and partial
  cancellation/reload separately. Preserve unimplemented constructs in the
  shared semantic model rather than reducing them to plain text.
- The Guide qualified only `Fxyz` and `Bxyz` colours. A separately pinned opaque
  Go renderer parses a candidate `F12ab34` as `F12a` plus literal `b34`, so
  this capture provides no support for six-hex truecolour syntax. See
  [`BLACK_BOX_RENDER.md`](BLACK_BOX_RENDER.md).
- The standalone literal delimiter is Guide-qualified. The spelling of the
  escape glyph and the `<` section-exit token have cross-renderer evidence but
  still need a version-matched NomadNet UI receipt before promotion.
- Same-node `:/page/...` now has a stock-client activation receipt. Bare and
  relative destinations remain parse-only evidence.
