# 10 — `vendored` feature: build librnp from source

- **Priority:** P3 — important for downstream adoption but not blocking
  any API work
- **Status:** done (this session) — vendored Cargo feature now drives a
  real CMake build of librnp from `vendor/rnp/`, statically links the
  resulting `librnp.a` + transitive deps. All 46 default tests pass under
  `--features vendored`.
- **Blocked by:** nothing (independent)

## Context

Today the crate links a system-installed `librnp`. That's a fine default but
raises the adoption bar — many potential consumers can't or won't install
librnp separately. A `vendored` Cargo feature that builds librnp from source
(via the `cmake` crate) and statically links `librnp.a` removes that friction.

`Cargo.toml` already declares `vendored = []` as a stub feature; this phase
makes it real.

## Work items

- [x] Add `cmake` as a build-dependency.
- [x] Add `rnp` C++ source as a git submodule under `vendor/rnp/`, pinned to
      the released tag `v0.18.1`.
- [x] Initialize `vendor/rnp/src/libsexpp` recursively.
- [x] `build.rs`: when `--features vendored`, run `cmake::Config::new
      ("vendor/rnp")` with `-DCRYPTO_BACKEND=botan` (overridable via
      `RNP_VENDOR_BACKEND`), `-DBUILD_SHARED_LIBS=OFF`,
      `-DBUILD_TESTING=OFF`, `-DENABLE_DOC=OFF`. Propagate `pqc` /
      `crypto-refresh` crate features to librnp's `ENABLE_PQC` /
      `ENABLE_CRYPTO_REFRESH` CMake options. Allow extra CMake args via
      `RNP_VENDOR_CMAKE_ARGS`.
- [x] Statically link `librnp.a` + `libsexpp.a` + transitive deps
      (`botan-3`, `json-c`, `z`, `bz2`, `c++` runtime). Auto-discover
      Homebrew prefixes for the deps at build time.
- [x] Document backend selection and required system libraries in
      `vendor/README.md`.
- [ ] Optional follow-up: also vendor Botan via `botan-src` or similar,
      so the build is fully self-contained. Large undertaking; defer.

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

**DONE** in this session — full CMake-driven vendored build.

- `Cargo.toml` adds `cmake = "0.1"` to `[build-dependencies]`.
- `vendor/rnp/` is a git submodule pinned to upstream tag `v0.18.1`,
  initialized recursively so `src/libsexpp/` comes along.
- `vendor/README.md` documents how to initialize the submodule manually
  (so the repo doesn't pin consumers to a specific librnp version).
- `build.rs` resolves link mode in this priority order:
  1. `--features vendored` → invoke `cmake::Config::new("vendor/rnp")`
     with `-DBUILD_SHARED_LIBS=OFF -DBUILD_TESTING=OFF -DENABLE_DOC=OFF`
     plus `ENABLE_PQC` / `ENABLE_CRYPTO_REFRESH` propagated from the
     crate features. Backend is `botan` by default; override via
     `RNP_VENDOR_BACKEND`. Extra CMake args via
     `RNP_VENDOR_CMAKE_ARGS="KEY=VAL ..."`.
  2. `RNP_INCLUDE_DIR` + `RNP_LIB_DIR` → use that install explicitly.
  3. Else → system search (Homebrew on macOS, /usr on Linux).
- Vendored link pulls in: static `rnp`, static `sexpp`, dynamic `botan-3`
  `json-c` `z` `bz2`, and `c++` (libc++) on macOS / `stdc++` on Linux.
  The Homebrew prefixes for botan/json-c/zlib are auto-detected at build
  time so consumers don't need `DYLD_LIBRARY_PATH`.
- Clear panic message if `vendor/rnp/CMakeLists.txt` is missing — tells
  the user exactly how to initialize the submodule.

**Tested:** `cargo test --features vendored` against the locally-added
`vendor/rnp/` submodule (tag v0.18.1, Botan 3.12 system install). All
46 tests pass — sign/verify, key generation, encryption/decryption,
armor/dump, keyring, security rules. The binary statically links
`librnp.a` + `libsexpp.a` and dynamically links the system Botan.
