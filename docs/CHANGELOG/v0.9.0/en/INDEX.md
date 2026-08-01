# Concord v0.9.0

Concord tasks can now carry a small outgoing todo list of ordinary future
tasks. `concord task todo add` links an existing same-domain task or creates a
repo-less target and links it in one locked operation. Repeating the same add
is byte-stable. `task todo remove` explicitly unlinks one target.

The relation stays deliberately smaller than an issue tracker or scheduler.
It has no status, assignee, priority, lease, dependency execution, or copied
prose. The target enters Concord's normal task lifecycle immediately and may
later own any number of issues, repositories, releases, decisions, and cleanup
actions.

Finishing a source task prints every surviving todo as a handoff and leaves the
targets intact. A target cannot finish while a source still references it.
Task rename rewrites incoming references atomically; linked task rehome is
refused until the relations are removed or handed off.

Registry version 3 owns the relation and validates sorted unique targets,
same-domain existence, and self-reference refusal. This keeps older binaries
fail-closed instead of allowing them to ignore an incoming relation during
task cleanup.
