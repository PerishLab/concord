# Migrating to Concord v0.7.1

No task, registry, or memory migration is required. Observation remains off
unless `CONCORD_LOCUS_ENABLED=true` is inherited by the process.

An enabled process must set `CONCORD_LOCUS_REPORT_FILE`. It may also set
`CONCORD_LOCUS_TRACE_FILE` for one shared generated trace identity or
`CONCORD_LOCUS_TRACE_ID` for an explicit identity. Invalid observation
configuration is diagnostic only and does not change the memory command's
result.
