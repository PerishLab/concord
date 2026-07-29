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
deno eval '
  const value = JSON.parse(await Deno.readTextFile(Deno.args[0]));
  if (
    value.schema !== 1 ||
    value.name !== "concord" ||
    value.version !== "0.2.0-test" ||
    value.keeper !== "concord"
  ) Deno.exit(1);
' "$fixture/unpacked/concord/metadata.json"

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
deno eval '
  const value = JSON.parse(await Deno.readTextFile(Deno.args[0]));
  if (
    value.schema !== 1 ||
    value.version !== "0.2.0-test" ||
    value.releaseVersion !== "0.2.0-test" ||
    value.assets.length !== 3 ||
    Object.keys(value.artifacts).length !== 1 ||
    value.artifacts.skillTarGz.sha256.length !== 64 ||
    !value.artifacts.skillTarGz.url.endsWith(
      "/stable/versions/0.2.0-test/concord-skill.tar.gz",
    )
  ) Deno.exit(1);
' "$fixture/metadata.json"
