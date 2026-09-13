# Micron form field fixtures

Copied unchanged from Retinue `2563202b7d004395adc7021a2e1bfca6f76664d7`,
`crates/retinue/tests/fixtures/micron_forms/`.

The JSON maps were observed from unmodified NomadNet 1.4.2 / RNS 1.5.3
submitting this authored page to a controlled public RNS request handler on
2026-09-13. Defaults, Unicode and multiline edits, checkbox/radio selection,
and named selectors plus fixed variables were captured independently of
Nematic. Retinue retains the actual decrypted MessagePack envelopes and hashes.
No GPL/AGPL implementation source was read.

These fixtures qualify field-map preparation, not native widgets, destination
resolution, transport, or arbitrary Micron forms. See the Retinue receipt:
`design_docs/2026-09-13_nomadnet_go_resource_compression_receipt.md`.

`unchecked.mu` and `unchecked-handler.jsonl` are a second public-handler
capture from stock NomadNet 1.4.2 / RNS 1.5.3 on 2026-09-13. The authored page
has no prechecked markers. The first map is from Submit selected; the final
map is from Submit all. Both omit unchecked checkbox/radio groups. Submit all
retains empty, masked and multiline text. The first click was initially
misidentified, then audited against the selector and repeated at the exact
Submit all row; the test retains both observations. This is a public-handler
value capture, not a raw-wire fixture. Scratch reproduction files are in
`C:/t/micron-forms-unchecked-20260913`; no licensed implementation source was read.
