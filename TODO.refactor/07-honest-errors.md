# TODO.refactor/07-honest-errors.md

## Honest error messages

**Priority:** P1
**Status:** TODO

## Goal

When a build fails or a feature is unavailable, the error message must say
exactly what's wrong, what does exist, and what the user should do. Never
again "init the submodule" for a crates.io consumer.

## Current problems

1. build.rs panic messages reference `vendor/rnp/CMakeLists.txt` which
   doesn't exist in the published crate.
2. No diagnostic for missing build tools (cmake, python3, curl).
3. Link failures don't explain which symbol is missing from which lib.

## Target state

### Missing build tools
```
rnp-rs vendored: cmake not found on PATH.
  Install: brew install cmake (macOS) / apt install cmake (Linux)
  Or use the default mode (no --features vendored) which links system librnp.
```

### Missing source download
```
rnp-rs vendored: failed to download librnp 0.18.1 source from
  https://github.com/rnpgp/rnp/releases/download/v0.18.1/rnp-v0.18.1.tar.gz
  HTTP 404. The release may have been yanked.
  Check https://github.com/rnpgp/rnp/releases for available versions.
```

### Platform not supported
```
rnp-rs vendored: target `x86_64-pc-windows-gnu` is not supported by the
  compile-from-source path. Supported targets:
    - x86_64-unknown-linux-gnu    - aarch64-unknown-linux-gnu
    - x86_64-apple-darwin         - aarch64-apple-darwin
  For Windows, use MSVC target (x86_64-pc-windows-msvc).
```

## Implementation

Create a helper module in rnp-src:
```rust
mod error;
pub use error::BuildError;

// Usage:
return Err(BuildError::missing_tool("cmake"));
return Err(BuildError::download_failed(url, status));
return Err(BuildError::unsupported_target(target));
```

Each error type produces a user-friendly message with actionable guidance.

## Tasks

- [ ] Create error types for missing tools, download failures, unsupported targets
- [ ] Add pre-flight checks at the start of build.rs (cmake, python3, curl)
- [ ] Replace all panic!() calls with structured errors
- [ ] Test: trigger each error path and verify message quality
