#!/usr/bin/env sh
set -eu

COMMAND=${1:-install}
[ $# -gt 0 ] && shift || true
case "$COMMAND" in
  -h|--help|help)
    COMMAND=install
    set -- --help
    ;;
esac

CHANNEL=${CONCORD_CHANNEL:-stable}
VERSION=${CONCORD_VERSION:-}
PUBLIC_URL=${CONCORD_RELEASES_PUBLIC_URL:-https://releases.concord.perish.uk}
INSTALL_ROOT=${CONCORD_INSTALL_ROOT:-"$HOME/.local/share/concord"}
LOCAL_BIN_DIR=${CONCORD_LOCAL_BIN_DIR:-"$HOME/.local/bin"}
RETAIN=${CONCORD_RETAIN:-true}
MARKER=concord-manager-v1

while [ $# -gt 0 ]; do
  case "$1" in
    --channel) CHANNEL=${2:-}; shift 2 ;;
    --channel=*) CHANNEL=${1#--channel=}; shift ;;
    --version) VERSION=${2:-}; shift 2 ;;
    --version=*) VERSION=${1#--version=}; shift ;;
    --public-url) PUBLIC_URL=${2:-}; shift 2 ;;
    --public-url=*) PUBLIC_URL=${1#--public-url=}; shift ;;
    --install-root) INSTALL_ROOT=${2:-}; shift 2 ;;
    --install-root=*) INSTALL_ROOT=${1#--install-root=}; shift ;;
    --bin-dir) LOCAL_BIN_DIR=${2:-}; shift 2 ;;
    --bin-dir=*) LOCAL_BIN_DIR=${1#--bin-dir=}; shift ;;
    --retain) RETAIN=true; shift ;;
    --retain=*) RETAIN=${1#--retain=}; shift ;;
    -h|--help|help)
      cat <<'EOF'
concord manager

Usage:
  manage.sh install [--channel stable|beta] [--version X.Y.Z] [--retain[=true|false]]
  manage.sh update [--channel stable|beta] [--version X.Y.Z] [--retain[=true|false]]
  manage.sh uninstall [--version X.Y.Z]

Options:
  --public-url <url>     release metadata and artifact base URL
  --install-root <path>  versioned install root
  --bin-dir <path>       directory for the concord link

Environment:
  CONCORD_RELEASES_PUBLIC_URL
  CONCORD_CHANNEL
  CONCORD_VERSION
  CONCORD_INSTALL_ROOT
  CONCORD_LOCAL_BIN_DIR
  CONCORD_RETAIN
EOF
      exit 0
      ;;
    *) echo "unknown argument: $1" >&2; exit 1 ;;
  esac
done

for value in "$CHANNEL" "$PUBLIC_URL" "$INSTALL_ROOT" "$LOCAL_BIN_DIR"; do
  [ -n "$value" ] || { echo "manager option cannot be empty" >&2; exit 1; }
done
case "$CHANNEL" in
  stable|beta) ;;
  *) echo "invalid channel: $CHANNEL" >&2; exit 1 ;;
esac
case "$RETAIN" in
  true|false) ;;
  *) echo "invalid --retain value: $RETAIN" >&2; exit 1 ;;
esac
PUBLIC_URL=${PUBLIC_URL%/}

normalize_version() {
  normalized=$(printf '%s' "$1" | sed 's/^v//')
  printf '%s\n' "$normalized" |
    grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-beta\.[1-9][0-9]*)?$' || {
      echo "invalid concord version: $1" >&2
      exit 1
    }
  printf '%s' "$normalized"
}

latest_version() {
  sed -n 's/.*"releaseVersion"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$1" |
    head -n 1
}

asset_sha() {
  wanted="$1"
  metadata="$2"
  awk -v wanted="$wanted" '
    /"name"[[:space:]]*:/ {
      line = $0
      sub(".*\"name\"[[:space:]]*:[[:space:]]*\"", "", line)
      sub("\".*", "", line)
      held = line == wanted
    }
    held && /"sha256"[[:space:]]*:/ {
      line = $0
      sub(".*\"sha256\"[[:space:]]*:[[:space:]]*\"", "", line)
      sub("\".*", "", line)
      print line
      exit
    }
  ' "$metadata"
}

