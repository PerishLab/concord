# Migrating to Concord v0.10.0

This release replaces every legacy Domain registry and file-backed task memory
with one Space-wide Keel estate. Complete the migration as one explicit staged
transition. Do not run old and new Concord mutation concurrently.

## Preconditions

Use the latest v0.9.x binary first. Every Domain registry must already be
version 3, the whole Space audit must agree, integration checkouts must be
clean, and retained worktrees and Artifact payload must remain in place. Stop
all other Concord mutation before survey and keep a recoverable filesystem
backup or read-only clone of the Space.

The v0.10.0 migrator refuses disagreement, unmodelled Task territory, invalid
member claims, content or permission drift, dangling todo endpoints, self
dependencies, and cycles. It never repairs or guesses.

## Survey and stage

Run the new binary against the exact Space root:

```sh
concord --json migration survey
concord --json migration stage
```

`survey` records a bounded Census of registries, MAIN/PHASE files, permissions,
Artifact payload, and worktree agreement. Member contents remain live, are not
migrated, and therefore do not enter the Census byte fingerprint. `stage`
repeats the migratable evidence under the Space lock, bootstraps
`.concord/migration/v0.10.0/stage`, imports the complete estate, and verifies
scalar, relation, closure, filesystem, and permission equality. It does not
activate the estate or fence old binaries.

Retain the returned `census.fingerprint`. A later session may only reopen the
same staged state when the live legacy evidence still matches exactly:

```sh
concord --json migration resume FINGERPRINT
```

Any fingerprint change requires investigation. Do not delete or rewrite the
stage to force agreement.

## Activation

Activation is a separate destructive authority. After reviewing the staged
Census and ensuring every old Concord process is stopped, run:

```sh
concord --json migration activate FINGERPRINT --apply
```

Activation re-audits worktree agreement and re-hashes migratable legacy
evidence under the Space lock. It first replaces every known registry with an
unsupported version-4 marker so an old binary cannot mutate the Space. It then
archives exact registries and
MAIN/PHASE files beneath `.concord/migration/v0.10.0/rollback`, renames each
`.task/resources` directory to `.task/artifacts` on the same filesystem,
prunes newly empty repo-less roots, and installs the staged database and sudo
possession as `.concord/estate.sqlite3` and `.concord/sudo`.

The activation journal rolls back an incomplete activation. After a successful
activation there is deliberately no ordinary rollback command and no usable
legacy authority. Preserve the rollback manifest and evidence; their deletion
requires separate exact-target authorization.

## Imported meaning

- Every legacy Domain, Repository, Task, Member, Claim, and Boundary keeps its
  exact proven meaning. Each current Task coordinate receives a lifetime
  Reservation.
- A Boundary proved by an older Plumb remains exact and appears as the
  non-gating `boundary.stale` observation after activation. Prove it again
  before Member release; version staleness does not refuse migration.
- Every todo becomes one direct `unknown / legacy-todo` dependency with the
  same endpoints. Closure is rebuilt by Keel and any cycle refuses the stage.
- Structured MAIN roles become current facts. Collection Markdown stays one
  intact ordered body; bullets are not split into invented Resources.
- Every PHASE becomes one frozen aggregate. A legacy Phase may remain without
  an Outcome and appears as the non-gating `phase.missing_outcome` observation.
- Legacy or unknown TOML/text content remains an attributed Addition. Blank
  sections create no invented text.
- Artifact bytes and Git worktrees never enter SQLite and are never bulk
  copied.

## New operating path

Read exact command grammar with `concord <command> --help`. The principal
replacements are:

```text
todo add/set/remove       -> task dependency add/set/remove
memory read/patch         -> task show / task change
memory settle/phase       -> phase settle / phase list
resource import/remove    -> artifact import/remove
member add/boundary       -> member attach/prove
member remove-landed      -> member release --apply
domain init               -> domain bootstrap (fresh Space only)
```

Task changes and settle consume version-1 JSON from stdin by default and carry
the expected Task revision. Dependency writes carry the expected Space graph
revision. Destructive removal, release, finish, and activation require
`--apply`.

After activation, run `concord --json audit` and inspect both `faults` and
`observations`. Zero faults proves ordinary mutation agreement. Unknown or
cross-domain dependencies and missing Goal/Focus/Next facts expose work to
organize; they do not authorize cleanup or block execution.
