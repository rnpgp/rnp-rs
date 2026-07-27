# vendor/

When the `vendored` Cargo feature is enabled, `build.rs` performs a
**hermetic static build**: it downloads pinned release tarballs of librnp,
Botan 3 and json-c, verifies their SHA-256 hashes, builds them all from
source, and statically links everything into the consumer — `librnp.a`,
`libsexpp.a`, `libbotan-3.a`, `libjson-c.a`, plus zlib and bzip2. No system
crypto libraries are used or referenced; the resulting binary has no
runtime dependency on Botan, json-c or librnp.

## Pinned sources

| Package | Version | Source | SHA-256 |
|---------|---------|--------|---------|
| librnp  | 0.18.1  | `rnp-v0.18.1.tar.gz` (official release dist tarball, bundles libsexpp) | `423c8e32…f18ecfd` |
| Botan   | 3.12.0  | `botan-3.12.0.tar.gz` (GitHub archive of the signed tag) | `cf152f47…ab7995` |
| json-c  | 0.18    | `json-c-0.18.tar.gz` (official release tarball) | `876ab046…b11724` |

The full hashes and URLs live in the `VENDORS` table in
[`build.rs`](../build.rs). Botan is configured with
`--minimized-build --enable-modules=<…>` using the module set librnp needs
(derived from rnp upstream's `ci/botan3-modules`, adjusted for Botan 3.8+
module renames), and everything is built with position-independent code so
the archives link into PIE executables.

Build-time downloads are exactly that — a build-time mechanism, not a
runtime dependency (the same pattern as `openssl-src`/`curl-sys`).

## Build-time requirements

Only tools, no libraries (except on Linux, see below):

- C and C++ compilers, `make`
- `cmake` (≥ 3.18)
- `python3` (Botan's `configure.py`)
- `tar`
- `curl` — skipped when `RNP_VENDOR_DIR` is set (see below)
- Linux: `zlib1g-dev` and `libbz2-dev` (for the static `libz.a` /
  `libbz2.a`; zlib and bzip2 are compression libraries, not crypto).
  On macOS the OS-provided libz/libbz2 are used; Apple ships no static
  variants, so those two remain OS dylib references — part of the base
  system, always present.

Debian/Ubuntu minimal package set:

```sh
sudo apt-get install build-essential cmake curl python3 zlib1g-dev libbz2-dev libclang-dev
```

(`libclang-dev` is needed by bindgen, as for every rnp-rs build mode.)

## Offline / air-gapped builds

Set `RNP_VENDOR_DIR` to a directory containing the three pinned tarballs
under their exact file names:

```
rnp-v0.18.1.tar.gz
botan-3.12.0.tar.gz
json-c-0.18.tar.gz
```

They are verified against the same pinned SHA-256 hashes, so a corrupt or
substituted file aborts the build.

## Usage

```sh
cargo build --features vendored
```

```toml
[dependencies]
rnp-rs = { version = "0.2", features = ["vendored"] }
```

The first build downloads ~19 MB of tarballs and compiles all three C
libraries (a few minutes); artifacts are cached under `target/` and reused
until the recipe, feature set (`pqc` / `crypto-refresh`) or target changes.

`RNP_VENDOR_CMAKE_ARGS="KEY=VALUE …"` still passes extra defines through to
the librnp CMake configure. `RNP_VENDOR_BACKEND` is gone — the hermetic
build always uses the pinned Botan 3 backend.

## Notes

- The `vendor/rnp/` git submodule is **not** used by this mechanism; it
  remains in the repository for maintainer workflows only.
- Cross-compilation is not supported by the vendored feature in this
  release — build librnp externally and use `RNP_INCLUDE_DIR` /
  `RNP_LIB_DIR` instead.
- `pqc` / `crypto-refresh` on top of `vendored` is best-effort: the pinned
  librnp is 0.18.1, whose PQC support targets the Botan 3.7-era API. The
  Botan module extras follow rnp upstream's `ci/botan3-pqc-modules`. For
  production PQC work, build current librnp HEAD with a matching Botan and
  use `RNP_INCLUDE_DIR` / `RNP_LIB_DIR` (see CLAUDE.md).
- Binary-size cost (measured on macOS arm64, release profile): the static
  archives are `libbotan-3.a` 5.6 MB (minimized modules), `librnp.a`
  2.1 MB, `libsexpp.a` 91 KB, `libjson-c.a` 89 KB; after dead-code
  stripping, the `sign_verify` example — keygen + sign + verify, i.e. a
  realistic hello-world — is a **3.2 MB** binary with zero non-OS dynamic
  references.
