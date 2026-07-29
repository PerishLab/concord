# Concord v0.5.0

## Resource health joins task audit

`concord audit` now reports protocol agreement and local resource health as
separate data planes. Every task audit observes allocated task, member, memory,
and resource-seat footprint, together with filesystem capacity, inode capacity
where available, and host available memory and swap.

Resource observations use conservative `OK`, `WARN`, `CRIT`, and `UNKNOWN`
states. They remain advisory: resource pressure does not become a protocol
fault and cannot prevent diagnosis, landing, or cleanup. Footprint-expanding
operations use their own current-headroom preflight instead.

Resource imports now inspect the complete source tree, preserve filesystem and
inode reserve, recheck under the task lock, reject links and special files
before allocation, and stream file contents into private seats. Member creation
refuses when its target filesystem is already critical.

## Landing proofs are visible

`concord member preflight` now emits one evidence record per declared member.
It exposes canonical repository identity, expected and actual branches, member
and integration heads and trees, and whether removal was proved by commit
reachability or exact tree equivalence.

## Safer and more composable operations

- `memory write` and `memory settle` accept one explicit stdin payload without
  ambiguously framing two inputs.
- One canonical repository can back concurrent task members when every member
  has a distinct mutable branch and worktree seat. Concord treats the branch,
  not the repository as a whole, as the unit of mutable ownership.
- Member creation refuses an already existing target branch during both dry
  run and apply, before changing the registry or worktree set.
- The installer removes superseded managed versions after activating and
  verifying the selected version. `--retain` preserves existing versions when
  an offline local rollback seat is deliberately wanted.

The release-matched Concord skill documents the new audit findings, operation
gates, and resource-import guarantees.
