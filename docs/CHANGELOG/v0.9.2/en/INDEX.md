# Concord v0.9.2

Concord's optional readonly observation seat now brackets every successfully
parsed CLI command with product-owned `cli.start` and `cli.finish` Atom facts.
The pair shares one process-cycle span, and the finish fact records the command
name and exit code only after dispatch has returned.

Existing traced kernel functions remain independent facts with their own spans.
All records inherit the same trace identity, including an exact
`CODEX_THREAD_ID` when present. The gate remains default-muted and returns
before Locus bootstrap when disabled.

Concord does not consume observation Context or provide an audit query. It does
not infer executor ownership, activity, liveness, or authority. Operators keep
using the separate `locus query` CLI to read the append-only Atom stream.
