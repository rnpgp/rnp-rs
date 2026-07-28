//! [`Input`] — RAII wrapper around `rnp_input_t`.

use crate::error::{self, Result, check};
use crate::ffi;
use std::ffi::CString;
use std::ptr;

/// Owns an `rnp_input_t` for the lifetime of this value.
///
/// Construct with [`Input::from_memory`], [`Input::from_path`], or
/// [`Input::from_stdin`]. Pass to higher-level operations via
/// [`Input::as_ptr`].
pub struct Input {
    handle: ffi::rnp_input_t,
}

impl Input {
    /// Wrap memory as an input. The bytes are copied on the C side, so the
    /// caller's slice does not need to outlive the [`Input`].
    pub fn from_memory(bytes: &[u8]) -> Result<Self> {
        let mut handle: ffi::rnp_input_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_input_from_memory(
                &mut handle,
                bytes.as_ptr(),
                bytes.len(),
                true, // copy
            ))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(Input { handle })
    }

    /// Open a file at `path` for reading.
    pub fn from_path(path: &str) -> Result<Self> {
        let c = CString::new(path).map_err(|_| error::Error::PathNul)?;
        let mut handle: ffi::rnp_input_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_input_from_path(&mut handle, c.as_ptr()))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(Input { handle })
    }

    /// Read from process stdin.
    pub fn from_stdin() -> Result<Self> {
        let mut handle: ffi::rnp_input_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_input_from_stdin(&mut handle))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(Input { handle })
    }

    /// Raw handle for passing to librnp functions. Crate-internal.
    pub(crate) fn as_ptr(&self) -> ffi::rnp_input_t {
        self.handle
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: handle was produced by rnp_input_from_* and not yet
            // destroyed.
            unsafe {
                let _ = ffi::rnp_input_destroy(self.handle);
            }
            self.handle = ptr::null_mut();
        }
    }
}
