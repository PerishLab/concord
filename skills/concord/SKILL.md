---
name: concord
description: Operate Concord Domains, Tasks, facts, Phases, dependencies, graphs, Members, Claims, Boundary proofs, and Artifacts across their managed lifecycle.
metadata:
  short-description: Objects and authorities for one managed task estate
---

# Concord

Concord governs one configured Space. Outside that boundary this brief is
silent. Use its estate as the sole authority for managed task state, read
revisions before mutation, and never edit managed state by hand.

The source says what Concord does, `--help` says how to invoke every command,
and this brief names the objects and authority boundaries needed to operate it.
Ask the owning surface instead of recalling a copied rule.

## Objects

- A Domain groups Tasks and Repository annotations.
- A Task has permanent identity, current facts, immutable Phases, and a revision.
- A Task name states what will be true once the Task is done. A standing line
  that has no terminal state names its function instead, which is correct for
  that shape and wrong for every other. A name taken from the finding that
  prompted the work binds nothing, so the work outgrows it.
- A current fact carries one role. Goal, Focus, and Next admit one fact each and
  omit rank. Constraint, Decision, Question, and Addition admit many and each
  carries a rank. Addition also carries a title and a nonblank origin.
- Goal is why the Task exists, Focus is its one current state, Next is the one
  step after this, Constraint binds how, Decision records what was settled, and
  Question holds what stays open.
- Addition is the running ledger of work in flight: one entry per item, titled
  by name and sourced by origin. Work that has not closed into a Phase belongs
  here, not inside Focus.
- A Dependency means `SOURCE depends_on TARGET`; its weight is `unknown`,
  `context`, `sequence`, or `required`.
- A Graph exposes direct edges, reachability, paths, cycles, and components.
- A Member binds one source identity, worktree, distinct branch, normalized
  Claims, and optional Boundary.
- A Claim declares the paths one Member intends to write and later proves. It
  is coordination evidence, not repository ownership, a lease, or an exclusion
  lock. Same-Task and cross-Task overlap succeeds and is reported as a
  `claim.overlap` observation for every active peer sharing the canonical Git
  identity.
- A Boundary proves one committed Member delta stays inside that Member's
  current Claim. It says nothing about PR state or another repository's landing
  policy.
- An Artifact is one named private directory attached to a Task.
- Recorded Task activity preserves operation and time. When exactly one
  supported session environment is visible, it may also carry Claude, Grok, or
  Codex agent and session context. That optional context is not authorship, a
  lock, owner, activity claim, authorization fact, or lifecycle state.

## Actions

- Inspect with bounded `task brief`, then `audit`, `task show`, `phase list`,
  `graph`, and `artifact list` as needed.
- Change current facts through a versioned `task change` envelope.
- Freeze an explicit Phase through `phase settle`.
- Add, weigh, list, or remove Dependencies; cycles and self-edges refuse.
- Attach, inspect status, claim, narrow, prove, release, and retire Members.
- Preflight, import, show, and remove Artifacts.
- Start, rename, rehome, and finish Tasks using current Task and Graph revisions.
- On a recent-session warning, preserve the reported operation and time plus
  any agent and session context, then apply the read/write strategy in the
  restrained scenario; never treat the warning itself as a gate.

## Authority split

Concord stops at estate lifecycle, local Git agreement, Claim coordination, and
per-Member Boundary proof. Plumb owns repository guard, PR consistency,
landing, product release, and skill placement. Do not make Concord discover or
mirror those states.

Follow [PATHS.md](PATHS.md) for routine execution and
[SCENARIOS.md](SCENARIOS.md) only when its case applies. Use
`concord <command> --help` for complete grammar.
