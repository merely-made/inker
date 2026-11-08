# Inker

Inker turns an address into a document a host can show. It selects the engine
for a resource and orchestrates it, runs the reader and smolweb lanes, lays the
result out on the document canvas, renders the smolweb (Gemini, Gopher,
Markdown, feeds, the knot note format) and lexes text for highlighting. It
emits a document, a predicate-bearing link or a JSON-LD block with its base
address; Mere's engine consumes the RDF and the host decides what to keep.

Inker is recorded as the owner of the scrying and welding engines. They stay in
their own repositories (`wgpu-scry`, `wgpu-weld`, engine split plan D21); Inker
carries the adapters that drive them.

## Crates

| Path | Package | What it is |
|---|---|---|
| `crates/inker/inker` | `inker` | Engine selection and orchestration, the per-address multiplexer and the engine flip |
| `crates/inker/document-canvas` | `document-canvas` | Parley layout of an Inker document into portable render packets |
| `crates/inker/engines/scrying-engine` | `scrying-engine` | Surface engine over the scrying system webview (WebView2, WKWebView, WebKitGTK) |
| `crates/inker/engines/graft-engine` | `graft-engine` | Surface engine over the wgpu-graft Servo embedding |
| `crates/inker/engines/weld-engine` | `weld-engine` | Surface engine over the wgpu-weld CEF embedding |
| `crates/nematic/nematic` | `nematic` | The smolweb rendering engine |
| `crates/nematic/illume` | `illume` | Wasm-safe lexer and highlighter (djot, injected languages, Markdown reuse-lexer) |
| `crates/system/document-lanes` | `mere-lanes` | The reader and smolweb sessions, their reading types and palette, and the remote fetcher |
| `crates/system/page-load` | `mere-pageload` | The fetch vocabulary and the sans-IO page load |
| `crates/fleece` | `mere-fleece` | Bounded traversal, clipping and capture over supplied documents |
| `crates/script/rhai` | `script-rhai` | The Rhai block evaluator and its command batches |

`knot-editor-host` retired 2026-11-07 under Mere's engine split plan D17; the
editor lives in Knot's `knot-document`, the lexers in `illume`.

The moved packages keep their names. Design documents are under
`design_docs/inker_docs/`, `design_docs/nematic_docs/` and
`design_docs/verso_docs/`.

## Where it came from

The crates and the three design areas were split out of
[Mere](https://github.com/merely-made/mere) at `f8c06097e` with their history
(`git filter-repo` on their paths, which stay as they were in Mere), under
Mere's engine split plan
(`design_docs/mere_docs/implementation_strategy/2026-11-07_engine_split_plan.md`,
phase P1, decisions D16, D17, D19, D21). History from before a path arrived at
its Mere location is in Mere's and genet's own histories.

One edge into Mere remains while the split finishes: `mere-fleece`'s optional
`structured-data` feature names `mere-rdf` (the plan's F3), from `mere.git`;
the root manifest's `[patch."https://github.com/merely-made/mere.git"]`
table serves the Inker packages it names back from this tree. `mere-lanes`
takes `tinct` and `script-rhai` takes `hest` from Graphshell's contract root,
which D19 allows.

## Building

```sh
cargo check --workspace --all-targets
cargo test --workspace
```

The toolchain is pinned in `rust-toolchain.toml`. Licensing is MPL-2.0; see
`LICENSE` and `LICENSES.md`.
