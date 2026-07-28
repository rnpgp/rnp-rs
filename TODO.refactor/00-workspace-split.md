# TODO.refactor/00-workspace-split.md

## Split rnp-rs into workspace with rnp-src crate

**Priority:** P0
**Status:** IN PROGRESS

## Goal

Split the monolithic rnp-rs crate into a Cargo workspace with two members:

- `rnp-src` — downloads + compiles librnp and all C/C++ dependencies from source
- `rnp-rs` — Rust safe API + bindgen FFI bindings

## Design

```
rnp-rs/                          (workspace root)
├── Cargo.toml                   (workspace + rnp-rs package)
├── rnp-src/
│   ├── Cargo.toml               (rnp-src package)
│   ├── build.rs                 (C/C++ compilation orchestration)
│   └── src/
│       └── lib.rs               (public API: lib_dir(), include_dir())
├── src/                         (rnp-rs crate source)
├── build.rs                     (rnp-rs build.rs — calls rnp-src when vendored)
└── TODO.refactor/
```

## Dependency chain

```
rnp-rs  (safe API + bindgen)
  └─[vendored feature]→ rnp-src  (compiles librnp + deps)
       └─→ botan-src    (compiles Botan, existing crate)
```

## API

```rust
// rnp-src/src/lib.rs
pub fn lib_dir() -> PathBuf;      // where librnp.a lives
pub fn include_dir() -> PathBuf;  // where rnp/*.h live
pub fn botan_lib_dir() -> PathBuf; // where libbotan-3.a lives
```

## Tasks

- [x] Create workspace Cargo.toml
- [x] Create rnp-src/Cargo.toml
- [x] Create rnp-src/src/lib.rs
- [x] Create rnp-src/build.rs (moved from rnp-rs/build.rs)
- [x] Refactor rnp-rs/build.rs to call rnp-src
- [x] Update rnp-rs/Cargo.toml to depend on rnp-src
- [ ] Test: cargo build --features vendored compiles from source
- [ ] Publish rnp-src v0.1.0 to crates.io
- [ ] Publish rnp-rs v0.1.10 to crates.io
