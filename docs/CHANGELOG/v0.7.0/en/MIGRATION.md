# Migrating to Concord v0.7.0

Existing tasks and legacy Markdown memory require no automatic data migration.
Legacy MAIN documents remain readable and replaceable within the new limits,
but projection and sparse patching require an explicit opt-in to the
`concord-memory:v1` envelope. Ordinary write and settle never downgrade v1
memory, and structured MAIN and retained phases must use the same format.

Memory mutation commands now consume an explicit `--file` input after a
complete success by default. Pass `--keep-file`, or `--keep-files` for settle,
where the source must remain. Generated content should normally use `--file -`
and stdin. A `memory.cleanup_after_apply` error means the memory change
succeeded but input cleanup did not; inspect the returned revision before
retrying.

MAIN is limited to 400 lines and 64 KiB. Each PHASE is limited to 800 lines and
128 KiB, and raw MAIN reads have a 4 MiB diagnostic ceiling. Oversized existing
memory should be compacted or split into a smaller follow-up task. Audit
findings remain corrective memory hygiene rather than protocol disagreement.
