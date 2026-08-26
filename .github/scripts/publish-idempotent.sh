#!/bin/sh
# Publish a workspace crate to crates.io, skipping versions already
# published (idempotent). Designed for trusted publishing (OIDC) — no
# token argument; -v surfaces the crates.io exchange error; cargo acquires a short-lived token via the GitHub
# Actions environment when the crate is configured as a trusted
# publisher on crates.io.
set -eu

CRATE="$1"
LOCAL_VERSION="$(cargo metadata --no-deps --format-version 1 \
  | jq -r ".packages[] | select(.name == \"$CRATE\") | .version")"

PUBLISHED="$(curl -sf -H "User-Agent: rnp-rs-publish-check" \
  "https://crates.io/api/v1/crates/$CRATE" \
  | jq -r '.versions[].num' || true)"

if printf '%s\n' "$PUBLISHED" | grep -qx "$LOCAL_VERSION"; then
  echo "$CRATE@$LOCAL_VERSION already on crates.io — skipping."
  exit 0
fi

echo "Publishing $CRATE@$LOCAL_VERSION via trusted publishing..."
DIR="$(cargo metadata --no-deps --format-version 1 \
  | jq -r ".packages[] | select(.name == \"$CRATE\") | .manifest_path" \
  | xargs dirname)"
cd "$DIR"
exec cargo publish -v
