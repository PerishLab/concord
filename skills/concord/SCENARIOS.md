# Restrained scenarios

## Existing state needs migration

On `concord.estate.absent` or `concord.estate.upgrade_required`, follow the exact
[English v0.11.0 migration contract](https://git.perish.top/PerishFire/concord/src/tag/v0.11.0/docs/CHANGELOG/v0.11.0/en/MIGRATION.md)
or [Chinese contract](https://git.perish.top/PerishFire/concord/src/tag/v0.11.0/docs/CHANGELOG/v0.11.0/zh/MIGRATION.md).
Use the release-sealed script named there; do not invent compatibility steps.

## Related work is surprising

Read both adjacency directions, then use bounded neighbors, reach, path, cycles,
or strongly connected components. Add a Dependency when the relationship is
real even if no action should be blocked.

## Concurrent Members share a Repository

Keep branches, worktrees, and Claims distinct. Expand with `member claim` and
shrink with `member narrow`; either change invalidates the prior Boundary.
Narrow refuses a proposed claim that would fail `prove`. If overlap is real,
serialize delivery instead of weakening the Claim.

## Another session recently touched the Task

Restate the reported operation and time plus any agent and session context as
objective facts. The context does not establish who initiated the operation.
For read-only operations, normally continue because the warning is not a gate.
Before a write, re-read the Task revision and inspect relevant Members, Claims,
and local Git impact. Continue when the write surface is clearly disjoint. When
surfaces overlap or the impact remains unclear, report the facts and impact to
the caller before writing. Do not infer an owner, lease, heartbeat, takeover,
current presence, or permission from the warning.

## Retiring a Task

First release every landed Member, or retire an unlanded Member against a
matching Artifact, then remove every Artifact. `task finish` archives
incident dependency facts, cuts the edges, and reserves the Task name while
retaining current facts and Phases as lineage.
