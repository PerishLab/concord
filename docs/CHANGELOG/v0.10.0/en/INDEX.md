# Concord v0.10.0

Concord now stores its structured control plane in one Keel SQLite estate per
Space. Domain, Repository, permanent Task identity, current Task facts, frozen
Phases, Member/Claim/Boundary state, and dependency closure have one database
authority. Keel bootstrap owns genesis and ordinary open performs exact sealed
replay without implicit schema evolution.

Task dependencies are directed `source depends_on target` edges with ordered
weights `unknown < context < sequence < required`. They expose coordination
problems but never schedule or block ordinary work. Self-edges and cycles are
invalid from day zero. Adjacency, neighbors, degree, reach, shortest path,
cycles, strongly connected components, and deterministic export are available
at cold start.

Task state is authored through versioned, stdin-first JSON change-sets. Settle
creates one immutable Phase and applies an explicit current-state delta in one
Task revision. New Phases require exactly one nonblank Outcome. Task names are
lifetime-reserved, while rename and rehome retain the permanent Task key and
repair derived worktree and Artifact paths.

Artifacts remain private direct filesystem payload. They are discovered under
`.task/artifacts/` and never become Keel Resources. Repo-less Tasks allocate no
default directory. Estate audit checks graph closure, structured aggregates,
Member agreement, private custody, and foreign derived-seat territory before
ordinary mutation; dependency observations remain non-gating.

Migration Census fingerprints only the bytes that transition; live Member
payload remains under agreement without wasteful recursive hashing.

The current skill is a closed three-file brief: objects and actions, hot paths,
and restrained complex scenarios.

This is an intentionally breaking authority transition. The `todo`, `memory`,
generic filesystem `resource`, and permission-normalization command families
are absent. Active `MAIN.md` and `PHASE-NN.md` files cease to exist after
activation. Follow the exact [v0.10.0 migration contract](MIGRATION.md); there
is no compatibility alias or dual-write period.
