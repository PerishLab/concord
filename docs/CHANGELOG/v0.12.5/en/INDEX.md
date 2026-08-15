# Concord v0.12.5

## The fact roles are named, and Addition is the ledger for work in flight

The shipped skill never named a single fact role. An agent learned the
vocabulary by imitating existing Tasks, and Addition appeared nowhere at all, so
a Task could be worked for months without its operator knowing the role existed.

The skill now names every role and its cardinality, and states what Addition is
for: the running ledger of work in flight, one titled and sourced entry per
item. A Phase closes only when an Outcome is true, so a line that is waiting,
blocked, or delivering in parts has nowhere to settle and accumulates Additions
until it does. Settling drains them — what closed becomes Outcome and Evidence,
what remains becomes Phase Carry, and the same envelope's edits end the
Additions it consumed. Focus stays the one current state.

Nothing about the estate changed. Addition already carried rank, title, and
origin; only the writing did not say so.

## Machine output drops its indentation

`--json` now prints one line. The encoded value is identical and the human
renderer was always a separate path, so a bounded Domain projection loses about
a third of its bytes without losing a field.

## A retired Task no longer pays for a Phase observation

Retired lineage cannot grow a legacy Phase, yet every mutation walked the Phases
of every retired Task before it was allowed to proceed. A retired Task keeps its
readability fault, which is the check that can refuse a write, and loses only
the migrated-Phase observation. A full estate audit settles near four fifths of
its former time.
