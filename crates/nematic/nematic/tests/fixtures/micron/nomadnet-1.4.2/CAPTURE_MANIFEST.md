# Capture manifest

This directory contains runtime/output receipts and original
small probes only; it contains no NomadNet, RNS, or LXMF implementation source.
Paths in commands identify the isolated capture environment; fixture names in
the digest tables are relative to this checked-in directory.

## Inputs

| Input | Version/provenance | SHA-256 |
| --- | --- | --- |
| `/mnt/c/t/nomadnet-venv-wsl/bin/nomadnet` | runtime reports Nomad Network Client 1.4.2 | `1ff3899a0abeac8dea4b79677f9214a842903940a146c3d80ab9f153fee22d16` |
| installed NomadNet `METADATA` | package version 1.4.2 | `0916bffb68b508b13b92aec79f9660c35b74a9daa8c9e1e7c7b405ad35d584d2` |
| `C:\t\micron-go-interop-20260912\gobin\view-mu.exe` | gmlewis/go-nomadnet v0.119.0, commit `56e711d53c474267a471c9d6dcdb0df691f81cf3`, go-reticulum v0.100.0; built opaquely | `6866928b94d7806e56ab0aa9e9c41f0153199b695139ca9047d15e95dd392939` |

## Commands

```text
TERM=xterm-256color /mnt/c/t/nomadnet-venv-wsl/bin/nomadnet --textui \
  --config /mnt/c/t/micron-reference-20260912/fresh/nomadnet \
  --rnsconfig /mnt/c/t/micron-reference-20260912/fresh/rns

C:\t\micron-go-interop-20260912\gobin\view-mu.exe -json \
  C:\t\micron-reference-20260912\fixtures\<fixture>.mu
```

The first command used a fresh profile whose RNS config has no interfaces,
transport disabled, and instance sharing disabled. The second command accepts
an existing local file and has no destination to resolve.

## Fixture digests

| File | SHA-256 |
| --- | --- |
| `guide-stable.mu` | `8e0fa8424e679d5848a6ef81e142916548f80b6be702df52e778b197e286e0f8` |
| `guide-structure.mu` | `47ee1779d3c727aa04b8d45d8101ccec0649241d272c3371fa11249e6b93c080` |
| `guide-links-fields.mu` | `95f3e93cab2dda77cad47b61893d2a296b381176136aa7338fd2017f78a6c165` |
| `probe-literals-escape.mu` | `efc04169bc5d7a821f102855bb856b3e7e4ae1392f8dc25a71071fc90b998111` |
| `probe-state-truecolor-section-exit.mu` | `c90497ece79dcd6b6cb40968eb9ce4bc5947cbc9fc185f656cf16eef23681e18` |
| `probe-link-resolution.mu` | `174c722ade8e764fe77d6c784092c9c6bc4e9b002d52a1e39fb893819c8aa126` |
| `guide-stable.go.json` | `6b54ba0d4477e56b16465ecfa0253de6eceec08ba4b9f1729911c5604d7b80e4` |
| `probe-table-style-state.mu` | `e558e6b907036062be3863e56d1df8d1946876385537735dc0f7f8d85cd179d7` |
| `probe-table-style-state.go.json` | `8101bc260776bc38b490c5111dde72969e8aa369310208a49ab32b9cd8adc1bb` |

`REFERENCE.md` carries the primary Guide-derived inventory. `BLACK_BOX_RENDER.md`
records only secondary opaque-renderer outcomes and limitations.

The two checked-in JSON files are stdout from `view-mu -json` with the matching
local Micron source. Their absolute source paths identify the capture location,
not a runtime requirement. Tests compare observable characters and attributes,
not the reference renderer's internal implementation.

## Activation receipts

| File | SHA-256 |
| --- | --- |
| `activation/ACTIVATION_RECEIPT.md` | `91f4f744c1621942aa08bc1fe12b161b84a48fd3dee57a693efe27c02c7159e` |
| `activation/logs/request-record.jsonl` | `afe9c279f3da619eb33061c504bea4b5f70709a69955c27d17c70ed05c168d33` |
