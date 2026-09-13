# Micron Guide-derived fixtures

These short fixtures use original test text. They preserve the leading backtick
in raw `.mu` files, which Markdown would otherwise obscure. Their primary
evidence is the stock NomadNet 1.4.2 text UI Guide capture recorded in
[`REFERENCE.md`](REFERENCE.md). The opaque Go renderer is only a
black-box comparison aid; it does not replace the Guide as documentation.

| Fixture | Evidence class | Purpose |
| --- | --- | --- |
| `guide-stable.mu` | Guide-qualified | line alignment, inline styles, three-hex colours, colour resets, and the two-backtick full formatting reset. |
| `guide-structure.mu` | Guide-qualified | `>` section headings, open/closed collapses, divider, and backtick-`t` Markdown-table delimiters. |
| `guide-links-fields.mu` | Guide-qualified syntax; selected activation captured separately | link and anchor delimiters, text/checkbox/radio fields, and request selector positions. The unprefixed link-shaped line is a negative control. |
| `probe-literals-escape.mu` | Mixed | standalone backtick-`=` delimiter is Guide-qualified. Backslash escaping and inline backtick-`=` are explicit candidates only. |
| `probe-state-truecolor-section-exit.mu` | Mixed | two-backtick reset is Guide-qualified. `<` section exit and six-hex colour forms are explicit candidates only. |
| `probe-link-resolution.mu` | Parse-only candidate | bare, relative, and same-node link destination spellings. It cannot establish resolution without a client-to-node transaction. |

The fixtures state document syntax. The separate
[activation receipt](activation/ACTIVATION_RECEIPT.md) establishes stock-client
same-node navigation and selected callback data for text, empty, masked,
checkbox, radio and fixed-variable submissions. It does not establish raw
request serialization or nonempty multiline values.

The separately captured renderer outcomes are recorded in
[`BLACK_BOX_RENDER.md`](BLACK_BOX_RENDER.md). In particular, it accepts
the leading-backtick local-link form and leaves the deliberately unprefixed
negative control inert, but it does not resolve any destination.
