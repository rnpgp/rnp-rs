#!/bin/sh
# Regenerate bindings/bindings-<librnp-version>.rs and print it for commit.
#
# Runs a vendored build (headers come from the exact librnp tarball rnp-src
# compiles) with bindgen forced on, then copies the output back into
# bindings/. RNP_BINDINGS_EXPERIMENTAL=1 includes the PQC + crypto-refresh
# surface so one file serves every feature combination.
set -eu

cd "$(dirname "$0")/.."

RNP_BINDINGS_RUNTIME=1 \
RNP_BINDINGS_EXPERIMENTAL=1 \
RNP_BINDINGS_REGENERATE=1 \
cargo build -p rnp-sys --features vendored

echo
echo "Done. Commit the file(s) under bindings/."
