# Migrating to Concord v0.12.3

Concord v0.12.3 preserves the v0.12.2 Keel model and estate protocol. Existing
Spaces open directly; no backup conversion or estate migration is required.

Nothing is owed by an operator who leaves observation muted, which is the
default. Task activity records and their warnings are unchanged.

A consumer that already parses an Atom report owes one change. The
`cli.start` and `cli.finish` payload `command` field carried a top-level group
such as `task` or `member`; it now carries the exact command such as
`task.show` or `member.attach`. A reader that matched those values literally
must match the new ones. Records written by earlier versions keep their old
values, so a mixed report contains both.

After installing the stable v0.12.3 manager, run:

```sh
concord --json audit
concord --version
```

Zero audit faults proves protocol agreement. To confirm the finer command fact:

```sh
CONCORD_LOCUS_ENABLED=true CONCORD_LOCUS_REPORT_FILE=/tmp/atoms.jsonl \
  concord --json task list
locus query locus.trace < /tmp/atoms.jsonl
```

The report's `cli.start` payload names `task.list`.
