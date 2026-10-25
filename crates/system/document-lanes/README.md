# mere-lanes

Mere's document content lanes above Genet, split out of `genet-documents` on
2026-09-02 under the platform boundary plan (mere
`design_docs/archive_docs/2026-10-06_completed_plans/2026-09-02_platform_boundary_and_repository_topology_plan.md`,
P1). The application lanes live here; Genet retains the engine contracts.

- `ReaderSessionEngine` / `ReaderDocumentSession` (`genet.reader`): held HTML
  through fleece into the portable document canvas.
- `SmolwebDocument` / `SmolwebSessionEngine` (`smolweb` feature): protocol
  content through Nematic into the same canvas over the errand transport.
- `RemoteFetcher`: the remote half of a host's resource fetcher, http(s) over
  netfetcher (`netfetch` feature) and the smolweb schemes over errand
  (`smolweb` feature). Compose it under `genet_documents::LocalFetcher::with_fallback`.

Every session here implements `document-session-api`, the engine-facing
contracts Genet owns; the lanes themselves are application session and
routing, which is why they are Mere's.

Reader, small-web prose, and normalized-document projections start at 18 logical
pixels with 1.6 line spacing. Their centered content column is capped at 648
logical pixels (36 em at the default size), with 24-pixel gutters that the layout
clamps to the current appearance. Headings use tighter 1.25 leading and retain
level hierarchy; code has a separate, smaller monospace role. Explicit document
styles and authored typography can override this baseline. Ordinary authored
HTML browser sessions keep their page layout.

A host choosing a reading face does not replace the code face or a fixed-width
menu's column grid. Pelt can receive explicit generic font bindings with
`sessions_with_generic_fonts`; those host-held bytes remain available after
reflow, including a sans-serif fallback for glyphs absent from the prose face.
The Netrender adapter emits long shaped lines as bounded text draws, preserving
absolute glyph positions and font resources while avoiding repeated whole-line
paths in retained tiles at large native sizes.
A native host can resolve those logical reading lengths against its display
scale per appearance, while its physical viewport, hit coordinates and producer
raster scale remain unchanged. The original style intent is retained. No remote
or ambient font acquisition is added. Application chrome uses the
separate [Cambium shell roles](../../cambium/cambium/src/shell.css): 13-pixel
controls, 12-pixel secondary text, and opt-in reading and Inspector classes.
