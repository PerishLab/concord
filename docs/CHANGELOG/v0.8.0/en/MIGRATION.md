# Migrating to Concord v0.8.0

Registry version 2 is required before adding a member, expanding a claim, or
recording a boundary proof. Inventory every active member in the domain and
assign the smallest truthful repo-relative path prefixes:

```bash
concord domain migrate perish.code \
  --claim task-a/repo-a=crates/lib \
  --claim task-a/repo-a=docs \
  --claim task-b/repo-b=. \
  --apply
```

Every active member must appear at least once. Repeated entries form one
normalized claim. Use `.` only for intentional whole-repository ownership.
The command refuses unknown members, missing claims, malformed paths, and
overlap with any active member sharing the same canonical Git identity.

After migration, create members with at least one `--write` value. Before
landing a member, run `concord member boundary <task> <member>`. If its HEAD or
claim changes, rerun the proof before landing. `member preflight` and
`remove-landed` will refuse a missing or stale version 2 proof.

Version 1 registries remain available for read, audit, landing preflight, and
cleanup. A pre-v0.8.0 Concord binary rejects a version 2 registry rather than
silently ignoring claims.

New orphan members are refused in version 2 because the current Plumb boundary
proof requires a commit merge-base. Existing version 1 orphan members retain
their audit and cleanup path.
