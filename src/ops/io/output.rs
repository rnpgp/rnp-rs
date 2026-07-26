//! [`Output`] — RAII wrapper around `rnp_output_t`, plus [`OutputFileFlags`].

use crate::error::{self, check, Result};
use crate::ffi;
use std::ffi::CString;
use std::ptr;

use super::armor_type::ArmorType;
use super::input::Input;

/// Owns an `rnp_output_t` for the lifetime of this value.
///
/// Construct with one of the `to_*` constructors. For memory destinations,
/// [`Output::into_bytes`] consumes the output and returns the buffered bytes.
pub struct Output {
    handle: ffi::rnp_output_t,
}

/// Flags for [`Output::to_file`]. Wraps the `RNP_OUTPUT_FILE_*` constants.
#[derive(Clone, Copy, Debug, Default)]
pub struct OutputFileFlags(pub u32);

impl OutputFileFlags {
    /// Overwrite an existing file. Wraps `RNP_OUTPUT_FILE_OVERWRITE`.
    pub const OVERWRITE: Self = Self(ffi::RNP_OUTPUT_FILE_OVERWRITE as u32);
    /// Write to a randomly-named temp file and rename on `finish()`. Wraps
    /// `RNP_OUTPUT_FILE_RANDOM`. Callers using this flag must call
    /// [`Output::finish`] before dropping to ensure the rename succeeds.
    pub const RANDOM: Self = Self(ffi::RNP_OUTPUT_FILE_RANDOM as u32);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for OutputFileFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl Output {
    /// Buffer output in memory. Use [`Output::into_bytes`] to drain.
    ///
    /// `max_alloc` of `0` means unlimited.
    pub fn to_memory() -> Result<Self> {
        Self::to_memory_with_max(0)
    }

    /// As [`Output::to_memory`], but with an upper bound on the allocation.
    pub fn to_memory_with_max(max_alloc: usize) -> Result<Self> {
        let mut handle: ffi::rnp_output_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_output_to_memory(&mut handle, max_alloc))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(Output { handle })
    }

    /// Discard all output.
    pub fn to_null() -> Result<Self> {
        let mut handle: ffi::rnp_output_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_output_to_null(&mut handle))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(Output { handle })
    }

    /// Write to `path`, overwriting if it already exists.
    pub fn to_path(path: &str) -> Result<Self> {
        let c = CString::new(path).map_err(|_| error::Error::PathNul)?;
        let mut handle: ffi::rnp_output_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_output_to_path(&mut handle, c.as_ptr()))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(Output { handle })
    }

    /// Write to `path` with explicit flags (overwrite / random-temp-rename).
    pub fn to_file(path: &str, flags: OutputFileFlags) -> Result<Self> {
        let c = CString::new(path).map_err(|_| error::Error::PathNul)?;
        let mut handle: ffi::rnp_output_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_output_to_file(&mut handle, c.as_ptr(), flags.bits()))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(Output { handle })
    }

    /// Write to process stdout.
    pub fn to_stdout() -> Result<Self> {
        let mut handle: ffi::rnp_output_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_output_to_stdout(&mut handle))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(Output { handle })
    }

    /// Wrap `base` with an ASCII-armor encoder. The returned [`Output`] owns
    /// the armor handle; `base` is borrowed for the lifetime of the wrapper
    /// (the C side stores the pointer).
    pub fn to_armor(base: &Output, ty: ArmorType) -> Result<Self> {
        let mut handle: ffi::rnp_output_t = ptr::null_mut();
        let c = CString::new(ty.as_str()).unwrap();
        unsafe {
            check(ffi::rnp_output_to_armor(base.handle, &mut handle, c.as_ptr()))?;
        }
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(Output { handle })
    }

    /// Write `bytes` to the output stream. Returns the number of bytes
    /// actually written.
    pub fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        let mut written: usize = 0;
        unsafe {
            check(ffi::rnp_output_write(
                self.handle,
                bytes.as_ptr() as *const _,
                bytes.len(),
                &mut written,
            ))?;
        }
        Ok(written)
    }

    /// Finalize the output. Required for [`OutputFileFlags::RANDOM`] (causes
    /// the temp file to be renamed to its final path) and harmless otherwise.
    pub fn finish(&mut self) -> Result<()> {
        unsafe { check(ffi::rnp_output_finish(self.handle)) }
    }

    /// Override the default armor line length (76). Only meaningful when the
    /// output was created via [`Output::to_armor`].
    pub fn set_armor_line_length(&mut self, line_len: usize) -> Result<()> {
        unsafe { check(ffi::rnp_output_armor_set_line_length(self.handle, line_len)) }
    }

    /// Pipe `input` through to this output until EOF. Consumes neither.
    pub fn pipe(&mut self, input: &Input) -> Result<()> {
        unsafe { check(ffi::rnp_output_pipe(input.as_ptr(), self.handle)) }
    }

    /// Consume a memory-backed output and return its buffered bytes.
    ///
    /// Only valid on outputs created via [`Output::to_memory`] /
    /// [`Output::to_memory_with_max`].
    pub fn into_bytes(self) -> Result<Vec<u8>> {
        // Pull the buffer out, then drop the Output handle. We do this by
        // taking ownership of `self` so Drop still runs and frees the
        // output structure (but not the byte buffer, which we own after the
        // do_copy=true call).
        let mut buf: *mut u8 = ptr::null_mut();
        let mut len: usize = 0;
        unsafe {
            let get = check(ffi::rnp_output_memory_get_buf(
                self.handle,
                &mut buf,
                &mut len,
                true, // copy — buf is owned by us, freed via rnp_buffer_destroy
            ));
            if get.is_err() {
                // Drop runs here and calls rnp_output_destroy.
                return get.map(|_| Vec::new());
            }
            if buf.is_null() || len == 0 {
                Ok(Vec::new())
            } else {
                let v = std::slice::from_raw_parts(buf, len).to_vec();
                ffi::rnp_buffer_destroy(buf as *mut _);
                Ok(v)
            }
        }
    }

    /// Raw handle for passing to librnp functions. Crate-internal.
    pub(crate) fn as_ptr(&self) -> ffi::rnp_output_t {
        self.handle
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: handle was produced by rnp_output_to_* and not yet
            // destroyed.
            unsafe {
                let _ = ffi::rnp_output_destroy(self.handle);
            }
            self.handle = ptr::null_mut();
        }
    }
}
