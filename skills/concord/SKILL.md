---
name: concord
description: Operate the private Concord `.tasks` + `.task` control plane, including task todo links and stdin-first structured task memory. Use when starting, locating, resuming, auditing, renaming, rehoming, landing, retaining, or finishing managed tasks; declaring future task work; adding or removing repository worktree members; reading, patching, or settling task memory; managing task resources or permissions; or replacing any proposed raw mutation of `.tasks/`, `.task/`, task worktrees, or `tasks.toml`.
---

# Concord

Concord is the executable control plane for private task state. Treat the
installed task policy, repository instructions, recorded filesystem and Git
state, and current authorization as truth; authorship or agent memory is never
truth.

## Upstream

Repository: https://git.perish.top/PerishFire/concord

Report defects, missing shapes, and unclear guidance there as issues. Use
`concord skill status` to check stable without changing an installation. When
a newer stable release is available, run `concord skill upgrade` and validate
it before preserving compatibility with an older managed installation.

Before any stateful task operation, read
[references/protocol.md](references/protocol.md) completely. For pure command
discovery, prefer `concord <command> --help`.

Before adding, claiming, proving, migrating, or removing a repository member,
also read [references/claims-v2.md](references/claims-v2.md) completely.

Before initializing, projecting, patching, settling, or diagnosing structured
memory, also read
[references/memory-v1.md](references/memory-v1.md) completely. Do not load that
grammar for an ordinary task entry or raw memory read.

## Principles

**Protocol-grounded truth.** Determine validity and transitions from observable
protocol state. Never infer a missing registry entry, member, branch, or
authorization.

**Composable kernel.** Express work through task, member, memory, resource,
permission, and audit primitives. Do not invent scenario-specific state.

**Refusal over repair.** Audit first. On disagreement, diagnose read-only and
stop ordinary mutation until recovery is explicitly authorized.

**Privacy by construction.** Keep `.tasks/`, task roots, `.task/`, and retained
artifacts outside repositories and private. Never copy them into product Git
history.

**Observation is one-way.** Optional Locus collection may bind an exact
`CODEX_THREAD_ID` to the current trace, but Context and report outcomes never
enter protocol state, authorization, command output, or control flow. An
explicit trace ID overrides collection; a muted gate performs no observation
work.

**The right to amend travels with the layer.** Concord owns task protocol law.
Repository shape and release law belong to Plumb; load the `plumb` skill when
building or operating a repository rather than restating its clauses here.

## Operating workflow

1. Resolve the domain-space root from config, `CONCORD_DOMAIN_SPACE_ROOT`, or
   `--root`. Never assume `~/Projects`.
2. Resolve the requested task. Enter its task root, read `.task/MAIN.md` through
   `concord memory read` when present, and run `concord audit`. Treat agreement
   faults as protocol blockers and resource `WARN`, `CRIT`, or `UNKNOWN`
   findings as early operational evidence, not as disagreement.
3. Before acting in a member repository, enter that member and discover every
   applicable repository instruction file. Run the repository's normal Plumb
   doctor before changing its shape.
4. Use Concord for every managed structure or lifecycle mutation. Git owns
   member payload and repository landing; Concord owns member seats and
   agreement.
5. Re-audit after structural mutation. Keep `MAIN.md` current when the task is
   long-running; settle completed history into the next phase. Prefer stdin for
   generated memory mutation input. Explicit files are consumed after success
   unless `--keep-file` or `--keep-files` is intentional.
6. Before landing, record `concord member boundary` for the clean committed
   member. Land through the repository's own process, verify reachability,
   cleanliness, and the still-current proof, then remove only the landed member
   seat. Before task finish, inspect outgoing todos and make their handoff
   explicit. Finish a repo-less task only after retained state is absent or its
   exact deletion is authorized.

When durable memory is first needed, initialize the v1 envelope from
`references/memory-v1.md` through `--file -` by default.

## Laws

- One task has one fixed home domain, zero or more repository members, and at
  most one task-level `.task/`. A repo-less task is valid.
- A task todo is a same-domain link to another real task, not a scheduler,
  issue, or backlog state. Todo links are sorted, unique, and never self-links.
- Todo add creates an absent repo-less target and records the link atomically.
  Todo removal is explicit; finishing a source reports handoffs and leaves each
  target alive, while incoming links prevent target finish.
- Mutable ownership attaches to declared path prefixes in one branch and
  worktree. One canonical Git identity may back several active task members
  only when their worktree paths, branches, and component-prefix claims are
  distinct.
- Ordinary mutation requires agreement among the registry entry, member path,
  canonical source identity, and Git worktree metadata.
- Integration checkouts are clean landed-state mirrors. Never branch, commit,
  or hold task work in them.
- Task and member names are single path components. Branches are Git refs and
  may differ from the task name only through a recorded override.
- Creation runs unless `--dry-run`. Deletion, migration, landed-seat removal,
  recovery, and permission normalization require their explicit execution
  flag.
- Registry and memory writes lock, re-read, compare, and atomically replace.
  Memory writes require the expected whole-file revision.
- Member claims are normalized, nonempty, UTF-8 repo-relative prefixes. `.` is
  the only whole-repository spelling. Claim expansion is union-only and clears
  a prior boundary proof.
- Versioned memory envelopes and fixed top-level section boundaries belong to
  Concord. Section bodies remain opaque Markdown. Sparse patches carry the
  expected whole-file revision and preserve untouched source bytes.
- `.task/` exists only for durable multi-round memory or artifacts. `MAIN.md`
  carries live execution state; settled rationale moves into numbered phases.
