# document-canvas

`document-canvas` is the document-view canvas for inker's block model
(genet engine family; hosts: turnstone's mere, pelt). It owns *within-document
layout*: parley-driven text shaping, simple block stacking, link
interaction regions, and render-packet derivation for downstream
rendering. Sibling to [`graph-canvas`](https://crates.io/crates/graph-canvas)
in the canvas-swatches taxonomy.

## What it owns

- **Layout** of every [`inker::DocumentBlock`] variant (structural +
  semantic) into positioned glyph runs and block bounds within a supplied
  viewport.
- **Text shaping** via [parley] (which uses skrifa + swash underneath).
  CPU-only; no GPU dependencies. Works on `wasm32-unknown-unknown`.
- **Interaction regions** — hit-testable rectangles over inline links and
  block-level navigation targets, ready for the host to translate into
  click / hover events.
- **Render-packet derivation** — emits a portable `DocumentRenderPacket`
  that downstream consumers can render through netrender, gpui, or any
  other backend. The packet is *what's where on screen* in pixel-space
  terms, not a paint command list.

## What it does NOT own

- **Rendering** — it emits packets, not pixels. Downstream backends
  (`netrender::Scene` emitter, gpui native consumer, AccessKit projector)
  consume the packet.
- **Scrolling / viewport management** — the consumer supplies the viewport
  rect; document-canvas lays out within it but doesn't manage scroll
  position, viewport interaction, or content-overflow strategy.
- **Editing / interaction state** — selection, cursor, IME are the host's
  job. Document-canvas produces the layout the host hangs interaction off
  of.
- **Network / I/O** — `inker::EngineDocument` arrives already-parsed.
- **Accessibility tree** — uxtree consumes the same `EngineDocument`
  separately for AccessKit projection. Document-canvas is the *visual*
  side; uxtree is the *semantic* side. Both sides see the same input.

## Host font resources

`layout_document_with_folds_and_font_resources` combines ordinary host faces
with explicit `(generic, bytes)` resources. `text::register_generic_fonts`
validates the whole batch in an isolated context before changing the caller's
mapping and returns `GenericFontError` for unsupported generics or invalid
bytes. Supported bindings are `sans-serif`, `serif`, `monospace` and `system-ui`.
No native or browser-global fallback is installed.

Retained consumers such as `SmolwebDocument` keep that ledger across viewport
reflow; consumers rebuilding a document for appearance changes must restore the
same admitted resources. The `FontTable` contains the face Parley actually
selected, and packet glyph IDs reference that sidecar. Downstream scene or
PaintList lowering must carry those same bytes. Authored typography remains a
request to this resource owner, rather than permission to substitute a different
render-side face. Proper serif/monospace and script coverage remain explicit
host face-selection and qualification decisions.

## Canvas swatches

A canvas in Mere is a self-contained renderable unit that knows how to lay
itself out. Document-canvas + graph-canvas are siblings; both consumed by
[`platen`](https://crates.io/crates/platen) for workbench composition,
both embeddable inside knots, settings panels, sidebars, etc. Each canvas
owns its internal layout; platen owns *which canvas swatch goes in which
frame pane*.

## Status

Pre-1.0. v1 implements layout for the structural [`DocumentBlock`]
variants (`Heading`, `Paragraph`, `List`, `Quote`, `CodeBlock`, `Image`,
`Preformatted`, `Rule`); semantic variants (`FeedHeader`, `FeedEntry`,
`MetadataRow`, `Badge`) lay out as composed structural patterns. Out of
scope for v1: bidi edge cases, inline-image flow inside paragraphs, table
layout, scrolling logic.

## License

MPL-2.0 (see the repository `LICENSE`).

[parley]: https://crates.io/crates/parley
[`inker::DocumentBlock`]: https://crates.io/crates/inker
