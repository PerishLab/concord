# Concord v0.12.3

## Observation names the exact command and reaches the estate reads

The observation seat's start and finish facts carry the exact parsed command
instead of its top-level group. A `task show` cycle now reports `task.show`
rather than `task`, and `task change`, `task brief`, `task list`, and
`task start` are separable from each other for the first time.

One vocabulary now serves both surfaces. The parsed command names itself once,
and the private activity ledger reads that same name instead of its own copy.
Every ledger operation string is unchanged, so recorded activity and its
warnings are byte-identical to v0.12.2.

Seven kernel reads append independent function facts through Locus: estate
inspection, world load, current, facts, phases, graph, and worktrees. Between
its start fact and its first Git fact a full estate audit was previously
opaque; that region is now attributable from the record rather than by
inference. Observation remains muted unless `CONCORD_LOCUS_ENABLED=true` and a
report file are configured, and it still cannot change command output, exit
status, or control flow.

The Plumb lock advances to 0.18.27.
