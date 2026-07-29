# Concord v0.5.1

## Agreement gates mutation; hygiene only reports

Fail-closed agreement covers four surfaces: the registry entry, path presence
under the task root, canonical source-repository identity, and Git worktree
metadata. Ordinary mutation blocked on every audit finding instead, including
permission modes and links under `.task/`. Those describe hygiene of Concord's
own private storage and say nothing about whether the four surfaces agree.

The wider gate closed a loop no command could open. A resource tree that
arrives out of band can hold symbolic links, which `resource import` refuses up
front and therefore never created. `permissions normalize` was the only way to
clear the permission findings, but it stopped at the first link after applying
to part of the walk. `resource remove` was the only way to delete the tree
holding that link, and it was refused by the findings its own removal would
clear. Such a seat could be cleared only by reaching around Concord.

Audit now reports agreement and hygiene apart, and still exits non-zero on
either. Only agreement gates ordinary mutation.

## normalize finishes what it can fix

`permissions normalize` skips symbolic links instead of aborting on the first
one, and its plan names every link it will skip before anything runs. Audit
keeps reporting those links until the tree holding them is removed.
