# rnp-src

Build scripts for compiling [librnp](https://github.com/rnpgp/rnp) and all its
C/C++ dependencies from source.

## What it does

When the `vendored` Cargo feature is enabled on the parent [`rnp-rs`](https://crates.io/crates/rnp-rs)
crate, `rnp-src`'s build script downloads and compiles:

| Dependency | Version | Purpose |
|---|---|---|
| **Botan** | 3.12.0 (via [`botan-src`](https://crates.io/crates/botan-src)) | Cryptographic backend |
| **json-c** | 0.17 | JSON parsing (librnp dependency) |
| **zlib** | 1.3.1 | Compression |
| **bzip2** | 1.0.8 | Compression (with `bz_internal_error` shim) |
| **librnp** | 0.18.1 | OpenPGP implementation (the thing we actually wrap) |

All five are statically linked into the final Rust binary. No system libraries required.

## Usage

You don't use this crate directly. It's pulled in automatically when you enable
the `vendored` feature on `rnp-rs`:

```toml
[dependencies]
rnp = { version = "0.1", features = ["vendored"] }
```

## Cargo features

| Feature | Description |
|---|---|
| `pqc` | Builds librnp with `ENABLE_PQC=ON` and Botan with PQC modules (ML-KEM, ML-DSA, SLH-DSA). Clones librnp HEAD instead of 0.18.1 (PQC code paths in 0.18.1 are incompatible with Botan 3.12's opaque EC types). |
| `crypto-refresh` | Builds librnp with `ENABLE_CRYPTO_REFRESH=ON`. Same HEAD-vs-0.18.1 reasoning as `pqc`. |

## Platform support

| Platform | Status | Toolchain |
|---|---|---|
| Linux x86_64 | Tested in CI | gcc/g++ |
| Linux arm64 | Tested in CI | gcc/g++ |
| macOS Intel | Tested in CI | clang/clang++ |
| macOS ARM (Apple Silicon) | Tested in CI | clang/clang++ |
| Windows x86_64 | Tested in CI (MSYS2 UCRT64) | mingw-w64 gcc/g++ |

## Build requirements

- C/C++ compiler (gcc, clang, or mingw-w64 gcc)
- `cmake` ≥ 3.5
- `python3` (for Botan's `configure.py`)
- First build takes ~5-10 min (Botan dominates); cached in `OUT_DIR` after that

## License

BSD-2-Clause, matching [RNP](https://github.com/rnpgp/rnp).
