# Concord v0.12.1

## Lightweight Task activity

Every observed Task command now retains operation and time in the private
activity ledger, even when no session context can be selected. When exactly one
valid supported session environment is visible, Concord also attaches its agent
and native session as best-effort context. That context does not establish who
initiated the operation.

Recent-session warnings remain observational and non-blocking. Concord warns
only when comparable context shows a different session with a touch newer than
the current session's previous touch, so returning to the same Task does not
repeat an already observed warning.

Missing, invalid, or multiple supported session environments are ignored as
context and do not produce an unavailable warning. Ledger corruption or lock
contention can still produce `concord.activity.unavailable` without changing
the primary command result.

The shipped Concord skill restates operation and time plus any reported
context. Read-only work normally continues; writes re-read current state and
check the relevant Member, Claim, and local Git impact before proceeding.
