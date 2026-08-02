# Concord v0.9.1

Concord's optional readonly observation seat can now collect an exact ambient
Codex thread identity. When `CODEX_THREAD_ID` is present, the CLI binds it to
`locus.trace` through the named `codex.thread` collection and preserves the
collection provenance in every accepted Atom.

An explicit `CONCORD_LOCUS_TRACE_ID` still has highest precedence. Exact Codex
collection follows it and precedes shared-file or random trace generation. The
gate remains default-muted and returns before Locus bootstrap when disabled.

Concord now resolves Locus 0.2.1. A newly created Unix report is owner-only
`0600`, and operators can retrieve its trace facts through the separate
`locus query` CLI. Observation Context and report outcomes never enter Concord
protocol state, authorization, business output, exit status, or control flow.
