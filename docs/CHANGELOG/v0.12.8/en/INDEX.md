# Concord v0.12.8

## A Member resolves from the Task that was addressed

`concord member narrow work repo` reported `concord.member.absent`. The Member
was attached, `concord member status work repo` found it, and the refusal named
an attachment that had never failed.

`Estate::member` matches a stored Worktree on the Task identity, `domain/name`.
Two callers passed one: `member_status`, and the lookup that closes `attach`.
Five passed the request's raw task string instead, so `narrow`, `prove`,
`claim`, `release` and `retire` refused every Task named without its Domain.

The divergence sat in plain sight. Each of those five already resolves the Task
one line earlier, and `release.rs` was already spelling `task.identity()`
further down its own file, so both spellings lived in one file at once. They now
agree, and all ten call sites read the identity.

`status` and `attach` were the two deeds an operator reaches for first, which is
why a Member could be attached and inspected by name and then refuse every deed
that would advance it.

## The tests only exercised the spelling that worked

Every estate test addressed its Task as `local/work`. The bug was therefore
invisible to a suite that otherwise covers attach, prove, narrow, claim, release
and retire.

`named` addresses one Task by its own name through attach, prove, narrow and
claim. Reverted against this release it fails on the first deed with `member not
found: work/repo` — a key missing the Domain it is stored under.
