# Concord

Concord is the executable control plane for the private `.tasks` + `.task`
protocol. It audits agreement between the registry, task root, canonical
repository, and Git worktree metadata before ordinary mutation. It keeps no
hidden database.

Configuration contains one absolute domain-space root:

```toml
domain_space_root = "/srv/projects"
home = "/home/operator/.concord"
releases = "https://releases.concord.perish.uk"
```

The default config path is `~/.config/concord/config.toml` on Unix and
`%USERPROFILE%\AppData\Roaming\concord\config.toml` on Windows; global
`--config` selects an explicit file. Runtime policy follows the Plumb cascade:
defaults, file, typed `CONCORD_` environment, then arguments.
`CONCORD_DOMAIN_SPACE_ROOT` or `--root` supplies the required task root;
`CONCORD_HOME` or `--home` selects user state. Concord does not assume
`~/Projects`, and skill commands do not require task-domain configuration.

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

Concord also ships its operating brief as a release-matched managed skill:

```text
concord skill install
concord skill install --path ~/.codex/skills/concord
concord skill list
concord skill upgrade
concord skill uninstall
```

Default installation detects present Claude, Codex, shared agent, and OpenCode
skill directories. Replacement and removal require both Concord's
`state/skills.json` record and the target's `metadata.json` ownership marker;
unmanaged paths are refused.
