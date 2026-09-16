# Probe 11c: unnamed section lines at equal and shallower depth inside closed folds.
`!PROBE 11C TOP`!
`->Closed One D
MARKER D BODY: inside the depth-one fold.
>
MARKER D AFTER UNNAMED: first line after an equal-depth bare >.
>Sentinel D
MARKER SENTINEL D.
`->>Closed Two E
MARKER E BODY: inside the depth-two fold.
>>
MARKER E AFTER UNNAMED: first line after an equal-depth bare >>.
>Sentinel E
MARKER SENTINEL E.
`->>>>Closed Four F
MARKER F BODY: inside the depth-four fold.
>>
MARKER F AFTER UNNAMED: first line after a shallower bare >>.
>Sentinel F
MARKER SENTINEL F.
