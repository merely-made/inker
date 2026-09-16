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

C1b (2026-09-16) reused this virtualenv unchanged: before capture it reported
Nomad Network Client 1.4.2, and the `METADATA`, launcher and cached-wheel
digests above all matched (`logs/c1b-instrument-verify.txt` in the durable
directory).

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

C1b ran the same commands against fresh profiles under
`/var/tmp/micron-c1b-20260916` (the binary stayed C1's), with `HOME` there and
the loopback pair on `127.0.0.1:45550`. Its node destination was
`5a77246c9fcc780468b44cff22c1ec51`. The receipt's C1b section gives the exact
command lines.

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
| `probe-nav-11a-unnamed-fold.mu` | `dcb6613764826b62193bc6097f4d2d030fad1e3cd0e530930237966b1ff24a9c` |
| `probe-nav-11b-next-heading-unnamed.mu` | `ebd88a5d274ca7a282741084ccd4243621438ed56ee4e458aa06b6e37e84e70d` |
| `probe-nav-11c-unnamed-fold-depths.mu` | `8cd3404f99c5ad930d4238d633059271bbce9d7aec353d92295c8e5e0253c824` |
| `probe-nav-12-double-less-than-fold.mu` | `c2696a1b311c541541299559d4fd616d81ec43c4ed8810cfea1d36129104285b` |
| `probe-nav-13-less-than-text-fold.mu` | `5edae4501e312e0e82d5d152abb4e74e86bcae0dc017f8868ed7c89652fc6b10` |
| `probe-nav-14-source.mu` | `6eba598220aa9ac5233e29689cf75c57b1ecc44f50a40289f116aa3872ff3287` |
| `probe-nav-14a-target.mu` | `cdef79d254eb514ab7a600566614705313379519f1547e48168da54fc634b688` |
| `probe-nav-14b-target-missing.mu` | `b48f455a25b6f5a2627bbe211925396ae976aacdebbf432d9141cc256e0ab5d1` |
| `probe-nav-14c-target-closed.mu` | `ccc587853890f06eb254f074c6f56eed3bab9ce3d95d1fa1d6518bd3c10513b9` |
| `probe-nav-14d-target-full-address.mu` | `3c7bcb6559973cb261ed2031e24b2d085dda109c21672331a2728572d9246d88` |
| `probe-nav-15a-heading-anchor-same.mu` | `bd66d12a5c5572bb997f3bd5c4e450614192eed2d8a5232ae328bcb2d46396e2` |
| `probe-nav-15b-heading-anchor-other.mu` | `3b61d739774ddf382dec427dbbf5062b28d01da6a2b0f9e98a6903b0a1064907` |
| `probe-nav-16-duplicate-explicit.mu` | `c743ebd44d625aec315a51ae6a79564f0133eb78756c40250ea97e3025eb3a45` |
| `probe-nav-17-nested-closed-target.mu` | `6fe4b3fc6bddeb438eb798107f69f6d63eab09d75d80c56ae7447734e4765d80` |

The pages are byte evidence under the `*.mu -text` rule in
[`../../.gitattributes`](../../.gitattributes), so these digests hold on any
checkout. The two Markdown files here have no such rule and are not digested.

Numbered `pad` lines are deliberate: a capture shows which pad numbers are on
screen, which pins the scroll position without measuring. Probes 06 and 07 were
each split during capture (06a/06b, 07a/07b/07c) after the first page proved
non-discriminating. Probes 01 to 05 and 08 to 10 kept identical bytes across
those regenerations, checked with `sha256sum -c` before each node restart.

The C1b pages (11 to 17) come from a separate generator,
`scripts/c1b-gen_pages.py`, which writes nothing else. The sixteen C1 pages
were checked against the digests above before and after every C1b generation.
Page 11c was added during capture, and every other page was proven unchanged
before the node restart it needed. `probe-nav-14-source.mu` is the one page
bound to a run: its 14d link spells out the C1b node destination
`5a77246c9fcc780468b44cff22c1ec51`. Its other three links are node-relative.

## Durable receipts

Captures, node console logs, the derived request log, configs and the driving
scripts are outside the repository at
`C:\Users\mark_\Code\testing\mere\micron-navigation-20260916\`, with their
digests listed in its `SHA256SUMS`.

| File | SHA-256 |
| --- | --- |
| `SHA256SUMS` first 228 lines, C1 (`head -n 228`) | `3cf889907eb5fdde4e6252dd6ff907c1de068f2cba4f82663ff28a8876e06118` |
| `SHA256SUMS` (388 entries, after C1b) | `3c55d4c09e1b42dfed8f01557082c90757b1977e0aa24109813c3b2acade81e1` |

C1b appended its 160 entries (every `c1b-*` file) after C1's lines rather than
regenerating the file, so C1's recorded digest can still be checked. The
`c1b-README.txt` beside `README.txt` describes the C1b receipts.
