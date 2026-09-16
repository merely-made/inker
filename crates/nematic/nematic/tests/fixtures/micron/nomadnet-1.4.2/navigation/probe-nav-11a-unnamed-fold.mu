# Probe 11a: does an unnamed section line end a closed fold?
`!PROBE 11A TOP`!
`->Closed One A
MARKER A BODY: inside the depth-one fold.
>>>>
MARKER A AFTER UNNAMED: first line after a bare >>>> inside the fold.
MARKER A SECOND AFTER.
>Sentinel A
MARKER SENTINEL A.
`->>>>Closed Four B
MARKER B BODY: inside the depth-four fold.
>>>>
MARKER B AFTER UNNAMED: first line after a same-depth bare >>>>.
MARKER B SECOND AFTER.
>Sentinel B
MARKER SENTINEL B.
`->>Closed Two C
MARKER C BODY: inside the depth-two fold.
>
MARKER C AFTER UNNAMED: first line after a shallower bare >.
MARKER C SECOND AFTER.
>Sentinel C
MARKER SENTINEL C.
