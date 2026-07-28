# TODO.refactor/02-hermetic-downloads.md

## Hermetic build-time downloads (ureq + tar + flate2 instead of curl/tar)

**Priority:** P1
**Status:** TODO

## Goal

Replace shell-out to `curl` and `tar` in build.rs with in-process Rust crates.
Eliminates CLI dependency, works in locked-down CI, no shell injection surface.

## Current state

```rust
Command::new("curl").args(["-sL", "-o"]).arg(&tarball).arg(url)
Command::new("tar").args(["xzf"]).arg(&tarball).arg("-C").arg(dest)
```

## Target state

```rust
// build-dependencies in rnp-src Cargo.toml
ureq = { version = "2", features = ["tls"] }
flate2 = "1"
tar = "0.4"

// In build.rs
let response = ureq::get(url).call()?;
let gz = flate2::read::GzDecoder::new(response.into_reader());
tar::Archive::new(gz).unpack(dest)?;
```

## Considerations

- `ureq` with rustls (no OpenSSL dependency for the downloader itself)
- `webpki-roots` for CA bundle (no system cert store dependency)
- Cache: OUT_DIR is already used; ureq adds ~500 KB to build-deps
- Error handling: surface download/extract failures as clear panic messages
- Retry: ureq supports retries; useful for flaky CI networks

## Tasks

- [ ] Add ureq, flate2, tar as build-dependencies in rnp-src
- [ ] Replace all Command::new("curl") / Command::new("tar") calls
- [ ] Add retry logic for downloads (3 attempts, 5s backoff)
- [ ] Test in CI without curl/tar on PATH
