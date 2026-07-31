# Concord v0.8.0

Concord now allocates mutable repository paths, not merely worktree seats.
Registry version 2 requires every member to declare normalized repo-relative
`write` prefixes. The domain-space lock compares those claims across every
active task sharing one canonical Git common-directory identity. Exact and
ancestor/descendant overlap is refused; sibling component prefixes remain
concurrent. Existing claims can grow only by union.

Delivery now has a mechanical boundary proof. `concord member boundary` binds
the member HEAD, merge-base, normalized claim digest, Plumb proof schema, and
the exact resolved Plumb version after the stable Plumb library proves the
committed Git delta stays inside the claim. A changed HEAD or expanded claim
invalidates the record. Version 2 landed removal refuses without a current
proof, in addition to the existing cleanliness and reachable-or-tree-equivalent
checks.

Registry version 1 remains readable for audit, landing, and cleanup. It cannot
add or expand members. `concord domain migrate` performs one explicit atomic
upgrade only when every active member has been assigned a claim and the
proposed domain remains conflict-free.
