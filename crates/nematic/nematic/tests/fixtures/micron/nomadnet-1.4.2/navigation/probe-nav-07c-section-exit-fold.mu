# Probe 07c: does a leading < end a closed fold? Candidate spelling only.
`!PROBE 07C TOP`!
`->Closed One A
MARKER A BODY: inside the depth-one fold.
<
MARKER A AFTER <: left depth one inside a closed fold.
>Sentinel A
MARKER SENTINEL A.
`->Closed One B
MARKER B BODY: inside the depth-one fold.
>>Plain Two B
MARKER B DEPTH TWO BODY.
<
MARKER B AFTER <: left depth two inside a closed depth-one fold.
>Sentinel B
MARKER SENTINEL B.
