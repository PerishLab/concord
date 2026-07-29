# Migrating to Concord v0.5.1

Existing tasks, registries, worktrees, and task memory require no data
migration.

Mutation now proceeds while permission and link findings are outstanding, so a
task those findings previously blocked becomes operable with no repair step.
`concord audit` still reports them and still exits non-zero. Automation that
reads a non-zero audit as "no command will run" should look at the `hygiene`
group in the human report, or classify the JSON `faults` array by `kind`, and
decide per finding.

`permissions normalize` no longer fails on a symbolic link under `.task/`. A
plan that previously ended in an error now names every skipped link and applies
each mode around it.
