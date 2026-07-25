//! RAII wrappers around `rnp_input_t` and `rnp_output_t`.
//!
//! Every byte-stream that crosses the Rust/C boundary in this crate goes
//! through [`Input`] or [`Output`]. They own the underlying handle and free
//! it on `Drop`, even on error paths reached via `?`. This eliminates the
//! bespoke cleanup functions that previously appeared in `signature.rs` and
//! the duplicated `drain_memory_output` helpers in `key.rs` /
//! `signature.rs`.
//!
//! This module is also the only place in the crate that calls
//! `rnp_buffer_destroy` — every C-allocated string or buffer returned to
//! Rust is freed here, via [`cstr_to_string`] / [`cstr_to_optional_string`].
//!
//! ## Listener model
//!
//! Callback-based inputs and outputs are deferred to a later phase (they
//! require careful lifetime design around boxed trait objects). Memory, path,
//! stdin/stdout, file, null, and armor destinations are all supported here.

use crate::error::{self, check, Result};
use crate::ffi;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

// ---------------------------------------------------------------------------
// C-string / buffer return helpers
// ---------------------------------------------------------------------------

/// Convert a librnp-returned NUL-terminated string into an owned [`String`]
/// and free the C buffer.
///
/// Returns `None` if `raw` is null. Use [`cstr_to_string`] when null is an
/// error condition.
///
/// # Safety
///
/// `raw` must be either null or a pointer returned by librnp that the caller
/// is responsible for freeing via `rnp_buffer_destroy`.
pub unsafe fn cstr_to_optional_string(raw: *mut c_char) -> Option<String> {
    if raw.is_null() {
        return None;
    }
    // SAFETY: caller guarantees raw is a valid librnp-owned C string.
    let s = unsafe { CStr::from_ptr(raw) }.to_string_lossy().into_owned();
    unsafe { ffi::rnp_buffer_destroy(raw as *mut _) };
    Some(s)
}

/// Like [`cstr_to_optional_string`] but treats null as an [`Error::NullPointer`].
///
/// # Safety
///
/// See [`cstr_to_optional_string`].
pub unsafe fn cstr_to_string(raw: *mut c_char) -> Result<String> {
    unsafe { cstr_to_optional_string(raw) }.ok_or(error::Error::NullPointer)
}

/// Run an FFI getter that writes a NUL-terminated string into an out-param,
/// then convert the result to an owned [`String`] and free the buffer.
///
/// This is the canonical wrapper for the very common C-API shape
/// `rnp_X_get_Y(handle, *mut *mut c_char) -> rnp_result_t`. Use it instead
/// of repeating the `let mut raw: *mut c_char = ptr::null_mut(); check(...)?;
/// cstr_to_string(raw)` triplet at every call site.
///
/// # Safety
///
/// `f` must populate the out-pointer with a librnp-allocated buffer that
/// `rnp_buffer_destroy` can free.
pub fn call_for_string<F>(mut f: F) -> Result<String>
where
    F: FnMut(*mut *mut c_char) -> u32,
{
    let mut raw: *mut c_char = ptr::null_mut();
    let code = f(&mut raw);
    check(code)?;
    // SAFETY: the caller's closure populates `raw` per the FFI contract;
    // cstr_to_string frees it via rnp_buffer_destroy.
    unsafe { cstr_to_string(raw) }
}

/// Like [`call_for_string`] but maps `RNP_ERROR_NOT_FOUND` (and a null
/// out-pointer) to `Ok(None)`. Use for getters that legitimately return
/// "no value" rather than treating it as an error.
pub fn call_for_optional_string<F>(mut f: F) -> Result<Option<String>>
where
    F: FnMut(*mut *mut c_char) -> u32,
{
    let mut raw: *mut c_char = ptr::null_mut();
    let code = f(&mut raw);
    if code == error::NOT_FOUND {
        return Ok(None);
    }
    check(code)?;
    // SAFETY: as above.
    Ok(unsafe { cstr_to_optional_string(raw) })
}

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

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

/// Armor stream type. See `rnp_enarmor()` for the canonical string values.
#[derive(Clone, Copy, Debug)]
pub enum ArmorType {
    /// `"message"` — the default.
    Message,
    /// `"public key"`.
    PublicKey,
    /// `"secret key"`.
    SecretKey,
    /// `"signature"`.
    Signature,
    /// `"cleartext signed message"`.
    Cleartext,
}

impl ArmorType {
    pub fn as_str(self) -> &'static str {
        match self {
            ArmorType::Message => "message",
            ArmorType::PublicKey => "public key",
            ArmorType::SecretKey => "secret key",
            ArmorType::Signature => "signature",
            ArmorType::Cleartext => "cleartext signed message",
        }
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
        unsafe {
            check(ffi::rnp_output_armor_set_line_length(self.handle, line_len))
        }
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
