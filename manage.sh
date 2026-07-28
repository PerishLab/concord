#!/bin/sh
set -eu

command=${1:-install}
version=${2:-}
base=${CONCORD_RELEASES_PUBLIC_URL:-https://releases.concord.perish.uk}
seat=${CONCORD_INSTALL_DIR:-"$HOME/.local/bin"}
binary="$seat/concord"

case "$command" in
  install)
    if [ -z "$version" ]; then
      metadata=$(curl -fsS "$base/stable/latest/metadata.json")
      version=$(printf '%s' "$metadata" | sed -n 's/.*"version": *"\\([^"]*\\)".*/\\1/p' | head -1)
    fi
    test -n "$version"
    case "$(uname -s):$(uname -m)" in
      Linux:x86_64) target=x86_64-unknown-linux-gnu ;;
      Darwin:arm64) target=aarch64-apple-darwin ;;
      *) echo "unsupported platform" >&2; exit 1 ;;
    esac
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    name="concord-$target"
    curl -fsS "$base/stable/versions/$version/$name.tar.gz" -o "$tmp/archive.tar.gz"
    tar -xzf "$tmp/archive.tar.gz" -C "$tmp"
    mkdir -p "$seat"
    install -m 0755 "$tmp/$name/concord" "$binary"
    "$binary" --version
    ;;
  uninstall)
    rm -f "$binary"
    ;;
  *)
    echo "usage: manage.sh [install [version]|uninstall]" >&2
    exit 2
    ;;
esac
