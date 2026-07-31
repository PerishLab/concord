# Concord registry claims v2

Read this reference before adding, claiming, proving, migrating, or removing a
repository member.

## Registry grammar

```toml
version = 2

[[task]]
name = "ship-feature"

[[task.repo]]
name = "repo-a"
source = "../repo-a"
write = ["crates/lib", "docs"]

[[task.repo]]
name = "repo-b"
source = "~/Projects/another-domain/repo-b"
branch = "legacy/slashed-branch"
write = ["."]

[[repo]]
name = "repo-a"
note = "optional annotation"
```

Every member declares one or more UTF-8 repo-relative path prefixes. `.` is the
only whole-repository spelling. Empty input, absolute paths, backslashes, NUL,
empty components, `.` components, and `..` components are invalid. Concord
sorts and deduplicates entries, then lets a parent absorb its descendants.

Prefix comparison is component-aware: `crates/lib` covers itself and
`crates/lib/src`, but not `crates/library`. Two active members sharing one
canonical Git common-directory identity conflict when either claim covers a
path in the other claim. Distinct Git identities do not conflict even when the
textual paths match.

Claim expansion is union-only. It rechecks all active members under the global
domain-space lock and clears a persisted boundary proof whenever the canonical
claim changes. Narrowing requires a future explicit operation; never edit the
registry to simulate it.

Version 2 refuses new orphan members because the Plumb v1 boundary proof
requires a commit merge-base. Existing version 1 orphan members remain
auditable and removable through the version 1 cleanup path.

## Boundary proof

Run `concord member boundary <task> <member>` after the member is clean and
committed, before repository landing. Concord requires a clean integration
checkout, computes the merge-base of integration and member HEAD, and invokes
the stable Plumb library with the exact normalized claim. A successful proof
records:

- Plumb boundary schema and resolved Plumb version;
- exact merge-base and member HEAD;
- SHA-256 digest of the NUL-delimited canonical claim.

The proof is invalid when member HEAD, claim digest, Plumb schema, or resolved
Plumb version changes. `member preflight` reports that state and version 2
`remove-landed` refuses it. Reachability or tree equivalence and worktree
cleanliness remain separate removal gates.

## Version 1 migration

Version 1 stays readable for audit, landing, and cleanup. It cannot add a
member, expand a claim, or record a boundary proof. Upgrade one whole domain:

```bash
concord domain migrate perish.code \
  --claim task-a/repo-a=crates/lib \
  --claim task-a/repo-a=docs \
  --claim task-b/repo-b=. \
  --apply
```

Every active member must be named at least once in the same guarded operation.
Repeated entries supply multiple prefixes. Concord rejects missing or unknown
members, malformed claims, and any proposed overlap before atomically replacing
version 1 with version 2. A version 1 binary rejects version 2 as unsupported;
it cannot silently ignore ownership state.
