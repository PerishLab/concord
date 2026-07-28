#!/bin/sh
set -eu

version=$1
target=$2
root=$(git rev-parse --show-toplevel)
name="concord-$target"
stage="$root/dist/stage/$name"

test -n "$version"
rm -rf "$stage"
mkdir -p "$stage"
cargo build --locked --release --target "$target" -p concord
cp "$root/target/$target/release/concord" "$stage/concord"
cp "$root/README.md" "$root/LICENSE" "$stage/"
mkdir -p "$root/dist"
tar -C "$root/dist/stage" -czf "$root/dist/$name.tar.gz" "$name"
