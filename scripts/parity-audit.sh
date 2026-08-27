#!/usr/bin/env bash
# FFI surface-parity audit: compare the rnp.h function surface (as bound by
# rnp-sys) against call sites in the safe crate's src/.
#
# Reports, for the vendored librnp version:
#   - functions declared in rnp.h
#   - functions bound by rnp-sys
#   - functions exercised by a safe wrapper call site (ffi::name(...))
#   - declared-but-not-bound or bound-but-not-exercised gaps (expected: none)
#
# The same audit is enforced by tests/ffi_parity.rs in CI; this script is
# the human-readable form. Exit code is non-zero on any gap.
set -euo pipefail
cd "$(dirname "$0")/.."

header="vendor/rnp/include/rnp/rnp.h"
bindings=$(ls rnp-sys/bindings/bindings-*.rs | head -1)

if [[ ! -f "$header" ]]; then
  echo "error: $header not found (git submodule not initialized?)" >&2
  exit 1
fi
if [[ ! -f "$bindings" ]]; then
  echo "error: no rnp-sys/bindings/bindings-*.rs found" >&2
  exit 1
fi

# Function names declared in the C header (typedefs like rnp_input_reader_t
# end in _t and are filtered out).
grep -oE '\brnp_[a-z0-9_]+[[:space:]]*\(' "$header" \
  | sed -E 's/[[:space:]]*\($//' | grep -v '_t$' | sort -u > /tmp/parity-header.txt

# Functions bound by rnp-sys.
grep -oE 'pub fn rnp_[a-z0-9_]+' "$bindings" \
  | sed 's/pub fn //' | sort -u > /tmp/parity-bound.txt

# Bound functions with at least one safe call site.
> /tmp/parity-exercised.txt
while read -r fn; do
  grep -rqE "ffi::${fn}\(" src/ && echo "$fn" >> /tmp/parity-exercised.txt
done < /tmp/parity-bound.txt
sort -u /tmp/parity-exercised.txt -o /tmp/parity-exercised.txt

declared=$(wc -l < /tmp/parity-header.txt | tr -d ' ')
bound=$(wc -l < /tmp/parity-bound.txt | tr -d ' ')
exercised=$(wc -l < /tmp/parity-exercised.txt | tr -d ' ')

echo "declared in rnp.h : $declared"
echo "bound by rnp-sys   : $bound"
echo "exercised in src/  : $exercised"

status=0
not_bound=$(comm -23 /tmp/parity-header.txt /tmp/parity-bound.txt)
if [[ -n "$not_bound" ]]; then
  echo "declared but not bound:"; echo "$not_bound" | sed 's/^/  /'
  status=1
fi
# Functions consciously excluded in the enforcement test count as covered.
mapfile -t excluded < <(grep -oE '^\s*"[a-z0-9_]+",' tests/ffi_parity.rs | tr -d ' ",' | sort -u)
excluded_re=$(IFS='|'; echo "${excluded[*]}")

not_exercised=$(comm -23 /tmp/parity-bound.txt /tmp/parity-exercised.txt)
unaccounted=$(echo "$not_exercised" | grep -Ev "^($excluded_re)$" || true)
if [[ -n "$not_exercised" ]]; then
  echo "bound but not exercised (excluded in tests/ffi_parity.rs):"
  echo "$not_exercised" | sed 's/^/  /'
fi
if [[ -n "$unaccounted" ]]; then
  echo "unaccounted gaps:"
  echo "$unaccounted" | sed 's/^/  /'
  status=1
fi
if [[ $status -eq 0 ]]; then
  echo "parity: complete"
fi
exit $status