- Existing memory and retained task artifacts require exact-target deletion
  consent. Landing permission to discard member files is separate.
- Resource health is a low-frequency task-entry audit surface. It has no
  history database, does not attribute arbitrary processes, and never
  authorizes automatic cleanup.
- Read, audit, landing, and cleanup remain available under resource pressure.
  Operations that expand the managed footprint may refuse when their current
  preflight cannot preserve conservative headroom.
- Renaming, rehoming, legacy adoption, and mismatch repair are explicit
  migrations. Rename rewrites incoming todo links atomically; a task with any
  todo link cannot rehome. Never perform one merely because a layout differs.

Read [references/protocol.md](references/protocol.md) for registry grammar,
entry resolution, lifecycle preconditions, memory layout, foreign-territory
handling, and recovery safety.

## Standing

Concord currently enforces:

- task/domain/member component syntax and registry schema validation;
- task-root, declared-member, canonical-source, branch, and Git worktree
  agreement, which alone gates ordinary mutation;
- private task, memory, and resource permissions, reported by audit as hygiene
  of Concord's own storage and repaired by `permissions normalize`, but never
  gating the mutation that would clear them;
- clean source checkout and an absent target branch before member creation;
- explicit write claims on version 2 and 3 members, with cross-task overlap refusal
  by canonical Git common-directory identity under the domain-space lock;
- explicit all-member migration from registry version 1 to current version 3,
  claim-free migration from version 2 to version 3, and retained version 1
  audit, landing, and cleanup;
- version 3 same-domain task todo links, including atomic target creation,
  idempotent add, explicit unlink, rename rewrite, guarded rehome and target
  finish, and source-finish handoff reporting;
- Plumb-backed committed-delta boundary proofs bound to exact base, member
  HEAD, normalized claim digest, proof schema, and resolved Plumb version;
- clean and reachable or tree-equivalent member state before landed removal;
- a still-current boundary proof before version 2 or 3 landed removal;
- locking, registry compare-before-replace, and memory revision CAS;
- bounded stdin or regular-file memory input, default post-success file
  consumption, explicit retention, and typed cleanup-after-apply errors;
- v1 memory schema, raw-byte sparse patching, no-op detection, fixed MAIN and
  PHASE limits, immutable phase listing and reading, and raw-read ceiling;
- memory schema and limit hygiene in audit, plus a non-failing phase-count
  advisory after 16 retained phases;
- explicit plans, default-on creation, and execution gates for destructive or
  migratory commands;
- task finish only when no members or retained filesystem state remain;
- task, member, memory, and resource-seat allocated-footprint observations in
  every task audit;
- filesystem capacity, inode capacity where available, and host
  available-memory observations with explicit `UNKNOWN`;
- resource-import whole-tree size and headroom preflight, streaming private
  copy, plus refusal of links and special files;
- default-muted process-cycle observation, exact `CODEX_THREAD_ID` trace
  collection, explicit trace override, command start/finish facts, and
  unchanged business output when observation is enabled;
- refusal to add a member when the target filesystem is already critical.

The following remain agent-held law; no machine will stop every violation:

- recognizing foreign territory and declining adoption;
- choosing the correct home domain and repository integration/landing
  conventions;
- coordinating semantics and landing order among disjoint concurrent claims;
- discovering repository-local instructions before acting;
- deciding whether work needs a task or durable memory;
- proving dirty or untracked member payload is durably preserved before land;
- interpreting authorization, especially exact-target cleanup consent;
- deciding whether retained history still steers execution;
- refusing a conflict that cannot be resolved confidently.

Do not claim a prose-only clause is checked. When Concord mechanizes one, move
its standing here in the same release as the check.

## Invocation

```bash
concord config path
concord config show
concord domain list
concord task start <domain>/<task>
concord task show <domain>/<task>
concord task todo add <domain>/<task> <target>
concord task todo remove <domain>/<task> <target> --apply
concord member add <domain>/<task> --source <integration-checkout> --write <path>
concord member claim <domain>/<task> <member> --write <path>
concord member boundary <domain>/<task> <member>
concord audit <domain>/<task>
concord member preflight <domain>/<task>
concord member remove-landed <domain>/<task> <member> --apply
concord memory read <domain>/<task> --json
concord memory read <domain>/<task> --section focus --section next
concord memory patch <domain>/<task> --file -
concord memory write <domain>/<task> --expect <sha256> --file -
concord memory settle <domain>/<task> --expect <sha256> \
  --phase-file - --main-file <MAIN.md>
concord memory phase list <domain>/<task>
concord memory phase read <domain>/<task> 0
concord resource import <domain>/<task> <name> --source <path>
concord permissions normalize <domain>/<task> --apply
concord task finish <domain>/<task> --apply
concord domain migrate <domain> [--claim <task>/<member>=<path>] --apply
```

Use `--dry-run` on creation to print only the plan. Use global `--json` for
machine-readable output. Use `concord <command> --help` as the complete grammar.

Managed skill operations are:

```bash
concord skill install [--channel stable] [--version <stable>]
concord skill install --path <agent-skills>/concord
concord skill stage --channel beta --version <exact> --path <isolated>/concord
concord skill list
concord skill status
concord skill upgrade --dry-run
concord skill upgrade
concord skill uninstall
```

Managed install, status, and upgrade accept stable only and replace paths proven
Concord-managed by both the state ledger and in-path marker. `--force` applies
only to an already managed install.

`skill stage` requires an exact non-stable version and a new explicit path
ending in `concord`. It writes a staged marker without reading or writing the
managed ledger.
