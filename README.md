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

The default config path is `~/.concord/concord.toml` on Unix and
`%LOCALAPPDATA%\concord\concord.toml` on Windows; global `--config` selects an
explicit file. Concord does not discover or fall back to another config seat.
Runtime policy follows the Plumb cascade: defaults, file, typed `CONCORD_`
environment, then arguments.
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

`audit` keeps protocol agreement and local resource health separate. Protocol
faults still fail the command; resource findings are advisory `OK`, `WARN`,
`CRIT`, or `UNKNOWN` observations and never make a task impossible to inspect,
land, or clean up. Each task audit measures allocated task, member, memory, and
resource-seat bytes, plus the task filesystem, inode capacity where available,
and current host memory:

| Observation | WARN | CRIT |
| --- | ---: | ---: |
| task or member footprint | 2 GiB | 8 GiB |
| memory or resource seat | 512 MiB | 2 GiB |
| filesystem or inode use | 60% | 75% |
| host memory available | 40% | 25% |

These intentionally conservative defaults catch accidental growth early in a
Rust-oriented workplane. Concord records no resource history and attributes no
arbitrary process to a task. Use global `--json` for structured observations;
an unavailable platform metric is reported as `UNKNOWN`.

Member preflight returns one record per declared member. A successful record
proves canonical repository identity, the expected branch, a clean worktree,
and either landed commit reachability or exact tree equivalence. JSON output
carries the Git identities, heads, and trees used as evidence.

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

`memory write --file -` reads MAIN from stdin. `memory settle` accepts `-` for
either `--phase-file` or `--main-file`; the other input must remain a file so
the two payloads are never ambiguously framed.

Resource payload remains opaque after Concord allocates or imports its private
seat:

```text
concord resource allocate perish.code/ship-feature evidence
concord resource import perish.code/ship-feature logs --source ./logs
concord resource show perish.code/ship-feature logs
```

Import preflight measures the complete source tree and preserves at least 25%
filesystem and inode headroom (with a 1 GiB minimum byte reserve). Apply
rechecks under the task lock and copies files as a stream. Member creation also
refuses to expand a filesystem already at the critical threshold.

`concord --help` is the complete command grammar. The stable binary is
installed or updated with `manage.sh` on Linux/macOS or `manage.ps1` on
Windows:

```text
manage.sh install
manage.sh update
manage.sh uninstall
```

Managers verify release metadata and the staged binary before activating a
versioned seat under `~/.local/share/concord/vX.Y.Z/`. The stable
`~/.local/bin/concord` entrypoint points at the selected version. Existing
entrypoints and version seats are replaced only when their ownership can be
proven; `--retain=false` explicitly prunes older managed versions.

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
