# Concord v0.12.0

## Bounded Task entry

`task brief --domain DOMAIN` now projects a fixed-size page of current Task
Goal, Focus, Question, and Next facts. Its exact cursor keeps full Task and
Phase history opt-in while giving operators a deterministic portfolio entry
view.

## Readable Member health

`member attach` returns the canonical attached Member, and `member status`
projects local worktree cleanliness, Boundary currency, integration relation,
configured upstream divergence, and containing local tracking refs. Status is
read-only and never fetches or claims current remote truth.

## Factual session awareness

Task-scoped commands record one latest private touch per permanent Task key and
Claude, Grok, or Codex session. A different session observed within the fixed
recent window produces a non-blocking warning containing its agent, session,
operation, and time.

The activity ledger is auxiliary observation, not Keel protocol state. It does
not change Task revisions, authorization, lifecycle, audit agreement, or the
primary command result. The Concord skill normally continues read-only work;
before writes it inspects revisions, Members, Claims, and local Git impact, and
reports overlapping or unclear surfaces to the caller.
