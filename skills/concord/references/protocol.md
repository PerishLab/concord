# Concord task protocol

## Contents

- [Territory and task model](#territory-and-task-model)
- [Domain and registry](#domain-and-registry)
- [Agreement and identity](#agreement-and-identity)
- [Entering and starting](#entering-and-starting)
- [Resource health](#resource-health)
- [Lifecycle](#lifecycle)
- [Long-running memory](#long-running-memory)
- [Recovery and migration](#recovery-and-migration)

## Territory and task model

A task is the authoritative unit of work:

```text
task(home domain, task name)
  -> {0..N repository worktree members, 0..1 task-level .task/}
```

A member is one repository's task-scoped worktree under the task root and is
declared by that task's registry entry. Zero-member tasks are valid for non-Git
work and for retained memory or artifacts after repositories land.

Treat a repository or tree as foreign when the user says so, its governing
instructions conflict with this protocol, or its metadata shows another system
owns it. Uncertainty blocks adoption or migration. Do not create Concord state
there merely to make it conform.

The task home domain is fixed at Start. Use the containing managed domain when
one exists; otherwise use a member repository's known managed domain. If
several introduced domains are equally plausible, ask once. Cross-domain
members remain under the one home task root and name their source checkout
explicitly. Never relocate an integration checkout for co-location.

## Domain and registry

A domain is a directory of sibling clean integration checkouts:

```text
<domain-space-root>/
└── <domain>/
    ├── repo-a/
    ├── repo-b/
    └── .tasks/
        ├── tasks.toml
        └── <task>/
            ├── .task/
            ├── repo-a/
            └── repo-b/
```

`.tasks/`, task roots, and task memory are private local state, never repository
content. On POSIX, managed directories are `0700`; registry and Markdown memory
are `0600`. Private executable resource payload may be `0700`.

Registry version 1 is:

```toml
version = 1

[[task]]
name = "ship-feature"

[[task.repo]]
name = "repo-a"
source = "../repo-a"

[[task.repo]]
name = "repo-b"
source = "~/Projects/another-domain/repo-b"
branch = "legacy/slashed-branch"

[[repo]]
name = "repo-a"
note = "optional annotation"
```

Rules:

- `task.name` names its root and is unique in a domain.
- `task.repo.name` names its member directory and is unique in the task.
- Both are nonempty single path components other than `.` or `..`.
- Relative sources resolve from `.tasks/`. A leading `~` means the current
  user's home. Cross-domain sources should be explicit.
- The branch defaults to the task name. Record `branch` only when different.
- Top-level `[[repo]]` entries are annotations, not task members.
- A canonical repository may join several tasks only through distinct
  worktrees and distinct branches. One mutable branch has at most one owner.
- Task rename changes the task root and registry identity together. Preserve
  member branch names through overrides unless repository policy separately
  authorizes branch rename.

## Agreement and identity

For every declared member, ordinary mutation requires four surfaces to agree:

1. its registry entry;
2. its path and presence under the task root;
3. canonical identity of the source repository;
4. Git worktree metadata, including the expected branch.

A missing declared member or undeclared Git worktree under the root is a
protocol violation. `.task/` and intentional non-repository artifacts are not
members. Audit a cross-domain member through its owning task only; never
register it again in the source domain.

On disagreement, report the exact mismatch and block ordinary mutation.
Read-only diagnosis and explicitly authorized recovery remain allowed. Never
silently infer, manufacture, or repair registry state. Before a registry write,
lock and verify the bytes still match the snapshot being replaced.

An integration checkout is the repository's clean landed-state mirror, updated
through that repository's normal landing process. Mirror does not mean
continuously synchronized and never authorizes task work in it. Repository
instructions must establish integration and landing conventions; ask rather
than infer missing conventions.

## Entering and starting

Resolve `domain/task` directly. For an unqualified name, check the containing
domain first, then search known registries read-only. Enter a unique match and
ask if several match. Lookup never creates or repairs.

At the task root:

1. read `.task/MAIN.md` first through `concord memory read` when present;
2. run `concord audit`;
3. inspect the relevant member;
4. enter that repository and discover all applicable repository instructions.

A session starting inside a member must locate its owning task root and load
the sibling task memory. Starting at the task root is outside every repository,
so repository-local instructions are not automatically in scope.

If no task exists, create persistent task state only when work needs a branch
or worktree, durable task artifacts, or durable multi-round memory. Pure Q&A,
read-only work without retained artifacts, and transient scratch work need no
task. Parallel lines touching one repository use separate tasks, branches, and
worktrees.

Start creates one coherent registry entry and private root, then the first
member if needed. Add `.task/` only under the memory rule below.

## Resource health

Task entry runs one low-frequency resource observation through the existing
`concord audit` surface. Audit presents two independent planes:

- agreement faults describe a mismatch in the task protocol and fail the
  command;
- resource observations describe local headroom as `OK`, `WARN`, `CRIT`, or
  `UNKNOWN` and remain advisory.

Concord measures allocated bytes for the whole task, each member, `.task/`, and
each resource seat. It also observes the task filesystem, inode capacity where
the platform exposes it, and current host available memory and swap. Thresholds
are deliberately conservative:

| Observation | WARN | CRIT |
| --- | ---: | ---: |
| task or member footprint | 2 GiB | 8 GiB |
| memory or resource seat | 512 MiB | 2 GiB |
| filesystem or inode use | 60% | 75% |
| host memory available | 40% | 25% |

Resource status never blocks read-only diagnosis, audit, landing, or cleanup.
Only an operation that expands the managed footprint may refuse on current
headroom. Resource import inspects the entire source before creating its seat,
preserves 25% filesystem and inode capacity with a minimum 1 GiB byte reserve,
rechecks under the task lock, and streams private copies. Member creation
refuses when its target filesystem is already at the critical threshold.

There is no resource history database, daemon, timer, process ownership
inference, or automatic cleanup. An unavailable observation is explicit
`UNKNOWN`; it is never guessed from another metric.

## Lifecycle

### Resume

Re-enter the task root, load current memory, and reapply four-surface agreement.
Block mutation on any mismatch until explicitly resolved.

### Add a repository

Use `concord member add` with the canonical integration checkout. Keep
cross-domain members under the existing home task root. The source checkout
must be clean.

### Land a repository

1. Run the repository's own pre-land verification and landing process.
2. Before removing the member, verify every dirty or untracked file is durably
   preserved or intentionally discarded with explicit authorization.
3. Verify each relevant commit remains reachable through landing, or that the
   landed integration tree is equivalent.
4. Update the clean integration checkout through the repository's process.
5. Run `concord member preflight`.
6. Remove only that landed member with
   `concord member remove-landed ... --apply`.

If unique state remains, stop and report it. Never silently move member payload
into `.task/resources/`. Branch deletion follows repository policy and is not
implied by task cleanup.

### Finish

Landing the final member leaves a valid repo-less task. Retain it while memory
or non-Git artifacts remain useful. If nothing remains, or exact deletion has
been authorized:

1. remove exact resource seats through Concord;
2. remove `.task/` through `concord memory remove ... --apply`;
3. finish the now-empty task through `concord task finish ... --apply`.

Existing `.task/` and retained task artifacts follow exact-target consent.
Unsuperseded consent recorded as live state remains valid across sessions.
Landing authorization to discard member files and task-memory deletion consent
do not imply one another.

## Long-running memory

Create `.task/` only for complex work spanning rounds or phases. There is at
most one, at the task root:

```text
.task/
├── MAIN.md
├── phases/
│   ├── PHASE-00.md
│   └── PHASE-01.md
└── resources/
```

`MAIN.md` is the concise current operating brief: goals, active constraints,
decisions still in force, current focus, open questions, and next step. It is
not a running log. Keep execution-driving detail concrete until resolved or
superseded.

Move settled milestones and rationale that no longer steer execution into the
next phase with `concord memory settle`. Keep still-active constraints in
`MAIN.md`, regardless of age. Use whole-file revision CAS for every write.

`resources/` holds opaque support material such as raw outputs or fetched
documentation. Initialize memory before allocating resources. Import through
Concord so links and special files are refused, headroom is proved, privacy
modes are preserved, and file contents are streamed rather than held whole in
memory. Do not copy secrets unless recovery genuinely requires them.

Default toward deleting memory at task completion, but only with exact consent.

## Recovery and migration

Rehoming, renaming, adopting legacy state, and repairing mismatched state are
explicit migrations requiring authorization. Never migrate merely because a
layout differs.

Before destructive legacy cleanup, verify:

- registry and task filesystem state;
- Git worktree paths and canonical repository identity;
- branches and upstreams;
- local modifications and untracked files;
- integration-checkout cleanliness;
- reachability or durable preservation of every relevant commit and artifact.

Report exact sources, targets, and mismatches. Use exact validated paths.
Resolve merge conflicts only when confident; never force an uncertain result.
