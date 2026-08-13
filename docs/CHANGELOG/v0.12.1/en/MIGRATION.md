# Migrating to Concord v0.12.1

Concord v0.12.1 preserves the v0.12.0 Keel model and estate protocol. Existing
Spaces open directly; no backup conversion or estate migration is required.

The private Task activity ledger upgrades lazily from version 1 to version 2 on
the next recorded Task command. Concord continues to read version-1 ledgers.
Version-2 touches always contain `operation` and `time`; `agent` and `session`
are optional context and may be absent. The ledger remains bounded auxiliary
observation and must not be edited as Task state.

`CLAUDE_CODE_SESSION_ID`, `GROK_SESSION_ID`, and `CODEX_THREAD_ID` remain the
supported context sources. Exactly one valid value may be attached. No value,
an invalid value, or multiple values leaves the context absent while the
operation and time are still retained.

After installing the stable v0.12.1 manager, run:

```sh
concord --json audit
concord --json task show TASK
```

Zero audit faults proves protocol agreement. Session warnings remain
observational and non-blocking.
