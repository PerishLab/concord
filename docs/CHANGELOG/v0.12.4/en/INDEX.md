# Concord v0.12.4

## The stdin envelope is published, and a refusal names itself

`task change` and `phase settle` read a JSON envelope from stdin and previously
described none of it. Their help listed flags only, and a decode refusal named
one missing field per call, so the only way to learn an envelope was to send it
repeatedly and read the refusals. Observed sessions did exactly that: one read
the help, learned nothing, and spent five calls collecting field names one at a
time.

Each envelope now owns one canonical shape beside its type. `task change
--help` and `phase settle --help` print it, and a `concord.input.json` refusal
carries the same shape under `details.envelope`. One call now answers what five
answered before. A test decodes both published shapes into their own types, so
a documented envelope that stops being valid input fails the guard rather than
drifting into prose.

The observation finish fact also names its refusal code as `fault`. Exit status
alone could not separate a rejected envelope from a stale revision, which left
an Atom report unable to explain its own failures without reading agent
transcripts.

The Plumb lock advances to 0.19.0.
