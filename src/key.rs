//! OpenPGP key handles.
//!
//! A [`Key`] wraps an `rnp_key_handle_t`. It borrows the [`Context`](crate::Context)
//! it came from for its lifetime.

use crate::context::Context;
use crate::error::{self, check, Result};
use crate::ffi;
use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::os::raw::c_char;
use std::ptr;

/// Borrowed handle to a key in the FFI's keyring.
pub struct Key<'ctx> {
    pub(crate) handle: ffi::rnp_key_handle_t,
    _ctx: PhantomData<&'ctx Context>,
}

/// How to locate a key within a keyring.
#[derive(Clone, Copy, Debug)]
pub enum KeyIdentifier<'a> {
    Userid(&'a str),
    Keyid(&'a str),
    Fingerprint(&'a str),
    Grip(&'a str),
}

impl<'a> KeyIdentifier<'a> {
    fn type_str(self) -> &'static str {
        match self {
            KeyIdentifier::Userid(_) => "userid",
            KeyIdentifier::Keyid(_) => "keyid",
            KeyIdentifier::Fingerprint(_) => "fingerprint",
            KeyIdentifier::Grip(_) => "grip",
        }
    }

    fn value_str(self) -> &'a str {
        match self {
            KeyIdentifier::Userid(s) => s,
            KeyIdentifier::Keyid(s) => s,
            KeyIdentifier::Fingerprint(s) => s,
            KeyIdentifier::Grip(s) => s,
        }
    }
}

impl Context {
    /// Locate a key by userid, keyid, fingerprint or grip.
    ///
    /// Returns `Ok(None)` when no key matches (librnp signals this by leaving
    /// the out-handle NULL while returning `RNP_SUCCESS`).
    pub fn find_key(&self, id: KeyIdentifier<'_>) -> Result<Option<Key<'_>>> {
        let type_c = CString::new(id.type_str()).unwrap();
        let value_c = CString::new(id.value_str()).map_err(|_| error::Error::NulByte)?;
        let mut handle: ffi::rnp_key_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_locate_key(
                self.ffi,
                type_c.as_ptr(),
                value_c.as_ptr(),
                &mut handle,
            ))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(Key::from_handle(handle)))
        }
    }

    /// Load keys from a raw byte buffer (e.g. an exported keyring, or an
    /// armored key block).
    ///
    /// `format` selects the on-the-wire keyring format (GPG, KBX, G10). For
    /// OpenPGP-armored or binary key data use `KeyringFormat::Gpg`.
    pub fn load_keys(
        &self,
        format: crate::context::KeyringFormat,
        bytes: &[u8],
        flags: LoadSaveFlags,
    ) -> Result<()> {
        let fmt_c = CString::new(format.as_str()).unwrap();
        let mut input: ffi::rnp_input_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_input_from_memory(
                &mut input,
                bytes.as_ptr(),
                bytes.len(),
                true, // copy — bytes may be freed after this call
            ))?;
            // Ensure input is destroyed even if load fails.
            let res = check(ffi::rnp_load_keys(
                self.ffi,
                fmt_c.as_ptr(),
                input,
                flags.bits(),
            ));
            let _ = ffi::rnp_input_destroy(input);
            res
        }
    }
}

/// Flags for `Context::load_keys` / `save_keys`. Wraps the `RNP_LOAD_SAVE_*`
/// constants.
#[derive(Clone, Copy, Debug, Default)]
pub struct LoadSaveFlags(pub u32);

impl LoadSaveFlags {
    pub const PUBLIC: Self = Self(ffi::RNP_LOAD_SAVE_PUBLIC_KEYS as u32);
    pub const SECRET: Self = Self(ffi::RNP_LOAD_SAVE_SECRET_KEYS as u32);
    pub const PERMISSIVE: Self = Self(ffi::RNP_LOAD_SAVE_PERMISSIVE as u32);
    pub const SINGLE: Self = Self(ffi::RNP_LOAD_SAVE_SINGLE as u32);
    pub const BASE64: Self = Self(ffi::RNP_LOAD_SAVE_BASE64 as u32);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for LoadSaveFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl<'ctx> Key<'ctx> {
    /// Crate-internal constructor: wrap a raw handle, borrowing `ctx`.
    pub(crate) fn from_handle(handle: ffi::rnp_key_handle_t) -> Self {
        Key {
            handle,
            _ctx: PhantomData,
        }
    }

    /// Export this key as raw bytes. Pass `ExportFlags::ARMORED` for ASCII
    /// armor, otherwise binary OpenPGP packets are produced.
    pub fn export(&self, flags: ExportFlags) -> Result<Vec<u8>> {
        let mut output: ffi::rnp_output_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_output_to_memory(&mut output, 0))?;
            // Run the export, then drain the buffer regardless of outcome.
            let export_res = check(ffi::rnp_key_export(self.handle, output, flags.bits()));
            let drain_res = self.drain_memory_output(output);
            export_res.and(drain_res)
        }
    }

    /// The key's primary user id, if any.
    pub fn primary_uid(&self) -> Result<Option<String>> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            // rnp_key_get_primary_uid returns RNP_ERROR_NOT_FOUND when there's
            // no primary uid; treat that as None.
            let code = ffi::rnp_key_get_primary_uid(self.handle, &mut raw);
            if code == error::NOT_FOUND {
                return Ok(None);
            }
            check(code)?;
            if raw.is_null() {
                return Ok(None);
            }
            let s = CStr::from_ptr(raw).to_string_lossy().into_owned();
            ffi::rnp_buffer_destroy(raw as *mut _);
            Ok(Some(s))
        }
    }

    unsafe fn drain_memory_output(&self, output: ffi::rnp_output_t) -> Result<Vec<u8>> {
        // Edition 2024: wrap body so unsafe ops are admitted.
        unsafe {
            let mut buf: *mut u8 = ptr::null_mut();
            let mut len: usize = 0;
            let res = check(ffi::rnp_output_memory_get_buf(output, &mut buf, &mut len, true));
            let out = if res.is_ok() {
                if buf.is_null() || len == 0 {
                    Ok(Vec::new())
                } else {
                    // do_copy was true, so buf is owned by us.
                    let v = std::slice::from_raw_parts(buf, len).to_vec();
                    // rnp allocated the copy via rnp_buffer style allocator.
                    ffi::rnp_buffer_destroy(buf as *mut _);
                    Ok(v)
                }
            } else {
                res.map(|_| Vec::new())
            };
            let _ = ffi::rnp_output_destroy(output);
            out
        }
    }
}

impl<'ctx> Drop for Key<'ctx> {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                let _ = ffi::rnp_key_handle_destroy(self.handle);
            }
            self.handle = ptr::null_mut();
        }
    }
}

/// Flags for `Key::export`. Wraps the `RNP_KEY_EXPORT_*` constants.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExportFlags(pub u32);

impl ExportFlags {
    pub const ARMORED: Self = Self(ffi::RNP_KEY_EXPORT_ARMORED as u32);
    pub const PUBLIC: Self = Self(ffi::RNP_KEY_EXPORT_PUBLIC as u32);
    pub const SECRET: Self = Self(ffi::RNP_KEY_EXPORT_SECRET as u32);
    pub const SUBKEYS: Self = Self(ffi::RNP_KEY_EXPORT_SUBKEYS as u32);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for ExportFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
