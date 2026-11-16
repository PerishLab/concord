# Linux Member-preserving data-home relocation

This source-only Cargo example implements the bounded preparation in Concord
#466. It is separate from the installed CLI and the drained-Member `cutover`
example. Ordinary open has no migration or compatibility behavior.

## Qualified boundary

Use one exact legacy Space and one existing private target home on the same
Linux filesystem. Require a compatible Keel model, canonical absolute Member
`.git` pointers, registered Integrations at their retained paths/branches and
unchanged Member branches, heads and raw indices. No provider credentials,
provider writes, source commits, Issue retirement or automatic writer shutdown
are involved. Claims, Boundaries and change references remain in the copied
exact-model estate; old prepared delivery/landing plans must be discarded and
prepared again under the new Space identity and paths.

The whole `.issues` tree moves by rename. Dirty/staged/untracked/ignored files,
file modes and inode/mtime relationships survive without recreation. Regular
files/directories and qualified Member symlinks are supported. Relative links
must stay within the moved tree; parent traversal after a normal target segment
is refused. Absolute links into the old Issue/control tree, Artifact links,
nested filesystems, Member nested Git repositories/submodules, special files, orphan
seats, explicit `core.worktree`, relative Git pointers and active Git locks
refuse. Archived Git fixture directories inside Artifacts remain opaque regular
data: preserve their bytes without opening, executing or repairing their Git
metadata. A payload inventory over one million entries refuses. Windows/macOS and
cross-filesystem Member relocation have no qualification here. Process
interruption recovery is qualified; sudden host/filesystem loss is not. Keep using the
separately qualified drained tool for its supported matrix.

## Preparation and stopped-writer window

1. Deliver and freeze the exact source through Guard/Boundary/Concord. Build
   `cargo build --locked -p concord-core --example relocate` from that source
   through the normal identity/environment. Retain source/tree, binary digest
   and formal target Concord release/seal evidence outside both moved trees.
2. Rehearse old installed v0.63.0 -> helper -> formal installed v0.64.0 in an
   isolated fixture, including dirty Members, Artifact bytes, abort, interrupted
   activation/repair/publication, resumed finish and native final audit.
3. Inventory all retained Members, registrations, Git HEAD/index/state and
   unsupported payload; record fresh facts, not inferred inactivity. Preserve
   any existing unknown delivery outcomes for native reconciliation first.
4. Coordinate and actually stop every writer: all Concord clients, Member
   editors/builds, Santi strands and arbitrary Git users touching these paths.
   An empty occupancy report, absent lock, old mtime or idle process is no proof.
   Keep other sessions from opening either layout until verification completes.
5. Prepare a private current-config replacement removing only retired Space
   selectors (`space`/`domain_space_root`); preserve unrelated settings and
   explicit identities. Keep the installed default operator and old config
   unchanged until the activated target is verified with explicit new inputs.

`--stopped` is an operator attestation, not a detector or shutdown command. Each
invocation acquires old native control locks. This does not protect against
arbitrary file writers and does not replace step 4. Do not run the helper from
inside a Member that is about to move; use an independent frozen executable and
a working directory outside both payload trees.

## Commands and verification

Set `helper`, `legacy`, and `data` to the qualified absolute executable, old
Space and target home. All variables are operator inputs; the example paths for
this consumer are `/home/perish/Projects` and `/home/perish/.concord`.
Run each command separately and inspect its exit before proceeding:

```sh
"$helper" stage "$legacy" "$data" --stopped
"$helper" activate "$legacy" "$data" --stopped
```

Stage freezes legacy possession, captures controls/payload and Git facts, and
quarantines target possession under `data/state/relocate`; the incomplete target
cannot open normally. Activation renames the staged `workspace` into place,
then renames the Issue payload tree, repairs exact owned Git backlinks, verifies
content/index/head/branch and only then publishes target possession.

Before cleanup, use formal v0.64.0 with explicit new config/home for config
observation, Integration/Member lists and full audit. Compare stable Issue keys,
revisions, Claims, Boundary/change records, Member branch/head/index and payload
fingerprints to the frozen inventory. Relevant new faults refuse; pre-existing
stale Boundaries stay observations and do not authorize rewriting their proof.
Confirm native `git worktree list --porcelain` names the new Member paths and
source Integrations remain unchanged. Do not resume writers during this check.

```sh
"$helper" finish "$legacy" "$data" --stopped
```

Finish validates the activated identity/model, exact controls, unchanged payload
and repaired registrations before hash-checked deletion of old controls. It
keeps only a source/home/Space completion receipt and removes raw intent/ready
inventories. Retrying completed finish verifies the receipt/target identity and
model and old-path absence; it performs no payload cleanup or provider write.
After successful finish, atomically install the prepared config, select the
sealed stable operator and matching skill through their native managers, verify
the default entrypoint, then release the stopped-writer window. Refresh Member
proofs and delivery plans only when their owning task resumes.

## Recovery

| Observed boundary | Action |
| --- | --- |
| Stage complete, `data/workspace` absent | `abort` restores original authority and removes only owned staged controls. The Issue payload and Git metadata have not moved. |
| Stage refused after intent publication | Inspect exact refusal; use `abort` only while original fingerprints still agree. Unknown/changed files remain preserved. |
| Interrupted before valid intent exists | No source freeze is allowed before a complete intent. Preserve the partial private journal for investigation; no automatic partial-intent deletion is qualified. |
| `data/workspace` exists | Never abort, move payload back or restore old possession. Re-run `activate` after resolving the named external conflict. |
| Issue tree moved, some backlinks repaired | Re-run `activate`; exact old/new backlinks are accepted and only unfinished owned repairs run. Target possession remains quarantined. |
| Target possession published, result unknown | Re-run `activate`; verify every final backlink/payload/control instead of replaying repairs. |
| Finish interrupted during old cleanup | Re-run `finish`; only unchanged known remaining old controls are removed. Changed/unknown material refuses. |
| Completion receipt exists | Re-run `finish` to validate completion and settle interrupted journal cleanup. No second raw control archive is retained. |

No post-activation backward migration is promised. Do not hand-edit Keel rows,
Git backlinks or serialized plans to escape a refusal. Stop, retain evidence and
repair the exact external cause under the owning native workflow.
