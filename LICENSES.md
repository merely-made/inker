# Licenses in this repository

**This repository: MPL-2.0.** Every file Mark wrote carries Exhibit A and the
SPDX tag `MPL-2.0`, per the license posture brief of 2026-08-22 (in Mere's
`design_docs/2026-08-22_license_posture_brief.md`). The full text is in
[`LICENSE`](LICENSE).

This file is the provenance ledger, carried from Mere's `LICENSES.md` at
`f8c06097e` when the Inker crates moved here with their history (engine split
plan, P1). Only the rows for paths in this repository came with it; Mere's
ledger keeps the rest. Provenance comes before license: a file gets Exhibit A
only if Mark wrote it.

## Retained licenses

Third-party files keep their own license and their own notices. Nothing here is
relicensed, and nothing here receives a Merely copyright line.

| Path | License | Upstream | Notice files |
|---|---|---|---|
| `crates/inker/document-canvas/tests/assets/Roboto-Regular.ttf` | Apache-2.0 | Google's Roboto 2.137 (2017), "Copyright 2011 Google Inc. All Rights Reserved." | `LICENSE-APACHE` and `README.md` beside it |

The font is test data: `document-canvas`'s text test registers it as a generic
family with system fonts off. In Mere it sat at
`crates/cambium/examples/genet_web_smoke/assets/Roboto-Regular.ttf` with no
licence file beside it (the plan's F5); its own name table gives the licence.

## Published grants that arrived with a crate

A published version keeps the grant it shipped with, permanently; MPL-2.0
reaches it at its next functional bump, per the sweep plan's invariant 8.

| Crate | Path | Published version's grant | In-tree notice files |
|---|---|---|---|
| `inker` | `crates/inker/inker` | 0.1.1, MIT OR Apache-2.0 | none (the pair carried over from `verso-tile` 0.1.0 was dropped by merge `0a8198ba`, 2026-09-06, and stays removed under ruling S40) |
| `document-canvas` | `crates/inker/document-canvas` | 0.1.0 | none |
| `nematic` | `crates/nematic/nematic` | 0.1.1 | none |
| `illume` | `crates/nematic/illume` | 0.0.2 | none |

Each arrived in Mere from genet on 2026-09-03 under its own published grant;
relicensing is the sweep plan's call at the next bump. The versions already on
crates.io keep the grant they were published under.

The three engine adapters, `scrying-engine`, `graft-engine` and `weld-engine`,
are `publish = false` and have no published version, so nothing is held open
for them.

## Exceptions under the fork/vendor criterion

**None.** The brief's test (a crate stays MIT OR Apache-2.0 only when a third
party would need to *modify or vendor* it rather than merely link it) admits
nothing here. `illume` was proposed and declined on 2026-08-22; what it carries
is the published-grant disposition above.

## How to add a file from elsewhere

1. Do not delete or rewrite the upstream copyright or license notice, ever.
2. Add its path to **Retained licenses** above with its license, upstream URL,
   and where its notice text lives.
3. If it is a substantial derivative rather than a verbatim import, the brief's
   rule is MPL-2.0 on the derivative *with the upstream notice retained*;
   record it here with that disposition.
4. Never add `license-file` to an owned manifest; the field is for retained
   third-party crates only.
