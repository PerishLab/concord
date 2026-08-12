# Concord v0.11.0

## Migration leaves the current product

The one-time legacy importer is no longer part of the Concord CLI or
`concord-core`. Current binaries expose only the permanent estate-backed Task,
Phase, dependency graph, Member, Artifact and audit protocol.

The release instead seals `migration.sh` and `migration.ps1` as version-owned
artifacts. Each script obtains the immutable v0.10.0 manager through its
content-addressed URL, verifies its fixed SHA-256 digest, installs the exact
v0.10.0 engine into a temporary isolated seat, and invokes that historical
engine without replacing the installed Concord binary.

Unix migration also repairs the v0.10.0 parent-mode defect. The script first
proves that `.concord` contains only the admitted migration territory, refuses
links and foreign entries, and only then normalizes managed directories to
`0700` and estate/possession files to `0600`.

## The artifact boundary stays generic

Plumb only preserves and seals the files under this version's `artifacts/`
directory. Concord owns the script pair, historical engine identity,
validation, permissions and transition semantics.
