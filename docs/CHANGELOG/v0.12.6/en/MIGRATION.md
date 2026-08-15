# Migrating to Concord v0.12.6

Concord v0.12.6 preserves the v0.12.5 Keel model and estate protocol. Existing
Spaces open directly; no backup conversion or estate migration is required.

No command, envelope, output, or refusal changed. An operator owes nothing, and
every existing Task keeps its name.

The change reaches an agent through its installed skill. After installing the
stable v0.12.6 manager, run:

```sh
concord skill upgrade
concord skill status
```

The upgraded brief states what a Task name carries and adds the restrained
scenario for a deliverable that cannot yet be named.
