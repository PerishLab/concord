# Migrating to Concord v0.12.8

Concord v0.12.8 keeps the Keel model and estate protocol of v0.12.7. Existing
Spaces open directly, with no backup conversion and no estate migration.

No command, envelope, exit code or refusal vocabulary changed.

One refusal stops appearing. `concord.member.absent` no longer answers a Task
addressed by its own name:

```sh
concord member prove work repo --revision 3
concord member narrow work repo --claim crates --revision 4 --apply
```

Both now resolve exactly as `perish.code/work` did. A caller that already wrote
the full identity is unaffected; both spellings resolve the same Member, as they
always did for `member status` and `member attach`.

An operator who worked around this by qualifying every Task name may stop, but
nothing requires it. Scripts that assert on the refusal itself should stop
expecting it for a Member that exists.

`concord.member.absent` still answers a Task that exists with no such Member and
a Task that does not exist with the same message. Distinguishing those two
remains open.
