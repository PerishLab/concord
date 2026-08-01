# Concord task protocol

## Territory and task model

A task is the authoritative unit of work: one home domain and task name, zero
or more repository worktree members, at most one task-level `.task/`, and zero
or more same-domain future task todos.

A member is one repository's task-scoped worktree under the task root and is
declared by that task's registry entry. Zero-member tasks are valid for non-Git
work and for retained memory or artifacts after repositories land.

A todo is an outgoing reference to another ordinary task in the same domain.
It records a future obligation without creating a backlog lifecycle, scheduler,
or issue mirror. The target may remain repo-less until execution pressure gives
it members, memory, issues, releases, or other resources.

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

A domain is a directory of sibling clean integration checkouts. Its private
`.tasks/tasks.toml` registry and `.tasks/<task>/` roots sit beside them.

`.tasks/`, task roots, and task memory are private local state, never repository
content. On POSIX, managed directories are `0700`; registry and Markdown memory
are `0600`. Private executable resource payload may be `0700`.

Registry version 3 is current. It retains the version 2 member write-claim
grammar defined in
[claims-v2.md](claims-v2.md). Read it before adding, claiming, proving,
migrating, or removing a repository member.

Rules:

- `task.name` names its root and is unique in a domain.
- `task.repo.name` names its member directory and is unique in the task.
- Both are nonempty single path components other than `.` or `..`.
- Relative sources resolve from `.tasks/`. A leading `~` means the current
  user's home. Cross-domain sources should be explicit.
- The branch defaults to the task name. Record `branch` only when different.
- Every version 2 or 3 member declares a normalized nonempty write claim.
- A version 3 task's `todo` list is sorted, unique, non-self, and names only
  tasks in the same registry. Version 1 and 2 tasks carry no todo state.
- Top-level `[[repo]]` entries are annotations, not task members.
- Mutable ownership attaches to claimed paths in one branch and worktree. A
  canonical Git common-directory identity may join several tasks concurrently
  only when every member has a distinct worktree, branch, and non-overlapping
  component-prefix claim. One mutable branch and one mutable path have at most
  one active owner.
- Task rename changes the task root and registry identity together. Preserve
  member branch names through overrides unless repository policy separately
  authorizes branch rename.

## Agreement and identity

For every declared member, ordinary mutation requires five surfaces to agree:

1. its registry entry;
2. its path and presence under the task root;
3. canonical identity of the source repository;
4. Git worktree metadata, including the expected branch;
5. its write claim and every active claim for the same canonical Git identity.

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

Members for one canonical repository may coexist when each task owns a distinct
branch, worktree seat, and disjoint declared path prefix. Semantic coordination
inside those claims and landing order remain branch concerns.

Start creates one coherent registry entry and private root, then the first
member if needed. Add `.task/` only under the memory rule below.

### Declare a future task

Use `concord task todo add <source> <target>` when a concrete follow-up exists
but does not yet need execution resources. The target may be an existing
same-domain task or a missing task name. A missing target is created repo-less
and linked in the same locked operation; failure to install the registry rolls
back its new root. Repeating the same add preserves the registry bytes.

The relation contains only the target task identity. Do not copy issue prose,
priority, assignment, status, or scheduling into it. Remove one relation with
`concord task todo remove ... --apply` only when the future obligation was
handed off, fulfilled elsewhere, or explicitly withdrawn.

## Resource health

Task entry runs one low-frequency resource observation through the existing
`concord audit` surface. Audit presents two independent planes:

- agreement faults describe a mismatch in the task protocol and fail the
  command;
- resource observations describe local headroom as `OK`, `WARN`, `CRIT`, or
  `UNKNOWN` and remain advisory.

Concord measures allocated bytes for the whole task, each member, `.task/`, and
each resource seat. It also observes filesystem and inode use plus host memory.
WARN/CRIT thresholds are 2/8 GiB for a task or member, 512 MiB/2 GiB for memory
or a resource seat, 60%/75% filesystem or inode use, and 40%/25% host memory
available.

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

Re-enter the task root, load current memory, and reapply five-surface agreement.
Block mutation on any mismatch until explicitly resolved.

### Add a repository

Use `concord member add` with the canonical integration checkout. Keep
cross-domain members under the existing home task root. The source checkout
must be clean. Resolve the mutable branch from the task name or recorded
override, submit the minimum `--write` prefixes, then add the member only when
its worktree path, branch, and claim are available. Expand an existing claim
through `concord member claim`; expansion is union-only, rechecks conflicts
under the global domain-space lock, and invalidates any prior boundary proof.

