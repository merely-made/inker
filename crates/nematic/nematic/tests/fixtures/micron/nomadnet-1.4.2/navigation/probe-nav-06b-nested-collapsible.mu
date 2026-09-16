# Probe 06b: a closed section containing depth-two collapsible headings.
`!PROBE 06B TOP`!
`->Outer Closed
MARKER OUTER: body directly under the closed outer heading.
`+>>Inner Authored Open
MARKER INNER OPEN: body under the depth-two heading authored open.
`->>Inner Authored Closed
MARKER INNER CLOSED: body under the depth-two heading authored closed.
>Sentinel After
MARKER SENTINEL: always visible, ends the outer fold.