platform_archive() {
  case "$(uname -s):$(uname -m)" in
    Linux:x86_64|Linux:amd64) echo concord-x86_64-unknown-linux-gnu.tar.gz ;;
    Darwin:arm64|Darwin:aarch64) echo concord-aarch64-apple-darwin.tar.gz ;;
    *) echo "unsupported platform: $(uname -s) $(uname -m)" >&2; exit 1 ;;
  esac
}

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

root_owned() {
  [ -f "$INSTALL_ROOT/.concord-manager" ] &&
    [ "$(sed -n '1p' "$INSTALL_ROOT/.concord-manager")" = "$MARKER" ]
}

version_owned() {
  seat="$1"
  [ -f "$INSTALL_ROOT/$seat/.concord-manager" ] &&
    [ "$(sed -n '1p' "$INSTALL_ROOT/$seat/.concord-manager")" = "$MARKER" ] &&
    [ "$(sed -n '2p' "$INSTALL_ROOT/$seat/.concord-manager")" = "version=$seat" ]
}

ensure_root() {
  if [ -e "$INSTALL_ROOT" ]; then
    [ -d "$INSTALL_ROOT" ] || { echo "install root is not a directory: $INSTALL_ROOT" >&2; exit 1; }
    root_owned || { echo "refusing unowned install root: $INSTALL_ROOT" >&2; exit 1; }
    return
  fi
  mkdir -p "$INSTALL_ROOT"
  printf '%s\n' "$MARKER" > "$INSTALL_ROOT/.concord-manager"
}

entry_owned() {
  entry="$LOCAL_BIN_DIR/concord"
  [ -L "$entry" ] || return 1
  target=$(readlink "$entry" || true)
  case "$target" in
    "$INSTALL_ROOT"/v*/concord)
      seat=$(basename "$(dirname "$target")")
      version_owned "$seat"
      ;;
    *) return 1 ;;
  esac
}

old_versions() {
  current="$1"
  [ -d "$INSTALL_ROOT" ] || return 0
  for found in "$INSTALL_ROOT"/v*; do
    [ -d "$found" ] || continue
    seat=$(basename "$found")
    [ "$seat" != "$current" ] || continue
    version_owned "$seat" || continue
    printf '%s\n' "$seat"
  done
}

install_concord() {
  tmpdir=$(mktemp -d)
  stage=
  cleanup() {
    rm -rf "$tmpdir"
    [ -z "$stage" ] || rm -rf "$stage"
  }
  trap cleanup EXIT INT TERM

  if [ -z "$VERSION" ]; then
    curl -fsSL --retry 8 --retry-all-errors --retry-delay 5 \
      "$PUBLIC_URL/$CHANNEL/latest/metadata.json" -o "$tmpdir/latest.json"
    VERSION=$(latest_version "$tmpdir/latest.json")
    [ -n "$VERSION" ] || { echo "failed to resolve latest concord version" >&2; exit 1; }
  fi
  VERSION=$(normalize_version "$VERSION")
  seat_version="v$VERSION"
  archive=$(platform_archive)
  name=${archive%.tar.gz}

  curl -fsSL --retry 8 --retry-all-errors --retry-delay 5 \
    "$PUBLIC_URL/$CHANNEL/versions/$VERSION/metadata.json" -o "$tmpdir/version.json"
  metadata_version=$(latest_version "$tmpdir/version.json")
  [ "$metadata_version" = "$VERSION" ] || {
    echo "metadata version mismatch: expected $VERSION got $metadata_version" >&2
    exit 1
  }
  expected=$(asset_sha "$archive" "$tmpdir/version.json")
  [ -n "$expected" ] || { echo "metadata missing sha256 for $archive" >&2; exit 1; }
  curl -fsSL --retry 8 --retry-all-errors --retry-delay 5 \
    "$PUBLIC_URL/$CHANNEL/versions/$VERSION/$archive" -o "$tmpdir/$archive"
  actual=$(sha256 "$tmpdir/$archive")
  [ "$actual" = "$expected" ] || {
    echo "checksum mismatch for $archive: expected $expected got $actual" >&2
    exit 1
  }

  mkdir -p "$tmpdir/unpacked"
  tar -xzf "$tmpdir/$archive" -C "$tmpdir/unpacked"
  candidate="$tmpdir/unpacked/$name/concord"
  [ -f "$candidate" ] || { echo "archive missing concord" >&2; exit 1; }
  chmod +x "$candidate"
  staged_version=$("$candidate" --version)
  case "$staged_version" in
    *"$VERSION"*) ;;
    *) echo "binary version mismatch: $staged_version" >&2; exit 1 ;;
  esac

  entry="$LOCAL_BIN_DIR/concord"
  if [ -L "$entry" ]; then
    entry_owned || { echo "refusing unowned concord link: $entry" >&2; exit 1; }
  elif [ -e "$entry" ]; then
    [ -f "$entry" ] || { echo "refusing non-file concord entrypoint: $entry" >&2; exit 1; }
    cmp -s "$entry" "$candidate" || {
      echo "refusing unowned concord binary: $entry" >&2
      echo "install its exact version first to migrate the legacy entrypoint" >&2
      exit 1
    }
  fi

  ensure_root
  target="$INSTALL_ROOT/$seat_version"
  if [ -e "$target" ]; then
    version_owned "$seat_version" || { echo "refusing unowned version seat: $target" >&2; exit 1; }
    cmp -s "$target/concord" "$candidate" ||
      { echo "refusing mismatched version seat: $target" >&2; exit 1; }
  else
    stage="$INSTALL_ROOT/.concord-stage-$$"
    mkdir "$stage"
    install -m 0755 "$candidate" "$stage/concord"
    printf '%s\nversion=%s\n' "$MARKER" "$seat_version" > "$stage/.concord-manager"
    mv "$stage" "$target"
    stage=
  fi

  mkdir -p "$LOCAL_BIN_DIR"
  next_link="$LOCAL_BIN_DIR/.concord-link-$$"
  rm -f "$next_link"
  ln -s "$target/concord" "$next_link"
  mv -f "$next_link" "$entry"
  "$entry" --version

  if [ "$RETAIN" = false ]; then
    old_versions "$seat_version" | while IFS= read -r old; do
      [ -n "$old" ] || continue
      rm -rf "$INSTALL_ROOT/$old"
      printf 'removed old concord %s from %s\n' "$old" "$INSTALL_ROOT"
    done
  fi
  printf 'installed concord %s to %s\n' "$seat_version" "$entry"
}

