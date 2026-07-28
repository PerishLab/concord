#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname "$0")/../../.." && pwd)
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT INT TERM

sh "$root/.forgejo/scripts/release/skill.sh" 0.2.0-test "$fixture"
tar -tzf "$fixture/concord-skill.tar.gz" >/dev/null
mkdir -p "$fixture/unpacked"
tar -xzf "$fixture/concord-skill.tar.gz" -C "$fixture/unpacked"
test -f "$fixture/unpacked/concord/SKILL.md"
test -f "$fixture/unpacked/concord/references/protocol.md"
jq -e \
  '.schema == 1 and .name == "concord" and
   .version == "0.2.0-test" and .keeper == "concord"' \
  "$fixture/unpacked/concord/metadata.json" >/dev/null

for name in \
  concord-x86_64-unknown-linux-gnu.tar.gz \
  concord-aarch64-apple-darwin.tar.gz \
  concord-x86_64-pc-windows-msvc.zip; do
  : > "$fixture/$name"
done
RELEASE_ROOT="$fixture" \
RELEASE_CHANNEL=stable \
RELEASE_VERSION=0.2.0-test \
CONCORD_RELEASES_PUBLIC_URL=https://releases.example.test \
CI_COMMIT=fixture \
  deno run \
    --allow-env \
    --allow-read="$fixture" \
    --allow-write="$fixture" \
    "$root/.forgejo/scripts/release/metadata.ts"
jq -e \
  '.schema == 1 and .version == "0.2.0-test" and
   .releaseVersion == "0.2.0-test" and
   (.assets | length == 3) and
   (.artifacts | length == 1) and
   (.artifacts.skillTarGz.sha256 | length == 64) and
   (.artifacts.skillTarGz.url |
     endswith("/stable/versions/0.2.0-test/concord-skill.tar.gz"))' \
  "$fixture/metadata.json" >/dev/null
