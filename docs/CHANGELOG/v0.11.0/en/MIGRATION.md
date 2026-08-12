# Migrating to Concord v0.11.0

This release keeps the v0.10.0 estate format and permanent operating protocol,
but moves the one-time legacy transition out of the current binary. Existing
Spaces already activated on v0.10.0 require no data migration. Legacy Spaces
must use the release-owned script pair described below.

## Preconditions

Use the latest v0.9.x binary first. Every Domain registry must be version 3,
the whole Space audit must agree, integration checkouts must be clean, and all
worktrees and Artifact payload must remain in place. Stop every other Concord
mutation and retain a recoverable filesystem backup or read-only clone.

Obtain `migration.sh` or `migration.ps1` from the exact v0.11.0 stable seal.
The artifacts are opaque to Plumb but content-addressed by that seal. Each
script independently pins and verifies the immutable v0.10.0 manager before it
installs the historical engine into a temporary isolated seat.

The script requires an explicit existing Space root. It refuses an active or
partial estate, symbolic links, unknown `.concord` entries, malformed stage
payload and ambiguous arguments. It never replaces the installed Concord
binary.

## Survey and stage

On Unix:

```sh
sh migration.sh --root /exact/space survey
sh migration.sh --root /exact/space stage
```

On Windows:

```powershell
./migration.ps1 -Root C:\exact\space -Action survey
./migration.ps1 -Root C:\exact\space -Action stage
```

Retain the returned `census.fingerprint`. `stage` creates and verifies the
estate beneath `.concord/migration/v0.10.0/stage` without fencing legacy
binaries. On Unix, the script normalizes only the already validated Concord
migration parents and files.

If an older stage exists, do not delete or overwrite it. Resume it only when
its live legacy evidence still has the same fingerprint. If later work has
made it stale, preserve it outside the active `.concord` migration territory
before creating a fresh stage.

Resume on Unix:

```sh
sh migration.sh --root /exact/space resume FINGERPRINT
```

Resume on Windows:

```powershell
./migration.ps1 -Root C:\exact\space -Action resume -Fingerprint FINGERPRINT
```

Any fingerprint change requires investigation. Never rewrite evidence to
force agreement.

## Activation

Activation is separately destructive. Review the exact Census, preserve the
backup and stage, and confirm every old Concord process is stopped.

On Unix:

```sh
sh migration.sh --root /exact/space activate FINGERPRINT --apply
```

On Windows:

```powershell
./migration.ps1 -Root C:\exact\space -Action activate -Fingerprint FINGERPRINT -Apply
```

The historical engine rechecks agreement, fences every legacy registry,
archives exact registry and MAIN/PHASE evidence under the v0.10.0 rollback
seat, renames filesystem resources to Artifacts, and activates the staged
database and sudo possession. The script then validates the resulting known
territory; Unix normalizes its managed modes.

After success, use the current v0.11.0 binary to run `concord --json audit`.
Zero faults proves ordinary mutation agreement. Preserve the rollback manifest
and evidence; deletion remains a separate exact-target authorization.
