#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname "$0")/../../.." && pwd)
binary="$root/target/debug/concord"
test -x "$binary"
version=$("$binary" --version | awk '{print $2}')
test -n "$version"

case "$(uname -s):$(uname -m)" in
  Linux:x86_64|Linux:amd64) archive=concord-x86_64-unknown-linux-gnu.tar.gz ;;
  Darwin:arm64|Darwin:aarch64) archive=concord-aarch64-apple-darwin.tar.gz ;;
  *) echo "unsupported manager smoke platform" >&2; exit 1 ;;
esac
name=${archive%.tar.gz}

fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT INT TERM
release="$fixture/public/stable/versions/$version"
mkdir -p "$fixture/stage/$name" "$release"
cp "$binary" "$fixture/stage/$name/concord"
tar -C "$fixture/stage" -czf "$release/$archive" "$name"
if command -v sha256sum >/dev/null 2>&1; then
  digest=$(sha256sum "$release/$archive" | awk '{print $1}')
else
  digest=$(shasum -a 256 "$release/$archive" | awk '{print $1}')
fi
cat > "$release/metadata.json" <<EOF
{
  "releaseVersion": "$version",
  "assets": [
    {
      "name": "$archive",
      "sha256": "$digest"
    }
  ]
}
EOF

beta="$version-beta.1"
beta_release="$fixture/public/beta/versions/$beta"
mkdir -p "$beta_release"
cp "$release/$archive" "$beta_release/$archive"
cat > "$beta_release/metadata.json" <<EOF
{
  "releaseVersion": "$beta",
  "assets": [
    {
      "name": "$archive",
      "sha256": "$digest"
    }
  ]
}
EOF

base="file://$fixture/public"
"$root/manage.sh" install --version "$version" --public-url "$base" \
  --install-root "$fixture/install" --bin-dir "$fixture/bin"
test -L "$fixture/bin/concord"
test "$("$fixture/bin/concord" --version)" = "concord $version"
"$root/manage.sh" update --version "v$version" --public-url "$base" \
  --install-root "$fixture/install" --bin-dir "$fixture/bin"
"$root/manage.sh" uninstall --install-root "$fixture/install" \
  --bin-dir "$fixture/bin"
test ! -e "$fixture/bin/concord"

"$root/manage.sh" install --channel beta --version "$beta" --public-url "$base" \
  --install-root "$fixture/beta-install" --bin-dir "$fixture/beta-bin"
test -x "$fixture/beta-install/v$beta/concord"
test "$("$fixture/beta-bin/concord" --version)" = "concord $version"
"$root/manage.sh" uninstall --install-root "$fixture/beta-install" \
  --bin-dir "$fixture/beta-bin"

mkdir -p "$fixture/legacy-bin"
cp "$binary" "$fixture/legacy-bin/concord"
"$root/manage.sh" install --version "$version" --public-url "$base" \
  --install-root "$fixture/legacy-install" --bin-dir "$fixture/legacy-bin"
test -L "$fixture/legacy-bin/concord"
"$root/manage.sh" uninstall --install-root "$fixture/legacy-install" \
  --bin-dir "$fixture/legacy-bin"

mkdir -p "$fixture/refusal-bin"
printf 'unowned\n' > "$fixture/refusal-bin/concord"
if "$root/manage.sh" install --version "$version" --public-url "$base" \
  --install-root "$fixture/refusal-install" --bin-dir "$fixture/refusal-bin"
then
  echo "manager accepted an unowned entrypoint" >&2
  exit 1
fi
test -f "$fixture/refusal-bin/concord"
test ! -e "$fixture/refusal-install"
