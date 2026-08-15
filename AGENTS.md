# Agents

Concord is the executable control plane for the private `.tasks` + `.task`
protocol. Its filesystem and Git checks are protocol mechanics; agent or human
authorship is never protocol truth.

One Keel SQLite estate owns the structured state of one configured Space;
worktrees and direct Artifact directories remain external payload. Keel
bootstrap is the only estate genesis path, and ordinary open only replays the
exact sealed model.

## Laws

- Fail closed when the exact estate, Task coordinates, external seats, source
  identity, and Git worktree metadata disagree. Read-only diagnosis remains
  available.
- Keep `.concord`, `.tasks`, task roots, and Artifacts private. Managed
  directories are `0700`; the estate, retained sudo possession, refusal
  markers, and bounded migration evidence are `0600`.
- Mutations print an exact plan. Creation executes unless `--dry-run` is set;
  deletion, migration, recovery, and permission normalization require
  `--apply`.
- One Keel SQLite estate per Space is the sole structured authority. Lock the
  Space, re-read aggregate revisions, compare, then commit one Keel batch.
  Ordinary open uses exact seal replay and never evolves the estate; only the
  explicit release migration may bind a changed graph.
- Keep protocol agreement and resource health separate. Resource warnings are
  audit facts, not protocol faults; read, audit, landing, and cleanup remain
  available.
- Gate footprint-expanding operations on current headroom. Do not infer task
  ownership for arbitrary processes or silently clean retained state.
- MAIN and PHASE are typed estate Resources, never files or projections.
  Addition retains honestly unstructured text without weakening mature roles.
  It is where work in flight accumulates while no Outcome is yet true, one
  titled and sourced entry per item, so Focus stays the one current state and a
  Phase still closes only on a real Outcome.
- Git owns member payload and repository-specific landing. Concord owns the
  member seat and verifies the four protocol surfaces.
- Optional Locus observation is one-way. Context and report outcomes never
  enter protocol state, command output, authorization, or control flow.
- Task-scoped activity is a private auxiliary observation ledger, not Keel
  protocol state. It records operation and time. When exactly one supported
  session environment is visible, it may also attach agent and native session
  as context; that context does not establish authorship. Its warnings never
  revise Tasks, assert ownership or presence, enter authorization or audit
  agreement, or replace command results.

## Architecture

- `crates/lib` is the complete protocol kernel: configuration, exact estate,
  typed Task/Phase transactions, dependency graph, protocol and resource
  audit, Git seats, Artifacts, permissions, plans, and private activity
  observations.
- `crates/cli` contains clap grammar and output dispatch only.
- Dependency direction is `cli -> lib`.
- `runseal.toml` and `.runseal/resources` carry env-only repository-local
  profile material. Generic operation belongs to canonical workflows or the
  managed task substrate; product behavior does not enter either surface.
- Concord owns one process-cycle readonly observation seat. The CLI binds its
  explicit config, reporter, exact `CODEX_THREAD_ID` collection, trace identity,
  and context. When enabled, every parsed command appends product-owned start
  and finish facts around dispatch; the finish code is a post-command fact.
  Kernel functions may only read that seat and append independent function
  facts through Locus. A missing seat is the default muted state.

## Operating

- Never work or commit in the clean `main` integration checkout.
- Validate the repository profile with `runseal profile`. Use
  `runseal : <command> [args...]` only when a command needs its environment.
- Before landing, run `plumb doctor .`, `cargo fmt --all --check`,
  `cargo clippy --locked --workspace --all-targets -- -D warnings`,
  `cargo check --locked --workspace --all-targets --release`,
  `cargo test --locked --workspace`, and `ectropy .`.
- Land through the managed task substrate. Do not add a repository wrapper or
  Git hook for generic initialization, guard, landing, or release behavior.
- Skill content ships through Plumb's shared skill mechanism. Concord owns its
  vocabulary and standing; Plumb owns packaging and managed placement.

## Release

- Stable Plumb owns the complete binary release mechanism. This repository
  declares the product and skill in `plumb.toml`; its exact/stable workflows
  are thin Actions callers.
- Every release produces immutable content-addressed objects and one exact seal
  at `v1/releases/<channel>/<version>/seal.json`.
- Non-stable releases stop at their exact seal and install only through its
  generated manager into explicit isolated paths.
- Stable promotion proves an exact non-stable seal from the same commit. Only
  stable activation updates `v1/channels/stable.json` and the canonical root
  managers.
- Release jobs bind build, publication, smoke, and stable tagging to one
  resolved commit. Publication and activation use separate credentials.
- Generated managers and capsules are release outputs, not repository files.
- Managed skill install, status, and upgrade are stable-only. Exact non-stable
  briefs use `skill stage` at a new explicit path and never enter the ledger.
- A stable release requires
  `docs/CHANGELOG/v<version>/{en,zh}/{INDEX.md,MIGRATION.md}`, enforced by the
  stable capsule compiler before anything irreversible.
  `plumb doctor` does not check this: a changelog is owed by a release, not by a
  working tree. A release requiring nothing of anyone still writes MIGRATION.md
  saying so. Follow the release-local contract under `docs/CHANGELOG`.
- A version-owned migration entrypoint is a closed Concord contract under
  `docs/CHANGELOG/v<version>/artifacts/`. Plumb preserves its opaque bytes;
  Concord tests the script pair, historical engine identity, help surface and
  product-specific safety law.
