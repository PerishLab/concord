# Concord v0.5.3

## Native Windows task worktrees

Concord now passes ordinary Windows paths to Git instead of the verbatim
`\\?\` form returned by filesystem canonicalization. Task start, member add,
audit, repair, landing preflight, and removal can therefore operate on native
Windows worktrees without Git rejecting an otherwise valid path.

If `git worktree add` creates a branch and then fails before attaching the
worktree, Concord removes that newly created branch. A failed member add no
longer leaves the task name reserved by an orphan branch.

## Windows delivery stays exercised

The guard lane now runs Rust formatting, Clippy, tests, and a PowerShell manager
smoke on a native Windows runner. The repository guard also selects the native
manager smoke on Windows, accepts PowerShell invocations with one option, and
uses Deno rather than an undeclared `jq` dependency for release metadata smoke
tests.

Path assertions in the CLI config tests now use platform-native joins. A
Unix-only recovery fixture is explicitly Unix-only, so Windows Clippy checks the
code that can actually run on that platform.
