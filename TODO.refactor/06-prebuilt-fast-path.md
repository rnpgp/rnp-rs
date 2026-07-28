# TODO.refactor/06-prebuilt-fast-path.md

## Optional prebuilt fast path via GitHub Releases

**Priority:** P1
**Status:** TODO (blocked on upstream librnp providing prebuilts — see rnpgp/rnp#2443)

## Goal

Offer an optional fast path where consumers download prebuilt static
libraries from GitHub Releases instead of compiling from source (~10 sec
vs ~10 min). Source compilation remains the fallback.

## Design

```
cargo build --features vendored
  → rnp-src build.rs:
    1. Check RNP_PREBUILT_URL env var (or default GitHub Releases URL)
    2. Download per-target tarball if available
    3. Fall back to compile-from-source if download fails or is disabled
```

## Release asset structure

```
rnp-v{version}-{target}.tar.gz    (full Botan)
rnp-v{version}-{target}-minimal.tar.gz  (minimal Botan)
```

Each contains: librnp.a, libsexpp.a, libjson-c.a, libbotan-3.a, libz.a,
libbz2.a, include/rnp/*.h.

## Integrity

- Per-asset SHA256 checksums in a `SHA256SUMS` file
- build.rs verifies checksum after download
- Fail closed on mismatch (never link untrusted binaries)

## Considerations

- Source compilation must remain the default — prebuilts are an optimization.
- Users opt in via `RNP_PREBUILT=1` env var or a `prebuilt` Cargo feature.
- Prebuilts must be built with correct flags (macOS 11 target, bz2 fix, etc.)
- Windows prebuilts: .lib instead of .a; needs separate handling.

## Tasks

- [ ] Create GitHub Release with per-target tarballs
- [ ] Add download + checksum verification to rnp-src build.rs
- [ ] Add `prebuilt` Cargo feature to rnp-rs
- [ ] Document the fast path in README
- [ ] Add CI smoke test: link hello-world against downloaded prebuilts
