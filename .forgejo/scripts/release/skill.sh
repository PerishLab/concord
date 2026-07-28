#!/bin/sh
set -eu

version=${1:-}
output=${2:-}
test -n "$version"
test -d "$output"

root=$(CDPATH= cd -- "$(dirname "$0")/../../.." && pwd)
source="$root/skills/concord"
test -f "$source/SKILL.md"

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT INT TERM
cp -R "$source" "$stage/concord"
cat > "$stage/concord/metadata.json" <<META
{
  "schema": 1,
  "name": "concord",
  "version": "$version",
  "keeper": "concord"
}
META
tar -C "$stage" -czf "$output/concord-skill.tar.gz" concord
