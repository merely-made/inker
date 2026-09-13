# Reference fixture: a Micron link has a leading backtick before [.
# The deliberately bare [Local page`:/page/target.mu] below is inert control text.

`[Local page`:/page/target.mu]
[Local page`:/page/target.mu]
`[Jump within this page`#local-anchor]
`:local-anchor
Anchor destination.

Name: `<name`seed>
Sized: `<16|sized_name`>
Notes: `<40x5|notes`>
Secret: `<!|secret`hidden text>
Check: `<?|flavour|mint`> Mint
Radio: `<^|level|high|*`> High
`[Submit all fields`:/page/submit.mu`*]
`[Submit selected fields`:/page/submit.mu`name|flavour|level|mode=check]
