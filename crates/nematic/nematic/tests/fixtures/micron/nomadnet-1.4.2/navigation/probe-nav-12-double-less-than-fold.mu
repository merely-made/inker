# Probe 12: << as the first line inside a closed fold.
`!PROBE 12 TOP`!
`->Closed Twelve A
<<
MARKER 12A AFTER: first line after << inside the fold.
MARKER 12A SECOND AFTER.
>Sentinel 12A
MARKER SENTINEL 12A.
`->Closed Twelve B
<
MARKER 12B AFTER: first line after < inside the fold.
MARKER 12B SECOND AFTER.
>Sentinel 12B
MARKER SENTINEL 12B.
`->Closed Twelve C
MARKER 12C BODY: inside the fold.
<<
MARKER 12C AFTER: first line after << that follows a body line.
MARKER 12C SECOND AFTER.
>Sentinel 12C
MARKER SENTINEL 12C.
