# Migrating to Concord v0.5.0

Existing tasks, registries, worktrees, and task memory require no data
migration.

Automation parsing the human `concord audit` sentence should move to global
`--json`. The human success line now names the agreement plane explicitly, and
JSON audit documents include a `resources` array.

Resource imports and member creation may now refuse before mutation when they
cannot preserve conservative local headroom. Free capacity or clean up through
an explicit task lifecycle operation, then retry; do not treat the refusal as a
protocol mismatch.

Install and update now keep only the selected managed version by default. Pass
`--retain` if older local version seats must remain available offline. Published
versions remain immutable and can still be selected explicitly for rollback.
