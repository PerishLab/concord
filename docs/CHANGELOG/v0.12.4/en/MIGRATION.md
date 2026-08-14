# Migrating to Concord v0.12.4

Concord v0.12.4 preserves the v0.12.3 Keel model and estate protocol. Existing
Spaces open directly; no backup conversion or estate migration is required.

Nothing is owed by an operator who sends a valid envelope today. The accepted
input is unchanged; only its description is new. A refusal keeps its
`concord.input.json` code and its message, and gains a populated `details`.

A consumer that already parses an Atom report owes one change. The `cli.finish`
payload gains a `fault` field carrying the typed refusal code, or null on
success. Records written by earlier versions have no such field, so a mixed
report contains both.

After installing the stable v0.12.4 manager, run:

```sh
concord --json audit
concord phase settle --help
```

Zero audit faults proves protocol agreement. The help now ends with the exact
envelope. To read the same shape from a refusal:

```sh
echo '{"version":1}' | concord --json phase settle
```

The error carries `details.envelope`.
