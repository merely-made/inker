# Current browser family prerequisite, 2026-10-07

Current published Mere main d041cc69 is merged into the owned compatibility
worktree. The old branch contributes no runtime Rust changes. All 38 active
Genet manifest rows adopt 965b64e206a47d1c8808472de9aa461233638768;
the optional Welding alias adopts c4dd593b7a730acb4aa6ffab0fe2edf8a0584083.

The initial and final resolvers pass. The final locked neutral all-target
check covers Mere, graft-engine and weld-engine; all four Graft and three
Weld library tests pass. Existing pictograph dead-code and unused Boa patch
warnings remain. The CEF-enabled Mere adapter is a downstream Turnstone gate.
Weld's exact source separately passed its current-CEF typed/runtime tests and
sandboxed native imported pixel controls in its own repository.

`family-lock-delta.json` compares the final lock with current published main.
Only wuff 0.2.9 and wuff-capi 0.2.0 enter the registry graph, replacing
fontsan-woff2 0.1.1; all unchanged registry package checksums remain identical.
The independently pinned dormant Djinn/Knot subtree still carries older Genet
packages in this full workspace lock. The enabled Knot/Redshank/Turnstone
consumer graph must establish one shared type family after their repins.

Result JSON files record commands, UTC intervals, exit status and BelowNormal
priority. Metadata JSON and logs preserve successful and warning evidence.
`family-inputs.json` hashes the final four manifest/lock files; the source ZIP
preserves their exact bytes. This prerequisite does not claim final consumer,
current Servo native, cross-platform accessibility or publication-release proof.
