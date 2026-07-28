#!/usr/bin/env bash
set -euo pipefail

for name in CONCORD_RELEASES_S3_AK CONCORD_RELEASES_S3_SK CONCORD_RELEASES_S3_BUCKET CONCORD_RELEASES_S3_URL RELEASE_CHANNEL RELEASE_VERSION; do
  test -n "${!name:-}" || { echo "$name is required" >&2; exit 1; }
done

export AWS_ACCESS_KEY_ID="$CONCORD_RELEASES_S3_AK"
export AWS_SECRET_ACCESS_KEY="$CONCORD_RELEASES_S3_SK"
export AWS_DEFAULT_REGION=auto
export AWS_EC2_METADATA_DISABLED=true
prefix="$RELEASE_CHANNEL/versions/$RELEASE_VERSION"
endpoint="${CONCORD_RELEASES_S3_URL%/}"

for name in \
  concord-x86_64-unknown-linux-gnu.tar.gz \
  concord-aarch64-apple-darwin.tar.gz \
  concord-x86_64-pc-windows-msvc.zip \
  concord-skill.tar.gz \
  metadata.json; do
  if held=$(aws --endpoint-url "$endpoint" s3api head-object \
    --bucket "$CONCORD_RELEASES_S3_BUCKET" \
    --key "$prefix/$name" \
    --no-cli-pager 2>&1); then
    echo "immutable object already exists: $prefix/$name" >&2
    exit 1
  fi
  case "$held" in
    *404*|*NotFound*|*"Not Found"*) ;;
    *) echo "cannot verify absence for $prefix/$name: $held" >&2; exit 1 ;;
  esac
done
