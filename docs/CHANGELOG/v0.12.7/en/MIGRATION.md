# Migrating to Concord v0.12.7

Concord v0.12.7 preserves the v0.12.6 Keel model and estate protocol. Existing
Spaces open directly; no backup conversion or estate migration is required.

No command, envelope, or output changed. Every refusal keeps its code, message,
and exit status.

Three refusals that previously carried `"details": null` now carry an object.
A reader that asserted the field was null must stop asserting it:

- `concord.task.absent` carries `identity` plus either `tasks` with `active`
  and `retired` counts, or `domains` when the domain is unknown.
- `concord.task.ambiguous` carries `identity` and the colliding `tasks`.
- `concord.domain.absent` carries `domain` and the managed `domains`.

`tasks` holds at most sixty-four active identities. When `active` exceeds the
list length the bound cut; `retired` states how many further names resolve but
are not listed.

The refusal vocabulary reaches an agent through its installed skill. After
installing the stable v0.12.7 manager, run:

```sh
concord skill upgrade
concord skill status
```

A repository that runs Concord's own suite while `CONCORD_LOCUS_ENABLED` is set
no longer appends fixture records to the configured report file. An operator
holding readings taken before this release should treat any `cargo test` window
as contaminated and cut a fresh baseline.
