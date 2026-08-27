#!/usr/bin/env sh
set -eu

MANAGER_URL=https://releases.concord.perish.uk/v1/objects/sha256/0e893486a22b5b189a58367323588755a965aa2e1f4737f2d2a4aac8906a9e77/manage.sh
MANAGER_SHA=0e893486a22b5b189a58367323588755a965aa2e1f4737f2d2a4aac8906a9e77

usage() {
  cat <<'EOF'
Concord v0.11.0 legacy transition

Usage:
  migration.sh --root SPACE survey
  migration.sh --root SPACE stage
  migration.sh --root SPACE resume FINGERPRINT
  migration.sh --root SPACE activate FINGERPRINT --apply

The script validates the exact migration territory, obtains the immutable
v0.10.0 engine in an isolated temporary seat, and never changes the installed
Concord binary. Activation remains an explicit destructive authority.
EOF
}

fail() {
  printf 'concord migration: %s\n' "$1" >&2
  exit 1
}

case "${1:-}" in
  -h|--help|help) usage; exit 0 ;;
esac
[ "${1:-}" = --root ] || fail "--root SPACE is required"
[ -n "${2:-}" ] || fail "--root requires a value"
SPACE=$2
shift 2
ACTION=${1:-}
[ -n "$ACTION" ] || fail "an action is required"
shift

case "$ACTION" in
  survey|stage) [ $# -eq 0 ] || fail "$ACTION accepts no arguments" ;;
  resume)
    [ $# -eq 1 ] && [ -n "$1" ] || fail "resume requires one fingerprint"
    FINGERPRINT=$1
    ;;
  activate)
    [ $# -eq 2 ] && [ -n "$1" ] && [ "$2" = --apply ] ||
      fail "activate requires FINGERPRINT --apply"
    FINGERPRINT=$1
    ;;
  *) fail "unknown action: $ACTION" ;;
esac

[ -d "$SPACE" ] && [ ! -L "$SPACE" ] || fail "Space root must be one real directory"
SPACE=$(CDPATH= cd -P -- "$SPACE" && pwd)
ESTATE=$SPACE/.concord
MIGRATION=$ESTATE/migration
RELEASE=$MIGRATION/v0.10.0
STAGE=$RELEASE/stage
ROLLBACK=$RELEASE/rollback

directory() {
  [ ! -e "$1" ] && [ ! -L "$1" ] && return 0
  [ -d "$1" ] && [ ! -L "$1" ] || fail "managed directory is not direct: $1"
}

regular() {
  [ ! -e "$1" ] && [ ! -L "$1" ] && return 0
  [ -f "$1" ] && [ ! -L "$1" ] || fail "managed file is not regular: $1"
}

entries() {
  root=$1
  shift
  [ -d "$root" ] || return 0
  for entry in "$root"/* "$root"/.[!.]* "$root"/..?*; do
    [ -e "$entry" ] || [ -L "$entry" ] || continue
    leaf=${entry##*/}
    admitted=false
    for name in "$@"; do
      [ "$leaf" = "$name" ] && admitted=true
    done
    [ "$admitted" = true ] || fail "unknown migration territory: $entry"
  done
}

stage() {
  directory "$STAGE"
  entries "$STAGE" estate.sqlite3 sudo
  regular "$STAGE/estate.sqlite3"
  regular "$STAGE/sudo"
}

before() {
  directory "$ESTATE"
  regular "$ESTATE/estate.sqlite3"
  regular "$ESTATE/sudo"
  if [ -e "$ESTATE/estate.sqlite3" ] || [ -e "$ESTATE/sudo" ]; then
    fail "the Space already carries an active or partial estate"
  fi
  entries "$ESTATE" migration
  directory "$MIGRATION"
  entries "$MIGRATION" v0.10.0
  directory "$RELEASE"
  entries "$RELEASE" stage
  stage
}

after() {
  directory "$ESTATE"
  directory "$MIGRATION"
  directory "$RELEASE"
  if [ "$ACTION" = activate ]; then
    entries "$ESTATE" estate.sqlite3 migration sudo
    entries "$MIGRATION" v0.10.0
    entries "$RELEASE" rollback
    regular "$ESTATE/estate.sqlite3"
    regular "$ESTATE/sudo"
    directory "$ROLLBACK"
  else
    entries "$ESTATE" migration
    entries "$MIGRATION" v0.10.0
    entries "$RELEASE" stage
    stage
  fi
}

private() {
  for path in "$ESTATE" "$MIGRATION" "$RELEASE" "$STAGE" "$ROLLBACK"; do
    [ ! -e "$path" ] || chmod 0700 "$path"
  done
  for path in \
    "$STAGE/estate.sqlite3" "$STAGE/sudo" \
    "$ESTATE/estate.sqlite3" "$ESTATE/sudo"; do
    [ ! -e "$path" ] || chmod 0600 "$path"
  done
}

before
if [ "$ACTION" != survey ]; then
  private
fi
umask 077
TEMP=$(mktemp -d "${TMPDIR:-/tmp}/concord-v0.10.0.XXXXXX")
trap 'rm -rf -- "$TEMP"' EXIT HUP INT TERM
MANAGER=$TEMP/manage.sh
curl --fail --silent --show-error --location "$MANAGER_URL" --output "$MANAGER"
if command -v sha256sum >/dev/null 2>&1; then
  ACTUAL=$(sha256sum "$MANAGER" | awk '{print $1}')
else
  ACTUAL=$(shasum -a 256 "$MANAGER" | awk '{print $1}')
fi
[ "$ACTUAL" = "$MANAGER_SHA" ] || fail "v0.10.0 manager digest mismatch"
sh "$MANAGER" install \
  --channel stable \
  --version v0.10.0 \
  --install-root "$TEMP/install" \
  --bin-dir "$TEMP/bin" >&2
ENGINE=$TEMP/bin/concord
[ -x "$ENGINE" ] || fail "v0.10.0 manager produced no engine"

case "$ACTION" in
  survey|stage) "$ENGINE" --root "$SPACE" --json migration "$ACTION" ;;
  resume) "$ENGINE" --root "$SPACE" --json migration resume "$FINGERPRINT" ;;
  activate)
    "$ENGINE" --root "$SPACE" --json migration activate "$FINGERPRINT" --apply
    ;;
esac

after
if [ "$ACTION" != survey ]; then
  private
fi
