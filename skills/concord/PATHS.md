# Hot paths

## Enter

```sh
concord --json task brief --domain DOMAIN
concord --json audit [TASK]
concord --json task show TASK
concord --json task dependency list TASK --direction both
```

Use the fixed-size Domain brief to select a Task from current Goal, Focus,
Question, and Next facts. Follow its exact `--after` cursor when another page
exists; full Task and Phase history stays opt-in.

Stop ordinary mutation on audit faults. Treat observations as evidence, not
authorization or blockers.

A refused coordinate answers itself. `concord.task.absent` carries the active
identities under `details.tasks` with `active` and `retired` counts, or the
managed `details.domains` when the domain itself is unknown;
`concord.task.ambiguous` carries the identities that collide and
`concord.domain.absent` the domains that exist. Read the refusal instead of
guessing a second name.

A recent-session warning is also observational and non-blocking. Restate its
operation and time plus any agent and session context without claiming
authorship or that the other session is still active, then follow the bounded
strategy in SCENARIOS.md.

## Change and settle

Send version-1 JSON to stdin:

```sh
concord task change
concord phase settle
```

`task change` carries the Task revision and explicit fact edits. `phase settle`
carries one nonblank Outcome, optional entries, and explicit current-state
edits; it never infers carry-forward. Both help surfaces print the exact
envelope, and a refused decode returns it under `details.envelope`.

Record work in flight as one ranked Addition per item, titled by name and
sourced by origin, and keep Focus to the one current state. A Phase closes only
when an Outcome is true, so a line that is waiting, blocked, or delivering in
parts accumulates Additions until then. Settle drains them: what closed becomes
Outcome and Evidence, what remains becomes Phase Carry, and the same envelope's
edits end the Additions it consumed.

## Coordinate

```sh
concord task dependency add SOURCE TARGET --weight sequence --revision GRAPH
concord graph adjacency
concord graph path SOURCE TARGET
concord graph cycles
```

Dependencies expose coordination and do not block lifecycle actions.

## Deliver

```sh
concord member attach TASK NAME --source PATH --claim PATH --revision TASK_REV
concord --json member status TASK NAME
concord member narrow TASK NAME --claim PATH --revision TASK_REV --apply
concord member prove TASK NAME --revision TASK_REV
concord member release TASK NAME --revision TASK_REV --apply
concord member retire TASK NAME --artifacts PATTERN --revision TASK_REV --apply
```

Status reads local worktree and integration-checkout health, Boundary currency,
integration relation, and local tracking refs. It never fetches and does not
claim its upstream or remote-tracking observations are current remote truth.

Prove a clean committed delta. Narrow replaces the claim with the given set
and invalidates any held Boundary; it refuses a set that `prove` would refuse.
Release only after delivery makes the proved Member reachable or tree-equivalent
and leaves it clean. Retire is the unlanded exit: same clean current-proof gates,
no landed check, and at least one Artifact name must match `--artifacts`
(`*` matches any name; quote it in the shell). Zero matches refuse.

## Retain or finish

Preflight before Artifact import. Removal and Task finish require exact targets,
current revisions, a nonblank reason where requested, and explicit `--apply`.
