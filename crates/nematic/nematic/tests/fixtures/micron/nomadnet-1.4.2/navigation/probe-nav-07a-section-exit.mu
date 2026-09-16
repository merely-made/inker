# Probe 07a: a leading < inside a nested section. Candidate spelling only.
`!PROBE 07A TOP`!
>Depth One
MARKER D1: body at depth one.
>>Depth Two
MARKER D2: body at depth two.
<
MARKER AFTER ONE LESS-THAN: line following a single leading <.
>>>Depth Three
MARKER D3: body at depth three.
<
<
MARKER AFTER TWO LESS-THAN: line following two leading < lines.
<<
MARKER AFTER DOUBLE LESS-THAN: line following one << line.
< trailing text on the same line
MARKER AFTER LESS-THAN WITH TEXT.
>Probe 07a end
End of probe 07a.
