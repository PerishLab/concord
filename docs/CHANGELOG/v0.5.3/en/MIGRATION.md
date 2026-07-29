# Migrating to Concord v0.5.3

Existing tasks, registries, worktrees, task memory, configuration, and managed
skill seats require no data migration.

Windows operators should update the binary and managed Concord skill together.
Task operations that previously failed because Git received a `\\?\` path can
be retried after the update. If an earlier failed `member add` left a branch
with the task name, inspect its contents and remove or preserve it deliberately
before retrying; Concord does not infer ownership of branches created by an
older release.

Linux and macOS behavior is unchanged.
