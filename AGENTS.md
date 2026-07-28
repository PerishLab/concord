# Agents

Concord is the executable control plane for the private `.tasks` + `.task`
protocol. Its filesystem and Git checks are protocol mechanics; agent or human
authorship is never protocol truth.

## Laws

- Fail closed when registry, task-root, source identity, and Git worktree
  metadata disagree. Read-only diagnosis remains available.
- Keep `.tasks`, task roots, and task memory private. Managed directories are
  `0700`; registry and memory files are `0600`.
- Mutations print an exact plan. Creation executes unless `--dry-run` is set;
  deletion, migration, recovery, and permission normalization require
  `--apply`.
- Lock, re-read, compare, then replace files atomically. Do not add a hidden
  database or persistent journal.
- Memory is opaque Markdown. Concord owns revisions and whole-file writes, not
  the meaning of headings.
- Git owns member payload and repository-specific landing. Concord owns the
  member seat and verifies the four protocol surfaces.

## Architecture

- `crates/lib` is the complete protocol kernel: configuration, discovery,
  registry, audit, Git seats, memory, permissions, and plans.
- `crates/cli` contains clap grammar and output dispatch only.
- Dependency direction is `cli -> lib`. Operator glue lives in `.runseal`;
  product behavior does not.

## Operating

- Never work or commit in the clean `main` integration checkout.
- `runseal :guard` must pass before `runseal :land`.
- `plumb doctor .` and `ectropy --strict .` must report no unknown or blind
  structure before landing.
- Skill content ships through Plumb's shared skill mechanism. Concord owns its
  vocabulary and standing; Plumb owns packaging and managed placement.
