# 10 — `vendored` feature: build librnp from source

- **Priority:** P3 — important for downstream adoption but not blocking
  any API work
- **Status:** scaffolded (this session) — the build.rs path is wired but
  the submodule + cmake invocation is not yet active. Falls through to
  the system-link path so consumers who don't enable `vendored` are
  unaffected.
- **Blocked by:** nothing (independent)

## Context

Today the crate links a system-installed `librnp`. That's a fine default but
raises the adoption bar — many potential consumers can't or won't install
librnp separately. A `vendored` Cargo feature that builds librnp from source
(via the `cmake` crate) and statically links `librnp.a` removes that friction.

`Cargo.toml` already declares `vendored = []` as a stub feature; this phase
makes it real.

## Work items

- [ ] Add `rnp` C++ source as a git submodule under `vendor/rnp/`, pinned to
      a released tag (e.g. `v0.18.1` or the latest stable).
- [ ] Initialize `vendor/rnp/src/libsexpp` recursively on the user's behalf
      (librnp bundles sexpp as a submodule).
- [ ] Add `cmake` build-dependency (under `[build-dependencies]`).
- [ ] `build.rs`: when `--features vendored`:
      - Probe for Botan (default backend) and OpenSSL; pick based on
        `RNP_VENDOR_BACKEND=botan|openssl` env var.
      - Run `cmake::Config::new("vendor/rnp")` with appropriate flags
        (`-DCRYPTO_BACKEND=botan`, `-DBUILD_SHARED_LIBS=OFF`,
        `-DBUILD_TESTING=OFF`, `-DENABLE_SM2=ON` if Botan 3, etc.).
      - Link the resulting `librnp.a` (and its transitive deps — Botan,
        JSON-C, zlib, sexpp) into the Rust binary.
      - Set `RNP_INCLUDE_DIR` automatically so the bindgen step uses the
        vendored headers.
- [ ] Document backend selection and required system libraries (Botan,
  JSON-C, zlib) that aren't themselves vendored.
- [ ] Optional follow-up: also vendor Botan via `botan-src` or similar, so
  the build is fully self-contained. Large undertaking; defer.

## Architecture notes

**Why submodule, not tarball:** a submodule pins to a commit SHA and can be
bumped independently. Tarballs couple us to release cadence. Submodules also
let us track `main` for testing against unreleased librnp.

**Why `vendor/rnp/` not `rnp/`:** the cargo convention is `vendor/` for
vendored dependencies. Keeps the crate root clean.

**Backend selection:** Botan is upstream's default and has the broadest
feature coverage (SM2, PQC, crypto-refresh). OpenSSL is the fallback for
environments where Botan isn't available but loses those features. Default
Botan; document the trade-off.

**`build.rs` complexity:** the current `build.rs` is ~110 lines and easy to
read. The vendored path will roughly double it. Keep the two paths clearly
separated (`if vendored { ... } else { ... }`) rather than interleaved. MECE.

**Cross-compilation:** a vendored C++ build is harder to cross-compile than
a Rust-only crate. Document this as a known limitation; don't try to solve
it here.

## Acceptance criteria

- `cargo build --features vendored` on a machine with Botan + JSON-C + zlib
  installed produces a binary that links `librnp.a` statically.
- `cargo test --features vendored` passes the full suite.
- README updated with the `vendored` workflow and the system-library
  prerequisites.
- CI (when set up): one job with system librnp, one job with `--features
  vendored`.

## Completion log

**SCAFFOLDED** in this session.

- `Cargo.toml` declares the `vendored` feature (unchanged from before).
- `build.rs` has a clearly-marked `#[cfg(feature = "vendored")]` block
  that documents the planned cmake invocation and emits a
  `cargo:warning` explaining the feature is currently a stub. Falls
  through to the system-link path so existing consumers are unaffected.

**Remaining work** (still TODO, well-scoped for a follow-up PR):

1. Add `cmake` as a `[build-dependencies]` entry.
2. `git submodule add https://github.com/rnpgp/rnp.git vendor/rnp` pinned
   to a released tag.
3. Initialize `vendor/rnp/src/libsexpp` recursively.
4. Replace the stub block with the real
   `cmake::Config::new("vendor/rnp")...build()` invocation and link the
   resulting `librnp.a` + transitive deps.
5. CI matrix: one job with system librnp, one with `--features vendored`.

The scaffolding landed here so the API surface is complete — the
`vendored` feature now exists, is documented, and gracefully no-ops
until the submodule is added.
