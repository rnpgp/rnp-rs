# TODO.refactor/08-ci-smoke-test.md

## CI per-target static link smoke test

**Priority:** P2
**Status:** TODO

## Goal

A CI job that links a hello-world binary against the produced static
libraries, per target. Would catch: external botan linkage, macOS SDK
mismatch, bz2 undefined symbol, missing PQC modules — all before any
consumer hits them.

## Design

After the vendored build succeeds in CI, link a minimal C program:

```c
// smoke.c
#include <rnp/rnp.h>
int main() {
    uint32_t ver = rnp_version();
    return ver > 0 ? 0 : 1;
}
```

```sh
cc smoke.c -I$INCLUDE_DIR -L$LIB_DIR -lrnp -lbotan-3 -ljson-c -lz -lbz2 -lstdc++
./a.out
```

If any symbol is undefined, the linker fails immediately.

## Considerations

- Run per target (gnu, musl, macOS x86_64, macOS arm64).
- Also test PQC variant: link with `-DRNP_EXPERIMENTAL_PQC` and call
  `rnp_supports_feature(RNP_FEATURE_PK_ALG, "ML-KEM-768")`.
- Cache: only run when build.rs changes, not on every PR.

## Tasks

- [ ] Add smoke.c test program
- [ ] Add CI job step after vendored build
- [ ] Test: each target links + runs
- [ ] Test: PQC symbols present when feature enabled
