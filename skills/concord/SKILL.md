---
name: concord
description: Operate the private Concord `.tasks` + `.task` control plane. Use when starting, locating, resuming, auditing, renaming, rehoming, landing, retaining, or finishing managed tasks; adding or removing repository worktree members; reading or settling task memory; managing task resources or permissions; or replacing any proposed raw mutation of `.tasks/`, `.task/`, task worktrees, or `tasks.toml`.
---

# Concord

Concord is the executable control plane for private task state. Treat the
installed task policy, repository instructions, recorded filesystem and Git
state, and current authorization as truth; authorship or agent memory is never
truth.

Before any stateful task operation, read
[references/protocol.md](references/protocol.md) completely. For pure command
discovery, prefer `concord <command> --help`.

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

**The right to amend travels with the layer.** Concord owns task protocol law.
Repository shape and release law belong to Plumb; load the `plumb` skill when
building or operating a repository rather than restating its clauses here.

## Operating workflow

1. Resolve the domain-space root from config, `CONCORD_DOMAIN_SPACE_ROOT`, or
   `--root`. Never assume `~/Projects`.
2. Resolve the requested task. Enter its task root, read `.task/MAIN.md` through
   `concord memory read` when present, and run `concord audit`.
3. Before acting in a member repository, enter that member and discover every
   applicable repository instruction file. Run the repository's normal Plumb
   doctor before changing its shape.
4. Use Concord for every managed structure or lifecycle mutation. Git owns
   member payload and repository landing; Concord owns member seats and
   agreement.
5. Re-audit after structural mutation. Keep `MAIN.md` current when the task is
   long-running; settle completed history into the next phase.
6. Land through the repository's own process, verify reachability and
   cleanliness, then remove only the landed member seat. Finish a repo-less task
   only after retained state is absent or its exact deletion is authorized.

## Laws

- One task has one fixed home domain, zero or more repository members, and at
  most one task-level `.task/`. A repo-less task is valid.
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
- `.task/` exists only for durable multi-round memory or artifacts. `MAIN.md`
  carries live execution state; settled rationale moves into numbered phases.
- Existing memory and retained task artifacts require exact-target deletion
  consent. Landing permission to discard member files is separate.
- Renaming, rehoming, legacy adoption, and mismatch repair are explicit
  migrations. Never perform one merely because a layout differs.

Read [references/protocol.md](references/protocol.md) for registry grammar,
entry resolution, lifecycle preconditions, memory layout, foreign-territory
handling, and recovery safety.

## Standing

Concord currently enforces:

- task/domain/member component syntax and registry schema validation;
- task-root, declared-member, canonical-source, branch, and Git worktree
  agreement;
- private task, memory, and resource permissions;
- clean source checkout before member creation;
- clean and reachable or tree-equivalent member state before landed removal;
- locking, registry compare-before-replace, and memory revision CAS;
- explicit plans, default-on creation, and execution gates for destructive or
  migratory commands;
- task finish only when no members or retained filesystem state remain;
- resource-seat initialization plus refusal of links and special files.

The following remain agent-held law; no machine will stop every violation:

- recognizing foreign territory and declining adoption;
- choosing the correct home domain and repository integration/landing
  conventions;
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
concord member add <domain>/<task> --source <integration-checkout>
concord audit <domain>/<task>
concord member preflight <domain>/<task>
concord member remove-landed <domain>/<task> <member> --apply
concord memory read <domain>/<task> --json
concord memory write <domain>/<task> --expect <sha256> --file <MAIN.md>
concord memory settle <domain>/<task> --expect <sha256> \
  --phase-file <phase.md> --main-file <MAIN.md>
concord resource import <domain>/<task> <name> --source <path>
concord permissions normalize <domain>/<task> --apply
concord task finish <domain>/<task> --apply
```

Use `--dry-run` on creation to print only the plan. Use global `--json` for
machine-readable output. Use `concord <command> --help` as the complete grammar.

Managed skill operations are:

```bash
concord skill install [--channel stable|beta] [--version <version>]
concord skill install --path <agent-skills>/concord
concord skill list
concord skill upgrade
concord skill uninstall
```

Install and upgrade only replace paths proven Concord-managed by both the state
ledger and in-path marker. `--force` applies only to an already managed install.
