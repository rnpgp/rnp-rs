//! Idiomatic Rust binding to the RNP OpenPGP C library (`librnp`).
//!
//! RNP is the OpenPGP implementation that powers Mozilla Thunderbird. This
//! crate provides a thin, idiomatic Rust wrapper over the public C FFI
//! declared in `<rnp/rnp.h>`.
//!
//! ## Quick start
//!
//! ```no_run
//! use rnp::{Context, KeyIdentifier, signature};
//!
//! let mut ctx = Context::new().unwrap();
//! // ...load keys, sign, verify...
//! ```
//!
//! ## Status
//!
//! This is an early binding. The first target is **sign + verify**, which is
//! what Confium needs for plugin artifact verification. Key generation,
//! encryption, keyring management and armor/dearmor are tracked as TODOs.
//!
//! ## Linking
//!
//! The crate links against a system-installed `librnp` (`-lrnp`). On macOS
//! install via `brew install rnp`; on Linux via your distro's package manager
//! or by building from source (see `~/src/rnp/rnp/`). To point at headers in a
//! non-standard location set `RNP_INCLUDE_DIR`.

pub mod context;
pub mod error;
pub mod ffi;
pub mod key;
pub mod keygen;
pub mod signature;

pub use context::{Context, KeyringFormat, PasswordProvider};
pub use error::{Error, Result};
pub use key::{ExportFlags, Key, KeyIdentifier, LoadSaveFlags};
pub use keygen::generate_test_key;
pub use signature::{sign, sign_detached, verify, verify_detached};

/// librnp version string, e.g. `"0.18.1"`.
pub fn version_string() -> String {
    // SAFETY: rnp_version_string returns a static C string.
    let ptr = unsafe { ffi::rnp_version_string() };
    if ptr.is_null() {
        return String::new();
    }
    unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}
