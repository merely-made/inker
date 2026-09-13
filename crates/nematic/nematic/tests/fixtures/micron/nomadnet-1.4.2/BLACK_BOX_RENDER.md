# Opaque renderer observations

On 2026-09-12, these fixtures were passed as local files to the separately
captured opaque executable:

```text
C:\t\micron-go-interop-20260912\gobin\view-mu.exe -json <fixture.mu>
```

The executable SHA-256 is
`6866928b94d7806e56ab0aa9e9c41f0153199b695139ca9047d15e95dd392939`.
It identifies local positional input as a `.mu` file. This is a renderer
comparison receipt, not primary documentation and not a NomadNet 1.4.2
client-to-node interaction test.

| Probe | Observed opaque-renderer result | Evidence limit |
| --- | --- | --- |
| alignment and two-backtick reset | `c` persists on subsequent lines; a two-backtick token restores left/default alignment and clears bold. | Matches the 1.4.2 Guide description. |
| `F3a3` / `B444` | JSON emits `#33aa33` foreground and `#444444` background. | Confirms the renderer's three-nibble expansion. |
| candidate `F12ab34` / `B12ab34` | JSON emits `#1122aa`; the remaining `b34` is visible text. | Rejects six-hex support in this renderer. It does not prove every NomadNet version rejects it. The 1.4.2 Guide only documented three hexadecimal digits. |
| standalone backtick-`=` | its delimiter lines are absent; enclosed `>`/link/style-looking text stays visible and unparsed. | Matches the 1.4.2 Guide literal-mode description. |
| backslash followed by backtick | `\`` renders as a literal backtick and the following style-looking text remains unstyled. | Cross-renderer evidence only; Guide glyph capture needs a version-matched visual receipt before treating the escape spelling as primary-qualified. |
| inline backtick-`=` | it does not open literal mode in this renderer; text after it remains normal output. | Candidate result only. |
| `<` after a depth-two section | following body has indent zero. | Cross-renderer evidence for a section-exit control. The 1.4.2 Guide capture did not include explanatory prose for this token. |
| table delimiters | backtick-`t` block is lowered to box-drawing table lines; nested inline color/bold styling remains present. | Establishes opaque rendering only. |
| table style state | In `probe-table-style-state.mu` bold starts before the table, reaches the header, is toggled off in a body cell, and stays off after the closing delimiter. | Exact JSON output is retained alongside the input. |
| link/field syntax | JSON retains `{label,url,fields}`. It preserves bare, relative, and `:/page/...` destinations exactly rather than resolving them. It also exposes field descriptors for text, checkbox, and masked text. | The renderer did not recognize the Guide's radio form and did not expose multiline rows, collapse behavior, or request dispatch. |
| collapse syntax | `+>` and `->` appeared as text in this renderer. | Do not use this renderer to reject the version-matched Guide syntax. |

No HTTP, Reticulum, LXMF, radio, or public network action occurred. The local
renderer never resolves a destination or executes a request.
