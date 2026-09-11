# NomadNet Micron black-box capture manifest

Captured 2026-09-11 with task-local stock fixtures only.

## Runtime

- Server: Python NomadNet text UI 1.4.2; task-local config: /tmp/knot-nomadnet-20260911/cap-node.
- Independent client and renderer: view-mu, built but not inspected from released gmlewis/go-nomadnet tag v0.119.0; diagnostic reports go-reticulum 0.100.0.
- Node destination used: 7fc8950ec4e50695be76eddc8e539ee8.

## Byte fixtures and observations

### index.mu

- Path: cap-node/storage/pages/index.mu
- Exact bytes in Base64: UGxhaW4gTWljcm9uIGZpeHR1cmUK
- SHA-256: 7af10886c9d3bdead1abd3801fe3b54d0e0a9035bc21a6166c367ecdf653badb
- Remote raw response: remote-index.mu. Byte equality checked with cmp -s.
- Stock Python Preview text: Plain Micron fixture.
- Go structured output: index.go.json. SHA-256: 318bc5dc123a66ff1b85291b048c34722f92e21d66f8a17f06b00c0ce18f4adb.

### probe.mu

- Path: cap-node/storage/pages/probe.mu
- Exact bytes in Base64: PiBIZWFkaW5nCi0tLQpwbGFpbg==
- Stock Python Preview: heading text Heading in heading background; a full divider line; then plain.
- Go structured output: probe.go.json. SHA-256: c80dc6fb6c1d711fcc1a4ef1ddce2f769403f6de310a60a3dd53faee3df52382.
- Qualified rules: greater-than heading level 1; three-dash divider; unmarked plain line.

### styles-links.mu

- Path: cap-node/storage/pages/styles-links.mu
- Exact bytes in Base64: cGxhaW4gYCFib2xkYCEgcGxhaW4KcGxhaW4gYCppdGFsaWNgKiBwbGFpbgpbTG9jYWwgcGFnZWA6L3BhZ2UvcHJvYmUubXVdCltSZW1vdGUgcGFnZWA3ZmM4OTUwZWM0ZTUwNjk1YmU3NmVkZGM4ZTUzOWVlODovcGFnZS9wcm9iZS5tdV0=
- Stock Python Preview visibly applies bold to the text between control-plus-exclamation pairs and italic to text between control-plus-asterisk pairs.
- Go structured output: styles-links.go.json. SHA-256: 065ec04e84928ad93f78c5cfc86bed05d8b23e73750e5c7801706fd1788d1461.
- Link candidate labels and path text render, but activation and resolved target have not been captured.

### large.mu

- Path: cap-node/storage/pages/large.mu
- Size: 131072 bytes.
- SHA-256: 51db52a11208ab47ac1a136991b76135984a3debf84a2959f81fe742be582691.
- Remote raw response: remote-large.mu. Byte equality checked with cmp -s.
- Client diagnostic: Received 131072 bytes. Transfer mode was not exposed, so this is not a Resource-mode claim.

## Boundaries

No Micron grammar beyond the listed qualified rules is implied. No NomadNet request envelope, link activation or relative resolution, forms, directives, tables, or Resource-selection rule is claimed. Reticulum wire payloads remain encrypted and were not decoded.