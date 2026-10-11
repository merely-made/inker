# Fleece

Fleece gathers supplied content through clipping, bounded traversal and capture.
The default library reexports Genet's lightweight extractor and provides a pure
traversal runtime. Document/engine providers supply content and relationships;
selectors rank admitted candidates against a user's goal or query. Source IDs,
versions and observation receipts remain distinct. Traversal needs no fetcher.
Goal ranking may reach a reference through a deep path before finding a shorter
one. Traversal tracks the minimum admitted depth and reobserves it only when that
depth improves, preserving both receipts. Those conservative repeats consume
the same node budget; pending references are updated without duplicate calls.

The native `crawl` feature supplies an explicitly started HTTP source adapter with
configurable request, response-byte, total-byte, depth, page, fanout, delay and
deadline bounds. Robots and sitemap requests consume those same budgets. Robots
are cached per origin; unreachable policies deny acquisition, while a typed
missing-file response allows it. Sitemap seeding remains subject to admission
bounds. Requests never follow ordinary browsing automatically.

Hosts inject a single-hop adapter that streams within the supplied byte cap,
identifies with the crawler User-Agent, and reports received bytes on failures.
It must not hide redirects or retries. Runtime checks reject an adapter's
oversized result and emit typed exhaustion/cancellation receipts. Cancellation
and deadlines drop the in-flight future; the adapter owns aborting its I/O.
Cancelled/timed-out operations cannot report their unfinished body's exact byte
count. Their summary marks `received_bytes_complete = false`, making the reported
bytes an explicit lower bound; their supplied cap remains bounded by the total
allowance remaining at admission, and acquisition stops immediately.
Mere's ready-made `crawl::fetch_crawler_request` bridges its single-hop transport
to this seam, preserving absent-file statuses and partial-byte errors.

The optional `crawl-graph` adapter binds generic gathered contributions to a
destination GraphId. It filters superseded run receipts before routing output.
The owning application/session authorizes graph mutations. Gathering itself
requires neither graph contributions nor graph IDs.

`clip` supplies portable capture over caller-owned document/engine inputs.
`eidetic-bridge` assembles retained content and provenance; `structured-data`
projects supplied source evidence. Durable identity, storage and session write
authority remain with their respective Mere owners.

`portable-clip` adds the first Knot/Redshank intake and selected-copy draft.
Eidetic owns its typed records and built-in schema definitions. Fleece retains
caller-supplied bytes, preserves native source selectors and evidence, and
exports only approved parts plus their declared dependencies. Complete source
backing occupies a separate local inventory. The wire package carries selected
bytes and content IDs; local custody manifests stay at the source. Receivers
check all required hashes and budgets before depositing material. Repeated
content reuses existing custody instead of rewriting its manifest.

The caller supplies part, byte and record limits, unique capture identity,
provenance and a host sealer when required. These helpers perform no acquisition,
session admission, graph writes or grant issuance. Raw parts have media types;
foreign typed parts currently refuse import. Source tags remain related facts;
asserted tags retain their original concept, target and attribution. Knot's
projection recipes, generated suggestions and reviewed reconnect edits need
their later owner adapters. Voice-note bytes must be supplied separately;
Redshank recording references do not imply retained recording bytes or exact seek.
Validated writes can leave immutable objects on a storage failure; the helpers
do not claim an atomic application transaction.

See the [development plan](../../design_docs/eidetic_docs/implementation_strategy/2026-10-11_eidetic_development_plan.md)
and the owner-generated [transfer fixtures](tests/fixtures/portable_clip/README.md).

Robots matching includes agent groups, allow/disallow precedence, wildcards,
end anchors, query strings and unreserved percent escapes. It is still a bounded
subset of [RFC 9309](https://www.rfc-editor.org/rfc/rfc9309.html), rather than a full
conformance implementation. SameDomain scope conservatively admits the seed
host and its descendants; it does not infer sibling domains with a Public
Suffix List. Sitemap index recursion is not implemented.
