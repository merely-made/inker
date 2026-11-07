# Test font

`Roboto-Regular.ttf` is Google's Roboto, version 2.137 (2017), "Copyright 2011
Google Inc. All Rights Reserved.", licensed under the Apache License, Version
2.0 (its own name table says so; the text is in `LICENSE-APACHE` beside it).
SHA-256 `319cff6e7a31f0f2a41c475dca42890aa5d19fe16017e2290f8c1d4e14f76481`.

It came from Mere at `f8c06097e`
(`crates/cambium/examples/genet_web_smoke/assets/Roboto-Regular.ttf`), where it
had no licence file beside it. `src/text/tests.rs` reads it to register a
generic font without system fonts. Engine split plan, P1, F5 (open): this
copy takes its first option, Inker carrying its own font with the licence.
