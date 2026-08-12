# Concord

Concord is the executable control plane for private task estates. One Keel
SQLite estate owns one configured Space; Git worktrees and direct Artifact
directories remain external payload.

```toml
domain_space_root = "/srv/projects"
home = "/home/operator/.concord"
releases = "https://releases.concord.perish.uk"
```

The default config path is `~/.concord/concord.toml` on Unix and
`%LOCALAPPDATA%\concord\concord.toml` on Windows. Global `--config` selects one
explicit file. `CONCORD_DOMAIN_SPACE_ROOT` or `--root` supplies the Space;
Concord never assumes `~/Projects`.

## Estate

Keel bootstrap is the only database genesis mechanism. Start a fresh Space and
add later Domains explicitly:

```text
concord domain bootstrap perish.code
concord domain add another.domain
concord domain list
```

Ordinary open replays the exact sealed model using the retained private sudo
possession. It never creates, binds, or evolves an occupied estate implicitly.
Legacy Spaces must follow the exact bilingual
[v0.10.0 migration contract](docs/CHANGELOG/v0.10.0/en/MIGRATION.md).

Domain, typed Repository annotation, permanent Task identity, current Task
facts, frozen Phases, Member/Claim/Boundary state, lifetime name Reservations,
and direct dependency closure live in the estate. Repo-less Task start creates
no filesystem root.

## Tasks and Phases

Task state is read with its revision and changed through a versioned JSON
envelope. Generated changes should use stdin:

```text
concord task start perish.code ship-feature
concord --json task show perish.code/ship-feature
concord task change
concord phase settle
concord phase list perish.code/ship-feature
```

Current roles are Goal, Constraint, Decision, Focus, Question, Next, and
Addition. Goal, Focus, and Next are singular. A change-set explicitly creates,
sets, ends, or reorders facts under Task revision CAS. Settle atomically creates
one immutable Phase and applies only its submitted current-state delta; it
never snapshots or infers carry-forward. Every new Phase requires exactly one
nonblank Outcome.

Rename and rehome keep the permanent Task key, all facts and Phases, and every
graph endpoint. Derived Member and Artifact seats move with the Task and Git
worktree registration is repaired. All occupied coordinates remain
lifetime-reserved.

## Dependency graph

An edge means `source depends_on target`. Weights are ordered
`unknown < context < sequence < required`. The graph exists first to expose
coordination problems: weight never schedules work or blocks member operations,
settle, rename, rehome, finish, or release. Self-edges and cycles refuse.

```text
concord task dependency add SOURCE TARGET --weight sequence --revision GRAPH
concord task dependency set SOURCE TARGET --weight required --revision GRAPH
concord task dependency remove SOURCE TARGET --revision GRAPH --reason TEXT --apply
concord task dependency list TASK --direction both
concord graph adjacency
concord graph neighbors TASK --direction out --depth 2
concord graph degree TASK
concord graph reach TASK --direction out --min-weight context
concord graph path SOURCE TARGET
concord graph cycles
concord graph scc
concord graph export
```

Cross-Domain dependencies are valid and exposed by audit. `--create-target`
atomically creates an explicitly qualified missing target; it is never the
default.

## Members and Artifacts

Member, Claim, and Boundary facts live in Keel while Git owns the worktree.
Claims are normalized nonempty repository-relative prefixes. Claim expansion
is union-only and invalidates the previous proof.

```text
concord member attach TASK NAME --source PATH --claim PATH --revision TASK_REV
concord member claim TASK NAME --claim PATH --revision TASK_REV
concord member prove TASK NAME --revision TASK_REV
concord member release TASK NAME --revision TASK_REV --apply
```

Release requires a current Plumb Boundary proof, a clean Member, and landed
reachability or exact tree equivalence.

Artifact is a named private directory at the derived
`.task/artifacts/<name>/` seat. It has no Keel row and does not participate in
Task revision CAS.

```text
concord artifact list TASK
concord artifact preflight TASK NAME --source PATH
concord artifact import TASK NAME --source PATH
concord artifact remove TASK NAME --apply
```

Import preflight measures the full source, refuses links and special files,
preserves conservative filesystem and inode headroom, and streams the private
copy.

## Audit and lifecycle

`concord --json audit` checks estate rows, name Reservations, current and Phase
shape, direct graph and materialized closure, retired endpoints, Member/Claim/
Boundary agreement, private custody, and foreign derived-seat territory. Every
ordinary mutation runs this agreement gate first.

Unknown and cross-Domain dependencies, missing Goal/Focus/Next, and migrated
legacy Phases without Outcome are observations. They expose problems but never
authorize cleanup or make ordinary work impossible.

Finish requires zero live Members and no retained Artifacts. It archives every
incident dependency, cuts those edges, marks the permanent Task retired, and
bumps Task and graph revisions. Current facts and frozen Phases remain lineage.

```text
concord task finish TASK --revision TASK_REV --graph GRAPH_REV --reason TEXT --apply
```

## Observation and skill

Optional Locus process-cycle observation remains environment-only and muted by
default. Enabling it requires `CONCORD_LOCUS_ENABLED=true` and
`CONCORD_LOCUS_REPORT_FILE`; Context never affects business output or control
flow.

Concord ships a release-matched managed skill:

```text
concord skill install
concord skill status
concord skill upgrade
concord skill uninstall
```

`concord <command> --help` is the exhaustive CLI grammar. The canonical release
manager is available at `https://releases.concord.perish.uk/manage.sh`.
