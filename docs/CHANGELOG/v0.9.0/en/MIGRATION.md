# Migrating to Concord v0.9.0

Task todos require registry version 3. Preview and migrate each version 2
domain without member claims:

```bash
concord domain migrate perish.code
concord domain migrate perish.code --apply
```

The migration changes only the registry version; existing tasks, members,
claims, and boundary proofs remain byte-equivalent in meaning. A version 1
domain still supplies every active member claim with repeated `--claim`
arguments, and v0.9.0 migrates it directly to version 3.

After migration, a pre-v0.9.0 Concord binary rejects the registry rather than
silently ignoring todo state. Upgrade every writer before applying the domain
migration.

Create or link a future task with:

```bash
concord task todo add perish.code/current future
```

Creation executes unless `--dry-run` is present. Removing a relation remains
guarded:

```bash
concord task todo remove perish.code/current future --apply
```

Version 3 todos are same-domain. Finishing the source reports handoffs and
keeps its targets. To remove a target first, explicitly unlink every incoming
todo or finish the source tasks that carry them.
