#!/bin/sh
set -eu

version=$1
channel=$2
base=${CONCORD_RELEASES_PUBLIC_URL%/}
machine=$(uname -m)
system=$(uname -s)

case "$system:$machine" in
  Linux:x86_64) target=x86_64-unknown-linux-gnu ;;
  Darwin:arm64) target=aarch64-apple-darwin ;;
  *) echo "unsupported smoke platform: $system $machine" >&2; exit 1 ;;
esac

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
name="concord-$target"
curl -fsS --retry 5 --retry-all-errors \
  "$base/$channel/versions/$version/$name.tar.gz" \
  -o "$tmp/archive.tar.gz"
tar -xzf "$tmp/archive.tar.gz" -C "$tmp"
binary="$tmp/$name/concord"
"$binary" --version

skill="$tmp/agent-skills/concord"
"$binary" --home "$tmp/home" --releases "$base" \
  skill install --channel "$channel" --version "$version" --path "$skill"
test -f "$skill/SKILL.md"
test -f "$skill/metadata.json"
"$binary" --home "$tmp/home" --releases "$base" skill list |
  grep -F "$skill"
"$binary" --home "$tmp/home" --releases "$base" skill uninstall
test ! -e "$skill"
