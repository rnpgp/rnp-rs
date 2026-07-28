//! rnp-src: compile librnp and all dependencies from source.
//!
//! This crate is consumed by `rnp-rs` when the `vendored` Cargo feature is
//! enabled. Its `build.rs` downloads and compiles:
//!
//! - librnp 0.18.1 (OpenPGP implementation)
//! - json-c 0.17 (JSON parsing, required by librnp)
//! - zlib 1.3.1 (compression)
//! - bzip2 1.0.8 (compression)
//!
//! Botan is provided by the [`botan-src`] crate dependency.
//!
//! After compilation, the install paths are available via [`lib_dir`] and
//! [`include_dir`].

use std::path::PathBuf;

/// Directory containing the compiled static libraries (librnp.a, etc.).
pub fn lib_dir() -> PathBuf {
    PathBuf::from(env!("RNP_SRC_LIB_DIR"))
}

/// Directory containing the librnp headers (rnp/*.h).
pub fn include_dir() -> PathBuf {
    PathBuf::from(env!("RNP_SRC_INCLUDE_DIR"))
}

/// Directory containing the compiled Botan static library (libbotan-3.a).
pub fn botan_lib_dir() -> PathBuf {
    PathBuf::from(env!("RNP_SRC_BOTAN_LIB_DIR"))
}
