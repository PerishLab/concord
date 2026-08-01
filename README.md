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

Function observation is a separate, environment-only control section:

```text
CONCORD_LOCUS_ENABLED=false
CONCORD_LOCUS_REPORT_FILE=
CONCORD_LOCUS_TRACE_FILE=
CONCORD_LOCUS_TRACE_ID=
```

The gate defaults to false and returns before Locus bootstrap. Enabling it
requires a report file. An optional trace file shares one generated identity
across processes. When present, `CODEX_THREAD_ID` is collected exactly and
bound to `locus.trace`; an explicit trace ID wins over that collection, which
in turn wins over shared or random generation. Collection provenance remains
in the accepted Atom. Invalid observation config is handed to stderr after the
fact and never replaces the command result. The cold-start surface currently
covers only `memory` operations and records function entry plus normal return;
other commands do not bootstrap observation.

Common operations are explicit and composable:

```text
concord domain list
concord task start perish.code/ship-feature
concord task todo add perish.code/ship-feature follow-up
concord member add perish.code/ship-feature --source /srv/projects/perish.code/repo
concord audit perish.code/ship-feature
concord member preflight perish.code/ship-feature
concord memory read perish.code/ship-feature --json
```

A task todo is an outgoing reference to another ordinary task in the same
domain. It records a concrete future obligation before that task needs a
member, assignee, priority, or any other execution resource. Adding a missing
target creates its private repo-less task root and links it atomically;
repeating the add is a no-op. Removing the relation requires `--apply`:

```text
concord task todo add perish.code/ship-feature follow-up
concord task todo remove perish.code/ship-feature follow-up --apply
```

Finishing the source prints every target as a handoff and leaves those tasks
alive. A target cannot finish while another task still references it. Rename
rewrites incoming references; linked tasks refuse rehome until the relation is
removed or handed off. Todos carry identities only: issue text and execution
state remain with the target task and their owning systems.

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
concord memory init perish.code/ship-feature --file -
concord memory read perish.code/ship-feature --json
concord memory read perish.code/ship-feature --section focus --section next
concord memory patch perish.code/ship-feature --file -
concord memory write perish.code/ship-feature --expect SHA256 --file -
concord memory settle perish.code/ship-feature \
  --expect SHA256 --phase-file - --main-file MAIN.md
concord memory phase list perish.code/ship-feature
```

All mutation inputs accept `-`; prefer stdin for generated content. Explicit
files are bounded regular inputs and are consumed after a complete successful
mutation by default. Use `--keep-file` or settle's `--keep-files` to retain
them. If post-apply cleanup fails, Concord returns a typed nonzero error with
`applied=true` and the resulting revision rather than rolling memory back.

New memory may use the `concord-memory:v1` fixed-section envelope. A projected
`--section` result embeds its whole-file revision and can be sent directly to
`memory patch`; patching splices only selected source ranges. MAIN is limited
to 400 lines/64 KiB and each immutable PHASE to 800 lines/128 KiB. Audit
reports schema and limit violations as non-gating memory hygiene, while 16
retained phases emit a non-failing advisory.

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

`concord --help` is the complete command grammar. The canonical manager installs
the stable consensus:

```sh
curl -fsSL https://releases.concord.perish.uk/manage.sh | sh
```

Every non-stable release exists only as an exact seal. Resolve its fixed manager
and give it an isolated seat:

```sh
seal=https://releases.concord.perish.uk/v1/releases/beta/v0.5.0-beta.1/seal.json
manager=$(curl -fsSL "$seal" | jq -er '.managers.unix.url')
curl -fsSL "$manager" | sh -s -- install \
  --install-root "$HOME/.local/share/concord-beta" \
  --bin-dir "$HOME/.local/concord-beta/bin"
```

Concord also ships its operating brief as a release-matched managed skill:

```text
concord skill install
concord skill install --path ~/.codex/skills/concord
concord skill stage --channel beta --version v0.5.0-beta.1 \
  --path ~/.local/share/concord-beta/skills/concord
concord skill list
concord skill upgrade
concord skill uninstall
```

Default installation detects present Claude, Codex, shared agent, and OpenCode
skill directories. Replacement and removal require both Concord's
`state/skills.json` record and the target's `metadata.json` ownership marker;
the exact stage path is separate from managed state.
