# Migrating to Concord v0.12.0

Concord v0.12.0 preserves the v0.11.0 Keel model and estate protocol. Existing
v0.11.0 Spaces open directly; no data migration, backup conversion, or release
artifact is required.

The first Task-scoped command carrying exactly one recognized session identity
lazily creates `.concord/activity/<task-key>.json` and its lock. The activity
directory is private `0700`, files are `0600`, and each Task file retains one
latest touch per session within fixed bounds. These files are auxiliary
observations and may not be edited as Task state.

Concord recognizes `CLAUDE_CODE_SESSION_ID`, `GROK_SESSION_ID`, and
`CODEX_THREAD_ID`. No recognized identity leaves observation muted. Multiple
identities or an unreadable activity file produce a warning but do not replace
the command result.

Success JSON remains on stdout. Structured session warnings use stderr, so
automation that treats any stderr output as command failure should instead use
the process exit status and inspect warning code
`concord.activity.concurrent_session` or `concord.activity.unavailable`.

After installing the stable v0.12.0 manager, run:

```sh
concord --json audit
concord --json task show TASK
```

Zero audit faults proves protocol agreement. A recent-session warning remains
observational and non-blocking.
