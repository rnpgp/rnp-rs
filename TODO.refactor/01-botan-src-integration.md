# TODO.refactor/01-botan-src-integration.md

## Use botan-src crate instead of manual Botan build

**Priority:** P0
**Status:** TODO

## Goal

Replace the 80-line manual Botan download + configure + make pipeline in
build.rs with the `botan-src = "0.31200.0"` crate. Botan's own maintainers
handle build correctness, cross-compilation, and platform support.

## Current state

rnp-src/build.rs manually:
1. Downloads Botan-3.12.0.tar.xz via curl
2. Runs `python3 configure.py --cc=gcc --cc-bin=g++ ...`
3. Runs `make -f ... -j$(nproc)`
4. Runs `make install`

This is ~8 minutes of build time and 80 lines of fragile shell orchestration.

## Target state

rnp-src depends on botan-src:
```toml
[dependencies]
botan-src = "0.31200.0"

[build-dependencies]
botan-src = "0.31200.0"
```

rnp-src/build.rs calls:
```rust
let botan_lib_dir = botan_src::lib_dir();
let botan_include_dir = botan_src::include_dir();
```

No manual download, configure, or make for Botan.

## Considerations

- botan-src v0.31200.0 provides Botan 3.12.0 — matches our librnp 0.18.1 requirement.
- botan-src's build.rs handles macOS clang vs Linux gcc selection.
- For vendored-minimal, we'd need a way to pass module flags to botan-src.
  Currently botan-src builds everything. Options:
  - Accept the larger Botan binary for vendored-minimal (simpler)
  - Fork botan-src to add module filtering (complex)
  - Rebuild Botan manually for the minimal variant (defeats the purpose)

## Tasks

- [ ] Add botan-src dependency to rnp-src/Cargo.toml
- [ ] Remove Botan build logic from rnp-src/build.rs
- [ ] Use botan_src::lib_dir() / include_dir() in cmake PREFIX_PATH
- [ ] Verify librnp builds against botan-src output
- [ ] Decide on vendored-minimal module handling
