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
"$tmp/$name/concord" --version
