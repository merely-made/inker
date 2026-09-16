# Navigation capture manifest

This directory contains original probe pages and a runtime receipt only; it
contains no NomadNet, RNS, or LXMF implementation source. Paths in commands
identify the isolated capture environment; fixture names in the digest table are
relative to this checked-in directory. Observations are in
[`NAVIGATION_RECEIPT.md`](NAVIGATION_RECEIPT.md).

## Inputs

| Input | Version/provenance | SHA-256 |
| --- | --- | --- |
| installed NomadNet `METADATA` | package version 1.4.2; identical to the 2026-09-12 [reference](../CAPTURE_MANIFEST.md) | `0916bffb68b508b13b92aec79f9660c35b74a9daa8c9e1e7c7b405ad35d584d2` |
| `/var/tmp/micron-c1-20260916/venv/bin/nomadnet` | runtime reports Nomad Network Client 1.4.2; pip launcher, digest depends on install path | `91f64c887c4e8bf591b5f9f18bb22a9cd4ab1f9f28bd71ec36dcd50800457257` |
| `nomadnet-1.4.2-py3-none-any.whl` | PyPI | `1895fe1057787de1a1f7aa9643ce7426880d2d392b271ce178afecbc40256fcd` |
| `rns-1.5.3-py3-none-any.whl` | PyPI | `0d02a0166b6f4d398549cb933153e7189c3fcde3c1fbcad03e47ce778091c351` |
| `lxmf-1.1.1-py3-none-any.whl` | PyPI | `3cdb4c5b3a4ec091ed538050228d8bd15db3ca4dfe7d69be3350ebc6c6d7e696` |

## Commands

```text
HOME=/var/tmp/micron-c1-20260916/home \
  /var/tmp/micron-c1-20260916/venv/bin/nomadnet --daemon --console \
  --config /var/tmp/micron-c1-20260916/node/nomadnet \
  --rnsconfig /var/tmp/micron-c1-20260916/node/rns

HOME=/var/tmp/micron-c1-20260916/home TERM=xterm-256color \
  /var/tmp/micron-c1-20260916/venv/bin/nomadnet --textui \
  --config /var/tmp/micron-c1-20260916/client/nomadnet \
  --rnsconfig /var/tmp/micron-c1-20260916/client/rns
```

Both RNS configs have transport and instance sharing disabled and a single TCP
interface on `127.0.0.1:45450`. The pages below were installed as mode 644
files in the node's `storage/pages` and fetched as
`923706ddc70d389bd3719258c41f6592:/page/<fixture>`.

## Fixture digests

| File | SHA-256 |
| --- | --- |
| `index.mu` | `556ceb4a3dc024789aeadb3b50f665790f49a013f73f93c8ece6fc7cf612682f` |
| `probe-nav-01-duplicate-heading.mu` | `f69ee6953e4fb2e1fa955430d051a2fc8effa67c2e5ae9a1e953caf1b9601201` |
| `probe-nav-02a-heading-first.mu` | `ba73d125dfa639ce30dec8f85ae5660919737d564fdebe7b796a25bd67be08ce` |
| `probe-nav-02b-anchor-first.mu` | `cfc96b3808fe4cc213955c501a2d412c0febf4ca900cd934b2015145a9e20cb6` |
| `probe-nav-03-missing-anchor.mu` | `152e4623142032543859ebe8abc9ce708d401f9c2b4504535fee63c64ae57f9c` |
| `probe-nav-04-next-heading-tail.mu` | `5aded37d59adbca25766b83825d40ff5c8938187a5107ac463aafbd6712d5b44` |
| `probe-nav-05-closed-target.mu` | `04b1f8396ad22720f999a10ea5ea7971e56eb248df51160a3302c35e308d861d` |
| `probe-nav-06a-same-depth-collapsible.mu` | `a6146ae46633993cf79f0ee0af61a90e432ad68ed40cfee0d80c152a20ff81cc` |
| `probe-nav-06b-nested-collapsible.mu` | `07afa5355c6b51caef598a89b4138f6ba008a9ec3660b7ee6b86ecde5ea1010a` |
| `probe-nav-07a-section-exit.mu` | `6f374ca4a462b315468f781f4c11dff748db69f34ec3608e9996e8dc67e62018` |
| `probe-nav-07b-section-exit-deep.mu` | `840c636067b4c11639fcaec46d2e51a2a67a936149b9d0cb68f391cf39c81053` |
| `probe-nav-07c-section-exit-fold.mu` | `454af31ec07750cfcaf615650d76f177d8057697a76b57d83b77255c8c167eca` |
| `probe-nav-08-toggle-keys.mu` | `2ced6a601fae60702224eda890030ea61bd0d11a462cc0ca8169411d8c07407a` |
| `probe-nav-09-control.mu` | `1459aaf04436846557fa36384a1e6c2113a57e22f6a91afacb0e9ddd87f5bd96` |
| `probe-nav-09-transport.mu` | `986b01a6083268b27531f8e7f0a1291514ab848759418289c0147a7833358018` |
| `probe-nav-10-location-back.mu` | `a270a8332a5e7bc4bc8a563c369993fa3b40b723037ba94e974574b7f89fd0a5` |

The pages are byte evidence under the `*.mu -text` rule in
[`../../.gitattributes`](../../.gitattributes), so these digests hold on any
checkout. The two Markdown files here have no such rule and are not digested.

Numbered `pad` lines are deliberate: a capture shows which pad numbers are on
screen, which pins the scroll position without measuring. Probes 06 and 07 were
each split during capture (06a/06b, 07a/07b/07c) after the first page proved
non-discriminating. Probes 01 to 05 and 08 to 10 kept identical bytes across
those regenerations, checked with `sha256sum -c` before each node restart.

## Durable receipts

Captures, node console logs, the derived request log, configs and the driving
scripts are outside the repository at
`C:\Users\mark_\Code\testing\mere\micron-navigation-20260916\`, with their
digests listed in its `SHA256SUMS`.

| File | SHA-256 |
| --- | --- |
| `SHA256SUMS` (228 entries) | `3cf889907eb5fdde4e6252dd6ff907c1de068f2cba4f82663ff28a8876e06118` |
