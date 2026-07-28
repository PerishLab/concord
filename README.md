# Concord

Concord is the executable control plane for the private `.tasks` + `.task`
protocol. It audits agreement between the registry, task root, canonical
repository, and Git worktree metadata before ordinary mutation. It keeps no
hidden database.

Configuration contains one absolute domain-space root:

```toml
domain_space_root = "/srv/projects"
```

The config path comes from `CONCORD_CONFIG` or the platform config home.
`CONCORD_DOMAIN_SPACE_ROOT` and the global `--root` option provide controlled
overrides. Concord does not assume `~/Projects`.

Common operations are explicit and composable:

```text
concord domain list
concord task start perish.code/ship-feature
concord member add perish.code/ship-feature --source /srv/projects/perish.code/repo
concord audit perish.code/ship-feature
concord member preflight perish.code/ship-feature
concord memory read perish.code/ship-feature --json
```

Creation prints its plan and executes by default; add `--dry-run` to stop after
planning. Deletion, task migration, permission normalization, and landed-seat
removal only execute with `--apply`.

Task memory uses whole-file compare-and-swap:

```text
concord memory init perish.code/ship-feature --file MAIN.md
concord memory read perish.code/ship-feature --json
concord memory write perish.code/ship-feature --expect SHA256 --file MAIN.md
concord memory settle perish.code/ship-feature \
  --expect SHA256 --phase-file PHASE.md --main-file MAIN.md
```

Resource payload remains opaque after Concord allocates or imports its private
seat:

```text
concord resource allocate perish.code/ship-feature evidence
concord resource import perish.code/ship-feature logs --source ./logs
concord resource show perish.code/ship-feature logs
```

`concord --help` is the complete command grammar. The stable binary is
installed with `manage.sh` on Linux/macOS or `manage.ps1` on Windows.
