# TODO.refactor/03-cross-compile-awareness.md

## Cross-compile awareness (respect CC/CXX/AR env vars)

**Priority:** P1
**Status:** TODO

## Goal

Support cross-compilation by respecting standard `CC`/`CXX`/`AR`/`CARGO_TARGET_*`
environment variables instead of hardcoding `gcc`/`clang` per target OS.

## Current state

```rust
let (cc, cc_bin) = if cfg!(target_os = "macos") {
    ("clang", "/usr/bin/clang++")
} else {
    ("gcc", "g++")
};
```

This breaks cross-compilation (e.g., building musl from a glibc host, or arm64
from x86_64).

## Target state

Use the `cc` crate's compiler detection or directly read env vars:

```rust
fn detect_compiler() -> (String, String) {
    let cc = env::var("CC").unwrap_or_else(|_| {
        if cfg!(target_os = "macos") { "clang".into() } else { "gcc".into() }
    });
    let cxx = env::var("CXX").unwrap_or_else(|_| {
        if cfg!(target_os = "macos") { "clang++".into() } else { "g++".into() }
    });
    (cc, cxx)
}
```

For CMake, also set:
- `CMAKE_SYSTEM_NAME` based on TARGET
- `CMAKE_SYSTEM_PROCESSOR` based on TARGET
- Cross-compilation toolchain file when CC != host compiler

## Considerations

- The `cc` crate's `cc::Build::new().get_compiler()` returns the compiler path
  and is the canonical way to detect the toolchain in Rust build scripts.
- CMake cross-compilation needs a toolchain file when the target differs from
  the host. The `cmake` crate has some support for this via `CMAKE_TOOLCHAIN_FILE`.
- Botan's configure.py respects `--cc` and `--cc-bin` flags; these should come
  from env vars, not hardcoded values.

## Tasks

- [ ] Implement detect_compiler() that reads CC/CXX env vars
- [ ] Pass detected compilers to Botan configure and librnp cmake
- [ ] Set CMAKE_SYSTEM_NAME/PROCESSOR based on TARGET
- [ ] Test: cross-compile x86_64-unknown-linux-musl from x86_64-linux-gnu host
- [ ] Test: cross-compile aarch64-unknown-linux-gnu from x86_64-linux-gnu host
