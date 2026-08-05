# Concord memory v1

Read this reference only when initializing, projecting, patching, settling, or
diagnosing structured task memory.

## Text and limits

Every structured document is UTF-8 without BOM, uses LF, and ends with a final
LF. A MAIN document is at most 400 lines and 64 KiB. One PHASE is at most 800
lines and 128 KiB. Raw `memory read` refuses a stored file above 4 MiB.

The marker is the first line. Unknown marker versions remain available through
raw MAIN read for diagnosis but refuse structured mutation. Unmarked Markdown
is legacy memory: it can still be read and replaced within the limits, but it
cannot be projected or patched.

## MAIN envelope

A v1 MAIN starts with:

```markdown
<!-- concord-memory:v1 -->
```

It has exactly these document-root H2 sections in this order:

```markdown
## Goal
## Active constraints
## Decisions in force
## Current focus
## Open questions
## Next step
```

Their command keys are `goal`, `constraints`, `decisions`, `focus`,
`questions`, and `next`. Content before the first H2 and every section body is
opaque Markdown. Use H3 or deeper headings to extend a body. H2 text inside a
fence, quote, or another CommonMark container is body content, not a boundary.

Ordinary write may opt a legacy MAIN into v1 only when every retained phase can
remain the same format. Ordinary write and settle never downgrade a v1 MAIN.
There is no automatic legacy migration.

## Sparse projection and patch

Project only the current sections needed:

```bash
concord memory read DOMAIN/TASK --section focus --section next
```

The returned `content` is directly patchable and begins with:

```markdown
<!-- concord-memory-patch:v1 revision=<sha256> -->
```

It contains the selected H2 spans in canonical order. Edit bodies without
changing the marker or fixed H2. Apply it through stdin:

```bash
concord memory patch DOMAIN/TASK --file -
```

The embedded revision is the whole current MAIN revision. `--expect` is
optional; when present it must equal the embedded value. A revision conflict
quick-fails before writing. Concord replaces only selected source ranges and
does not reserialize untouched Markdown. A byte-identical result returns
`changed=false` with the same revision and skips the managed rewrite.

## PHASE envelope

A structured phase starts with:

```markdown
<!-- concord-phase:v1 -->
```

It has exactly these document-root H2 sections in order:

```markdown
## Outcome
## Decisions
## Evidence
## Carry-forward
```

MAIN and all retained phases use the same format. A created phase is immutable.
`memory phase list` validates names, continuity, format, and limits before
listing. `memory phase read TASK NUMBER` reads one validated phase.
Immutability also holds at task cleanup: once any phase exists, `memory remove`
refuses to delete the `.task/` tree. Retain that repo-less task as lineage and
start a follow-up task for later work. Unphased memory remains exactly
removable.

Settle requires the replacement MAIN to differ from the held MAIN before it
allocates the next `PHASE-NN.md`. If phase creation succeeds but MAIN
replacement fails, the typed error returns the retained phase path and current
revision. Complete recovery with an ordinary memory write; Concord does not
keep a journal or replay the settle automatically.

## Input lifecycle

`memory init`, `patch`, `write`, and both settle inputs accept `PATH` or `-`.
Prefer stdin for generated content. Settle admits at most one stdin and requires
distinct explicit paths.

File input must be a bounded regular file, not a symlink, special file, managed
memory path, or duplicate settle source. After a complete success, Concord
re-reads it, compares its payload hash, and removes it by default. Use
`--keep-file` or settle's `--keep-files` to retain it.

If the path changes or removal fails after apply, Concord does not roll back.
It exits nonzero with code `memory.cleanup_after_apply`; JSON details include
`applied=true`, the resulting revision, and the input path whose cleanup was
refused or failed.

## Audit and correction

Schema, text, size, malformed phase name, gap, and mixed-format findings are
memory hygiene. They make audit nonzero but do not break four-surface agreement
or block corrective memory mutation. Sixteen retained phases produce one
advisory without changing audit success.

There is no local multi-file transaction, journal, file-identity proof,
automatic migration, exactly-once cleanup, monotonic revision, or ABA defense.
On ambiguity, quick-fail and use the reported current state.
