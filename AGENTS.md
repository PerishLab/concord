# Agents

Read the canonical [PerishLab delivery governance](https://github.com/PerishLab/.github/blob/main/GOVERNANCE.md)
at work start and again before delivery or Issue closure. That document owns
organization-wide Issue, pull-request and acceptance policy; this file keeps
repository-specific constraints without copying that policy.

Concord is the executable control plane for one managed Space. GitHub Issues
are the durable work ledger. One exact Keel estate is the sole authority for
Issue-anchored local execution state; Git worktrees and Artifact directories
remain external payload and must agree with that estate. GitHub Issues and PRs
are the only work narrative and delivery ledger.

Five surfaces answer different questions. The source says what Concord does,
`--help` says how to invoke it, the Concord skill says how to operate its
objects, `concord cookbook CODE` owns bounded recovery, and this file records
repository constraints those surfaces cannot. Ask the owning surface instead
of copying its answer here.

## Authority

- Ordinary open replays exactly the current Issue-execution model. It never
  evolves, repairs, migrates, or falls back to an older managed shape.
- An Issue anchor retains only stable GitHub node identity, the minimum
  coordinate needed to observe it again, and a local execution revision. Forge
  prose, state, type, comments, and relationships remain GitHub truth.
- Git owns Member payload. Concord owns the Member seat and verifies source
  identity, branch, registered worktree, and derived path agreement. New
  Member, Claim, Boundary and pull-reference state relates directly to one
  Anchor and uses its stable node identity beneath `.issues`; Issue title and
  coordinate changes cannot move local payload.
- A Claim is a normalized declaration used for coordination and one Member's
  Boundary proof. It is not ownership, a lease, or a global exclusion lock.
  Overlap between Members sharing one canonical repository is an observation,
  never a mutation or audit fault.
- A Boundary proves only that one committed Member delta stays inside that
  Member's Claim. Concord composes an exact Issue-led delivery plan from the
  live Issue, Member and Boundary, delegates repository proof and candidate
  construction to Plumb's released delivery kernel, and invokes an explicit
  external provider command for PR mutation. Concord never carries provider
  credentials or replaces repository policy.
- A Guard proof is accepted only from the Plumb Concord was compiled against
  or the current Plumb stable named by its release seal; Plumb's exact
  producer and depot judgment is unchanged. A plan records the authority it
  was prepared under and refuses when that authority is no longer accepted.
  When stable is unreachable only the compiled authority remains, and the
  refusal names the unreachable authority. The stable location is fixed;
  tests inject authorities through the `Authorities` trait, never through
  runtime configuration.
- Optional Locus facts and the private activity ledger are observations only.
  They never establish authorship, ownership, presence, authorization, or
  lifecycle state and never replace a command result.
- Issue briefs, relationship graphs and closure readiness are bounded live
  GitHub projections. They retain exact provider cursors and observation time,
  distinguish structural parentage from blocking, never cache lifecycle state,
  and report provider failure or truncation rather than treating it as absence.
- Occupancy records only successful writes, expires without asserting session
  death, and warns on intersecting Issue, Member, or normalized repository
  write surfaces. It is never a lock or an authorization source.

## Boundaries

- Fail closed when the estate, coordinates, external seats, source identity,
  branch, or Git worktree metadata disagree. Read-only diagnosis remains
  available.
- Keep `.concord`, `.issues`, execution seats, and Artifacts private.
  Never edit or infer managed state by hand.
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
- Plumb owns repository governance, release markers, Guard proof, the shared
  Cookbook model, and the provider-neutral delivery kernel; Concord owns its
  compiled recovery entries, Issue/Member orchestration, and the explicit
  provider adapter. Wharf builds, binds, and distributes each release. Read
  their current help and rules; do not restate a release workflow or changelog
  shape here.
- `plumb.toml` and `ectropy.toml` are this repository's own declarations,
  layered over the base Plumb carries in its binary. Where they depart from
  that base, `plumb doctor` says so as a noted finding.
- The repository carries no workflow, no skill source, and no release notes.
  Release notes and skill generations live on Depot; the binary consumes its
  own skill generation through `concord skill` and never a floating release.
  Guard hooks are projected by `plumb configuration install`.