uninstall_concord() {
  entry="$LOCAL_BIN_DIR/concord"
  if [ -n "$VERSION" ]; then
    VERSION=$(normalize_version "$VERSION")
    seat_version="v$VERSION"
    target="$INSTALL_ROOT/$seat_version"
    if [ -e "$target" ]; then
      root_owned || { echo "refusing unowned install root: $INSTALL_ROOT" >&2; exit 1; }
      version_owned "$seat_version" ||
        { echo "refusing unowned version seat: $target" >&2; exit 1; }
    fi
    if [ -L "$entry" ] && [ "$(readlink "$entry" || true)" = "$target/concord" ]; then
      rm -f "$entry"
    fi
    [ ! -e "$target" ] || rm -rf "$target"
    printf 'removed concord %s from %s\n' "$seat_version" "$INSTALL_ROOT"
    return
  fi

  [ ! -e "$INSTALL_ROOT" ] || root_owned ||
    { echo "refusing unowned install root: $INSTALL_ROOT" >&2; exit 1; }
  if [ -L "$entry" ]; then
    entry_owned || { echo "refusing unowned concord link: $entry" >&2; exit 1; }
    rm -f "$entry"
  elif [ -e "$entry" ]; then
    echo "refusing unowned concord binary: $entry" >&2
    exit 1
  fi
  if [ -d "$INSTALL_ROOT" ]; then
    for found in "$INSTALL_ROOT"/v*; do
      [ -d "$found" ] || continue
      seat=$(basename "$found")
      version_owned "$seat" || continue
      rm -rf "$found"
    done
    held=false
    for found in "$INSTALL_ROOT"/* "$INSTALL_ROOT"/.[!.]* "$INSTALL_ROOT"/..?*; do
      [ -e "$found" ] || [ -L "$found" ] || continue
      [ "$found" = "$INSTALL_ROOT/.concord-manager" ] && continue
      held=true
    done
    if [ "$held" = false ]; then
      rm -f "$INSTALL_ROOT/.concord-manager"
      rmdir "$INSTALL_ROOT"
    else
      echo "preserved unowned content under $INSTALL_ROOT" >&2
    fi
  fi
  rmdir "$LOCAL_BIN_DIR" 2>/dev/null || true
  printf 'removed concord from %s and %s\n' "$INSTALL_ROOT" "$entry"
}

case "$COMMAND" in
  install|update) install_concord ;;
  uninstall) uninstall_concord ;;
  *) echo "unknown command: $COMMAND" >&2; exit 1 ;;
esac
