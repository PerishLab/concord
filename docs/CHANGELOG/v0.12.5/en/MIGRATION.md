# Migrating to Concord v0.12.5

Concord v0.12.5 preserves the v0.12.4 Keel model and estate protocol. Existing
Spaces open directly; no backup conversion or estate migration is required. No
role was added, removed, or changed.

An operator owes nothing. Every Task keeps its facts exactly as written, and a
Focus that carries a ledger today keeps carrying it until someone moves it.

A consumer that reads `--json` from a terminal will see one line where it used
to see an indented block. The decoded value is unchanged, so a parser needs no
change; only an eye does. Pipe through a formatter when reading by hand:

```sh
concord --json task brief --domain DOMAIN | python3 -m json.tool
```

A consumer that counted `phase.missing_outcome` observations across an estate
will see fewer of them. The observation now reports only for active Tasks. It
never entered audit agreement, so no refusal changes.

After installing the stable v0.12.5 manager, run:

```sh
concord --json audit
concord skill status
```

Zero audit faults proves protocol agreement. The upgraded skill names the roles
and the Addition ledger.
