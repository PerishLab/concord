# Agents

Concord is the executable control plane for one managed Space. One exact Keel
estate is the sole authority for structured Domain, Task, Phase, dependency,
Member, Claim, Boundary, and Artifact state. Git worktrees and Artifact
directories remain external payload and must agree with that estate.

Four surfaces answer different questions. The source says what Concord does,
`--help` says how to invoke it, the Concord skill says how to operate its
objects, and this file records repository constraints those surfaces cannot.
Ask the owning surface instead of copying its answer here.

## Authority

- Ordinary open replays the exact sealed estate model. It never evolves or
  repairs managed state; only an explicit release migration may change a model.
- Git owns Member payload. Concord owns the Member seat and verifies source
  identity, branch, registered worktree, and derived path agreement.
- A Claim is a normalized declaration used for coordination and one Member's
  Boundary proof. It is not ownership, a lease, or a global exclusion lock.
  Overlap between Members sharing one canonical repository is an observation,
  never a mutation or audit fault.
- A Boundary proves only that one committed Member delta stays inside that
  Member's Claim. Concord does not inspect a PR, Forgejo state, or another
  repository's landing policy. Repository-specific landing belongs to Plumb.
- Optional Locus facts and the private activity ledger are observations only.
  They never establish authorship, ownership, presence, authorization, or
  lifecycle state and never replace a command result.

## Boundaries

- Fail closed when the estate, coordinates, external seats, source identity,
  branch, or Git worktree metadata disagree. Read-only diagnosis remains
  available.
- Keep `.concord`, `.tasks`, Task seats, and Artifacts private. Never edit or
  infer managed state by hand.
- Lock the Space, re-read the relevant revision, compare, and commit one Keel
  batch for every mutation.
- Keep protocol agreement separate from coordination and health observations.
  An observation may guide an operator but cannot authorize or block work.
- Tests spawn Concord only through the shared test seat, which clears inherited
  `CONCORD_` configuration and redirects observation output into the fixture.

## Repository

- `crates/lib` is the protocol kernel; `crates/cli` owns grammar and output.
  Dependency direction is `cli -> lib`.
- Never work or commit in the clean `main` integration checkout.
- Run `plumb doctor .` before and after changing repository shape. Before
  landing, run `cargo fmt --all --check`,
  `cargo clippy --locked --workspace --all-targets -- -D warnings`,
  `cargo check --locked --workspace --all-targets --release`,
  `cargo test --locked --workspace`, and `ectropy .`.
- Plumb owns governed lanes, product release truth, depot changelog derivatives,
  skill packaging, and repository landing. Read its current help and rules;
  do not restate a released workflow or changelog shape here.
- Historical `docs/CHANGELOG` is a retired source seat. New release notes enter
  the Plumb depot flow and never recreate that directory.
