# Migrating to Concord v0.5.2

Existing tasks, registries, worktrees, and task memory require no data
migration. The audit JSON keeps every field it had, in the same place.

A task's `status` no longer rises with filesystem or host-memory pressure, so
tasks that reported `WARN` for machine reasons now report the state of their
own footprints. Automation gating on `resources[].status` will see fewer
warnings and every remaining one names a footprint. Read
`resources[].filesystem` and `resources[].host_memory` directly to act on
machine capacity.

Within one audit, `resources[].host_memory` is now identical across tasks.
Automation that treated per-task readings as independent samples of the machine
should read one of them.
