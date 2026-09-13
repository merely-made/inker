# Black-box probe. The documented literal delimiter is a line consisting of `=.
Before literal mode.
`=
>This must remain visible text if literal mode is active.
`[This must remain visible text if literal mode is active`:/page/not-a-link]
`!This must remain visibly unstyled if literal mode is active`!
`=
After literal mode.

# Candidate only: test whether a backslash prevents the next backtick from opening a tag.
Candidate escape: \`!not-bold\`!
# Candidate only: an inline `= is not documented as a delimiter.
Candidate inline delimiter: before `= after
