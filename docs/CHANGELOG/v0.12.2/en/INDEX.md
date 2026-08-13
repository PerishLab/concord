# Concord v0.12.2

## Member claims can shrink; evidence Members can exit

`member claim` still expands only. `member narrow` replaces the claim with the
given set, ends any held Boundary, and refuses a proposed set that `prove`
would refuse. A shrink that would leave committed work outside the remaining
claim is `concord.boundary.outside`. There is no force override.

`member release` still refuses an unlanded Member. `member retire` is the other
door: the same clean current-proof gates, no landed check, and at least one
Artifact name on the same Task must match `--artifacts`. `*` matches any name;
everything else is literal. Zero matches refuse. Quote `'*'` in the shell.

The shipped Concord skill documents both verbs. Expand and shrink each
invalidate the prior Boundary. An unlanded evidence Member is retired against a
matching Artifact, not reset onto main.
