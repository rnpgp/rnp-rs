//! rnp-src: compile librnp and all dependencies from source.
//!
//! Consumed by `rnp-rs` when the `vendored` Cargo feature is enabled.
//! The build script (`../build.rs`) downloads and compiles:
//!
//! - librnp 0.18.1 (OpenPGP implementation)
//! - json-c 0.17 (JSON parsing, required by librnp)
//! - zlib 1.3.1 (compression)
//! - bzip2 1.0.8 (compression, with bz_internal_error fix)
//!
//! Botan is provided by the [`botan-src`] crate dependency.
//!
//! Paths are communicated to `rnp-rs`'s build.rs via Cargo's `links`
//! mechanism (DEP_RNP_LIB_DIR, DEP_RNP_INCLUDE_DIR, DEP_RNP_<NAME>_LIB_DIR).
//!
//! Pure-logic types and constants live in [`links`] so they can be shared
//! with the build script (via `#[path]`) and unit-tested here without
//! invoking the C/C++ toolchain.

pub mod links;

pub use links::{CmakeDep, Deps, JSON_C, ZLIB, lib_dir_env_var};

/// librnp version this crate compiles.
pub const RNP_VERSION: &str = "0.18.1";