### Land a repository

1. Run the repository's own pre-land verification.
2. While the member HEAD and clean integration checkout still describe the
   delivery edge, run `concord member boundary <task> <member>`. Concord
   computes their merge-base and asks the stable Plumb library to prove every
   committed changed path lies within the member claim. The registry records
   the exact base, member HEAD, claim digest, Plumb proof schema, and resolved
   Plumb version.
3. Run the repository's landing process.
4. Before removing the member, verify every dirty or untracked file is durably
   preserved or intentionally discarded with explicit authorization.
5. Verify each relevant commit remains reachable through landing, or that the
   landed integration tree is equivalent.
6. Update the clean integration checkout through the repository's process.
7. Run `concord member preflight`. The persisted boundary proof must still
   match the current member HEAD, normalized claim digest, Plumb schema, and
   resolved Plumb version.
8. Remove only that landed member with
   `concord member remove-landed ... --apply`.

If unique state remains, stop and report it. Never silently move member payload
into `.task/resources/`. Branch deletion follows repository policy and is not
implied by task cleanup.

### Finish

Landing the final member leaves a valid repo-less task. Retain it while memory
or non-Git artifacts remain useful. If nothing remains, or exact deletion has
been authorized:

- A task still referenced by another task's todo cannot finish. Remove the
  incoming relations explicitly or finish their source tasks first.
- A source may finish with outgoing todos. Its finish plan prints every target
  as a handoff, removes the outgoing relations with the source, and leaves all
  targets in their normal task lifecycle.

1. remove exact resource seats through Concord;
2. remove `.task/` through `concord memory remove ... --apply`;
3. finish the now-empty task through `concord task finish ... --apply`.

Existing `.task/` and retained task artifacts follow exact-target consent.
Unsuperseded consent recorded as live state remains valid across sessions.
Landing authorization to discard member files and task-memory deletion consent
do not imply one another.

## Long-running memory

Create at most one `.task/` for complex work spanning rounds or phases. It may
contain `MAIN.md`, contiguous `phases/PHASE-NN.md`, and `resources/`.

`MAIN.md` is the concise current operating brief: goals, active constraints,
decisions still in force, current focus, open questions, and next step. It is
not a running log. Keep execution-driving detail concrete until resolved or
superseded.

Move settled milestones and rationale that no longer steer execution into the
next phase with `concord memory settle`. Keep still-active constraints in
`MAIN.md`, regardless of age. Use whole-file revision CAS for every write.
Concord owns a recognized versioned envelope and its fixed top-level section
boundaries; content inside those sections remains opaque Markdown.

New memory should use `concord-memory:v1`. Use `memory read --section` plus
`memory patch` for small current-state changes, and whole-file write or settle
when the envelope itself must change. Prefer `--file -` for generated content.
A regular-file input is consumed only after the complete mutation succeeds;
use `--keep-file` or `--keep-files` when retention is deliberate. A cleanup
failure reports that the mutation remains applied and never rolls it back.

MAIN is limited to 400 lines and 64 KiB; each PHASE is limited to 800 lines and
128 KiB. Retained phase names are contiguous and immutable. Schema and limit
violations are non-gating memory hygiene faults, while 16 retained phases emit
one non-failing advisory. Read
[memory-v1.md](memory-v1.md) before structured mutation or diagnosis.

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

Registry version 1 remains readable for audit, landing, and cleanup, but cannot
add or expand members. Its explicit all-member claim migration goes directly
to version 3 and is defined in [claims-v2.md](claims-v2.md). Version 2 migration
requires no claims and changes only the registry protocol version. Both use
`concord domain migrate ... --apply`; pre-v0.9.0 binaries reject version 3.

Renaming a version 3 task rewrites every same-registry incoming todo in the
same atomic registry replacement. Rehoming a task with incoming or outgoing
todo relations refuses until those links are removed or handed off; cross-domain
todo semantics are intentionally absent until demonstrated pressure requires
them.

Before destructive legacy cleanup, verify:

- registry and task filesystem state;
- Git worktree paths and canonical repository identity;
- branches and upstreams;
- local modifications and untracked files;
- integration-checkout cleanliness;
- reachability or durable preservation of every relevant commit and artifact.

Report exact sources, targets, and mismatches. Use exact validated paths.
Resolve merge conflicts only when confident; never force an uncertain result.
