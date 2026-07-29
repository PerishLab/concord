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
- Keep protocol agreement and resource health separate. Resource warnings are
  audit facts, not protocol faults; read, audit, landing, and cleanup remain
  available.
- Gate footprint-expanding operations on current headroom. Do not infer task
  ownership for arbitrary processes or silently clean retained state.
- Memory is opaque Markdown. Concord owns revisions and whole-file writes, not
  the meaning of headings.
- Git owns member payload and repository-specific landing. Concord owns the
  member seat and verifies the four protocol surfaces.

## Architecture

- `crates/lib` is the complete protocol kernel: configuration, discovery,
  registry, protocol and resource audit, Git seats, memory, permissions, and
  plans.
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

## Release

- `manage.sh` and `manage.ps1` leave exactly one version under the install root.
  Earlier versions are removed once the new binary is linked and answers
  `--version`, and each removal is named. `--retain` keeps what is there. The
  default was the opposite, protecting a rollback path that does not exist:
  `install --version <older>` refetches, so nothing ever read what accumulated.
- A stable release refuses to publish without
  `docs/CHANGELOG/v<version>/{en,zh}/{INDEX.md,MIGRATION.md}`, enforced by the
  `Changelog` step in `release-stable.yml` before anything irreversible.
  `plumb doctor` does not check this: a changelog is owed by a release, not by a
  working tree. A release requiring nothing of anyone still writes MIGRATION.md
  saying so. See `plumb/docs/changelog.md`.
