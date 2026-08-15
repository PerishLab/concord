# Concord v0.12.7

## A refused coordinate answers itself

`concord.task.absent` was the single largest refusal an operator actually met,
and it said only that the name was not found. An agent that guessed a name had
no way to learn which names exist, so it guessed again: one session issued
twenty-two such refusals in three minutes, in same-second batches.

Four refusals now carry the coordinates the caller needed:

- `concord.task.absent` carries the active identities under `details.tasks`
  with `active` and `retired` counts, or the managed `details.domains` when the
  domain itself is unknown.
- `concord.task.ambiguous` carries the identities that collide.
- `concord.domain.absent` carries the domains that exist.

The list holds active identities only. Retired names still resolve, but sorting
them together buried every active name past the bound; the `retired` count
states what the list leaves out, and the gap between `active` and the list
length states where the bound cut.

Four sites had copied one refusal literally; they now name a single
`World::absent`, so the details reach all four at once.

## Tests no longer write into the operator's observation report

`cargo test` inherited `CONCORD_LOCUS_*` from the operator's shell, so every
suite run appended its fixtures to the live report file. A quarter of the
recorded invocations and forty-four percent of the recorded failures were
fabricated by the test suite, concentrated on exactly the refusals an operator
would study.

Tests now spawn the binary through one shared seat that clears every inherited
`CONCORD_` variable and redirects the report into the fixture. The redirect is
the load-bearing half: muting can be forgotten, but a redirected report cannot
reach the operator's file. A guard test refuses any test that spawns the binary
outside the seat.

The Plumb lock advances to 0.20.0.
