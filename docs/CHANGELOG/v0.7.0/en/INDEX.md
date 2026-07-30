# Concord v0.7.0

Concord memory now has a stdin-first, bounded input lifecycle. Every memory
mutation accepts standard input, explicit input files are consumed after a
complete success by default, and cleanup failures report that the mutation was
already applied together with its resulting revision.

This release also introduces the opt-in `concord-memory:v1` envelope. Agents can
project only the fixed MAIN sections they need and send the edited projection
directly to `memory patch`; Concord preserves every untouched source byte and
quick-fails stale revisions. Immutable phases can now be listed and read
through the CLI.

MAIN and PHASE documents have fixed line and byte ceilings, and audit reports
schema, limit, phase-name, continuity, and mixed-format findings as memory
hygiene. Sixteen retained phases produce a non-failing task-splitting advisory.
The release-matched Concord skill includes the complete v1 grammar and makes
stdin the default agent workflow.
