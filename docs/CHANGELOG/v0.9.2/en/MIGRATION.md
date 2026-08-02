# Migrating to Concord v0.9.2

No task registry or memory migration is required. Observation remains disabled
unless `CONCORD_LOCUS_ENABLED=true` and a report file is configured.

Enabled operators will now receive process-cycle records for every successfully
parsed Concord command, rather than function records from `memory` commands
only. Review report capacity and retention for the broader coverage. The JSONL
Atom format, trace precedence, endpoint configuration, and owner-only creation
mode are unchanged.

Treat `cli.finish` as an immutable post-dispatch fact, not a liveness or active
executor signal. Query remains outside Concord:

```sh
locus query locus.trace "$CODEX_THREAD_ID" < report.jsonl
```
