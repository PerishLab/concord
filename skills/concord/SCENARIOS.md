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

## The deliverable is not clear enough to name

Report it and stop. Do not settle for a name drawn from the finding that opened
the line, from the tool being touched, or from the activity about to start; each
reads as a decision that was never made and survives long after the work moves
past it. Renaming later keeps the permanent identity but leaves every prose
reference in other Tasks and in frozen Phases pointing at a name that no longer
exists, which is the first class of drift this estate is audited for. The caller
owns the vocabulary of their own domain; an unclear shape is theirs to resolve,
not yours to guess.

## Retiring a Task

First release every landed Member, or retire an unlanded Member against a
matching Artifact, then remove every Artifact. `task finish` archives
incident dependency facts, cuts the edges, and reserves the Task name while
retaining current facts and Phases as lineage.
