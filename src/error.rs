//! Error type for the RNP binding.
//!
//! Maps `rnp_result_t` (a `u32`) to a structured `snafu` error. `RNP_SUCCESS`
//! is `0`; every other value is a failure with a category derived from the
//! high nibble of the code (see `rnp_err.h`).

use snafu::Snafu;

// A subset of the `RNP_*` result codes from `rnp_err.h`. bindgen does not pick
// these up because they live in an anonymous C enum (the constants emitted by
// bindgen are only the `#define`-based flags). They are part of librnp's
// stable ABI and will not change without a major version bump.
//
// Source: include/rnp/rnp_err.h
mod codes {
    pub const RNP_SUCCESS: u32 = 0x0000_0000;
    // RNP_ERROR_GENERIC = 0x10000000, auto-incremented 8 times.
    pub const RNP_ERROR_NOT_FOUND: u32 = 0x1000_0008;
}

/// A failure returned by the underlying librnp call.
#[derive(Debug, Snafu)]
pub enum Error {
    /// The RNP call returned a non-success result code.
    #[snafu(display("rnp call failed: {message} (code 0x{code:08x})"))]
    Rnp {
        code: u32,
        message: String,
    },

    /// A signature was processed but did not verify.
    #[snafu(display("signature did not verify"))]
    SignatureInvalid,

    /// A required pointer/handle was unexpectedly null.
    #[snafu(display("librnp returned a null pointer where one was required"))]
    NullPointer,

    /// A NUL byte was found in a string that must be passed to librnp as a
    /// C string.
    #[snafu(display("string contained an interior NUL byte"))]
    NulByte,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Re-export of `RNP_SUCCESS` for other modules in this crate.
pub(crate) const SUCCESS: u32 = codes::RNP_SUCCESS;

/// Re-export of `RNP_ERROR_NOT_FOUND`.
pub(crate) const NOT_FOUND: u32 = codes::RNP_ERROR_NOT_FOUND;

/// Check an `rnp_result_t` return value; convert non-success into an
/// [`Error::Rnp`].
///
/// Implemented as a free function rather than a method on a wrapper because
/// librnp returns the result code by value rather than through an out-handle.
pub(crate) fn check(code: u32) -> Result<()> {
    if code == codes::RNP_SUCCESS {
        Ok(())
    } else {
        let message = result_to_string(code);
        Err(Error::Rnp { code, message })
    }
}

fn result_to_string(code: u32) -> String {
    // SAFETY: rnp_result_to_string returns a static C string (or NULL on a
    // truly bad input). We never free it.
    let ptr = unsafe { crate::ffi::rnp_result_to_string(code) };
    if ptr.is_null() {
        return format!("unknown rnp error 0x{code:08x}");
    }
    // SAFETY: librnp returns a NUL-terminated string.
    let cstr = unsafe { std::ffi::CStr::from_ptr(ptr) };
    cstr.to_string_lossy().into_owned()
}
