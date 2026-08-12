# Hot paths

## Enter

```sh
concord --json audit [TASK]
concord --json task show TASK
concord --json task dependency list TASK --direction both
```

Stop ordinary mutation on audit faults. Treat observations as evidence, not
authorization or blockers.

## Change and settle

Send version-1 JSON to stdin:

```sh
concord task change
concord phase settle
```

`task change` carries the Task revision and explicit fact edits. `phase settle`
carries one nonblank Outcome, optional entries, and explicit current-state
edits; it never infers carry-forward.

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
concord member prove TASK NAME --revision TASK_REV
concord member release TASK NAME --revision TASK_REV --apply
```

Prove a clean committed delta. Release only after delivery makes the proved
Member reachable or tree-equivalent and leaves it clean.

## Retain or finish

Preflight before Artifact import. Removal and Task finish require exact targets,
current revisions, a nonblank reason where requested, and explicit `--apply`.
