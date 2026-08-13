# Migrating to Concord v0.12.2

Concord v0.12.2 preserves the v0.12.1 Keel model and estate protocol. Existing
Spaces open directly; no backup conversion or estate migration is required.

The new verbs are additive. Held Members, Claims, and Boundary proofs are
unchanged until an operator runs `member narrow` or `member retire`. `member
release` keeps its landed gate.

After installing the stable v0.12.2 manager, run:

```sh
concord --json audit
concord member --help
```

Zero audit faults proves protocol agreement. `member narrow` and `member retire`
appear on the help surface. To end an unlanded evidence Member whose content is
already preserved:

```sh
concord member prove TASK NAME --revision REV
concord member retire TASK NAME --artifacts NAME --revision REV --apply
```
