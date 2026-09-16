# Probe 06a: collapsible headings at equal depth, a control for 06b.
`!PROBE 06A TOP`!
`->Outer Closed
MARKER OUTER: body directly under the closed outer heading.
`+>Inner Authored Open
MARKER INNER OPEN: body under the inner heading authored open.
`->Inner Authored Closed
MARKER INNER CLOSED: body under the inner heading authored closed.
>Sentinel After
MARKER SENTINEL: always visible, ends the outer fold.

`+>Outer Open
MARKER OUTER OPEN: body under an outer heading authored open.
`->Nested Under Open
MARKER NESTED UNDER OPEN: body under a closed heading inside an open one.
>Probe 06a end
End of probe 06a.
