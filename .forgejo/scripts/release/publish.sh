#!/usr/bin/env bash
set -euo pipefail

: "${CONCORD_RELEASES_PUBLIC_URL:?}"
: "${CONCORD_RELEASES_S3_AK:?}"
: "${CONCORD_RELEASES_S3_BUCKET:?}"
: "${CONCORD_RELEASES_S3_SK:?}"
: "${CONCORD_RELEASES_S3_URL:?}"
: "${RELEASE_CHANNEL:?}"
: "${RELEASE_VERSION:?}"

export AWS_ACCESS_KEY_ID="$CONCORD_RELEASES_S3_AK"
export AWS_SECRET_ACCESS_KEY="$CONCORD_RELEASES_S3_SK"
export AWS_DEFAULT_REGION=auto
export AWS_EC2_METADATA_DISABLED=true
prefix="$RELEASE_CHANNEL/versions/$RELEASE_VERSION"
endpoint="${CONCORD_RELEASES_S3_URL%/}"

held=$(aws --endpoint-url "$endpoint" s3api list-objects-v2 \
  --bucket "$CONCORD_RELEASES_S3_BUCKET" \
  --prefix "$prefix/" \
  --max-items 1 \
  --query KeyCount \
  --output text \
  --no-cli-pager)
if [[ "$held" != "0" ]]; then
  echo "immutable release already exists: $prefix" >&2
  exit 1
fi

export RELEASE_ROOT=dist
export CI_COMMIT="${GITHUB_SHA:-}"
deno run --allow-env --allow-read=dist --allow-write=dist \
  .forgejo/scripts/release/metadata.ts

for file in dist/concord-*; do
  name=${file##*/}
  aws --endpoint-url "$endpoint" s3 cp "$file" \
    "s3://$CONCORD_RELEASES_S3_BUCKET/$prefix/$name" \
    --cache-control "public,max-age=31536000,immutable" \
    --no-cli-pager
done
aws --endpoint-url "$endpoint" s3 cp dist/metadata.json \
  "s3://$CONCORD_RELEASES_S3_BUCKET/$prefix/metadata.json" \
  --content-type application/json \
  --cache-control "public,max-age=31536000,immutable" \
  --no-cli-pager
aws --endpoint-url "$endpoint" s3 cp dist/metadata.json \
  "s3://$CONCORD_RELEASES_S3_BUCKET/$RELEASE_CHANNEL/latest/metadata.json" \
  --content-type application/json \
  --cache-control "no-store" \
  --no-cli-pager
