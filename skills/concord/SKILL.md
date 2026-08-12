---
name: concord
description: Operate Concord Domains, Tasks, facts, Phases, dependencies, graphs, Members, Claims, Boundary proofs, and Artifacts across their managed lifecycle.
---

# Concord

Use Concord as the sole authority for managed task state. Read revisions before
mutation, prefer JSON for deterministic reads, and never edit managed state by
hand.

## Objects

- A Domain groups Tasks and Repository annotations.
- A Task has permanent identity, current facts, immutable Phases, and a revision.
- A Dependency means `SOURCE depends_on TARGET`; its weight is `unknown`,
  `context`, `sequence`, or `required`.
- A Graph exposes direct edges, reachability, paths, cycles, and components.
- A Member owns one worktree, branch, normalized Claims, and optional Boundary.
- An Artifact is one named private directory attached to a Task.

## Actions

- Inspect with `audit`, `task show`, `phase list`, `graph`, and `artifact list`.
- Change current facts through a versioned `task change` envelope.
- Freeze an explicit Phase through `phase settle`.
- Add, weigh, list, or remove Dependencies; cycles and self-edges refuse.
- Attach, claim, prove, and release Members.
- Preflight, import, show, and remove Artifacts.
- Start, rename, rehome, and finish Tasks using current Task and Graph revisions.

Follow [PATHS.md](PATHS.md) for routine execution and
[SCENARIOS.md](SCENARIOS.md) only when its case applies. Use
`concord <command> --help` for complete grammar.
