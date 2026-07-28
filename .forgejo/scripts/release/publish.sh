#!/usr/bin/env bash
set -euo pipefail

for name in CONCORD_RELEASES_PUBLIC_URL CONCORD_RELEASES_S3_AK CONCORD_RELEASES_S3_BUCKET CONCORD_RELEASES_S3_SK CONCORD_RELEASES_S3_URL RELEASE_CHANNEL RELEASE_VERSION; do
  test -n "${!name:-}" || { echo "$name is required" >&2; exit 1; }
done

export AWS_ACCESS_KEY_ID="$CONCORD_RELEASES_S3_AK"
export AWS_SECRET_ACCESS_KEY="$CONCORD_RELEASES_S3_SK"
export AWS_DEFAULT_REGION=auto
export AWS_EC2_METADATA_DISABLED=true
prefix="$RELEASE_CHANNEL/versions/$RELEASE_VERSION"
endpoint="${CONCORD_RELEASES_S3_URL%/}"
mkdir -p dist

for name in \
  concord-x86_64-unknown-linux-gnu.tar.gz \
  concord-aarch64-apple-darwin.tar.gz \
  concord-x86_64-pc-windows-msvc.zip; do
  aws --endpoint-url "$endpoint" s3api get-object \
    --bucket "$CONCORD_RELEASES_S3_BUCKET" \
    --key "$prefix/$name" \
    --no-cli-pager \
    "dist/$name" >/dev/null
done

tar -tzf dist/concord-x86_64-unknown-linux-gnu.tar.gz >/dev/null
tar -tzf dist/concord-aarch64-apple-darwin.tar.gz >/dev/null
unzip -tq dist/concord-x86_64-pc-windows-msvc.zip >/dev/null
sh .forgejo/scripts/release/skill.sh "$RELEASE_VERSION" dist
tar -tzf dist/concord-skill.tar.gz >/dev/null
export RELEASE_ROOT=dist
export CI_COMMIT="${GITHUB_SHA:-}"
deno run --allow-env --allow-read=dist --allow-write=dist \
  .forgejo/scripts/release/metadata.ts

aws --endpoint-url "$endpoint" s3 cp dist/concord-skill.tar.gz \
  "s3://$CONCORD_RELEASES_S3_BUCKET/$prefix/concord-skill.tar.gz" \
  --content-type application/gzip \
  --cache-control "public,max-age=31536000,immutable" \
  --no-cli-pager
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
aws --endpoint-url "$endpoint" s3 cp manage.sh \
  "s3://$CONCORD_RELEASES_S3_BUCKET/manage.sh" \
  --content-type text/x-shellscript \
  --cache-control "public,max-age=60,must-revalidate" \
  --no-cli-pager
aws --endpoint-url "$endpoint" s3 cp manage.ps1 \
  "s3://$CONCORD_RELEASES_S3_BUCKET/manage.ps1" \
  --content-type text/plain \
  --cache-control "public,max-age=60,must-revalidate" \
  --no-cli-pager
