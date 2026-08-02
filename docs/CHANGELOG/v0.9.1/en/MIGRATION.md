# Migrating to Concord v0.9.1

No task registry or memory migration is required. Observation remains disabled
unless `CONCORD_LOCUS_ENABLED=true` and a report file is configured.

When observation is enabled, trace identity precedence is now:

1. explicit `CONCORD_LOCUS_TRACE_ID`;
2. exact `CODEX_THREAD_ID` collection;
3. shared-file or random generation.

New Unix report files are created with `0600` permissions. Existing report
files keep their current mode. Operators that intentionally share an endpoint
must provision that policy explicitly.

The audit query stays outside Concord:

```sh
locus query locus.trace "$CODEX_THREAD_ID" < report.jsonl
```
