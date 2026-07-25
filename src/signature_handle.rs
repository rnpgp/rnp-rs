//! Signature handle and subpacket type.
//!
//! [`Signature`] wraps `rnp_signature_handle_t`. It borrows the parent
//! [`Key`](crate::Key) for its lifetime. A signature is one of:
//!
//! - a self-signature binding a UID or subkey to its primary,
//! - a direct signature on a key,
//! - a third-party certification,
//! - a revocation signature.

use crate::error::{self, check, Result};
use crate::ffi;
use crate::ops::{cstr_to_optional_string, cstr_to_string};
use std::marker::PhantomData;
use std::os::raw::c_char;
use std::ptr;

/// Borrowed handle to a signature on a [`Key`](crate::Key) or [`Uid`](crate::Uid).
pub struct Signature<'parent> {
    pub(crate) handle: ffi::rnp_signature_handle_t,
    _parent: PhantomData<&'parent ()>,
}

impl<'parent> Signature<'parent> {
    pub(crate) fn from_handle(handle: ffi::rnp_signature_handle_t) -> Self {
        Signature {
            handle,
            _parent: PhantomData,
        }
    }

    /// Signature type as a string (e.g. `"binary"`, `"text"`,
    /// `"certification generic"`). See RFC 4880 §5.2.1 for the meaning.
    pub fn sig_type(&self) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_signature_get_type(self.handle, &mut raw))?;
            cstr_to_string(raw)
        }
    }

    /// Signing algorithm name (e.g. `"RSA"`, `"EDDSA"`).
    pub fn alg(&self) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_signature_get_alg(self.handle, &mut raw))?;
            cstr_to_string(raw)
        }
    }

    /// Hash algorithm name (e.g. `"SHA256"`).
    pub fn hash_alg(&self) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_signature_get_hash_alg(self.handle, &mut raw))?;
            cstr_to_string(raw)
        }
    }

    /// Creation time, seconds since the Unix epoch.
    pub fn creation(&self) -> Result<u32> {
        let mut n: u32 = 0;
        unsafe { check(ffi::rnp_signature_get_creation(self.handle, &mut n))? };
        Ok(n)
    }

    /// Expiration in seconds from creation. `0` means no expiration.
    pub fn expiration(&self) -> Result<u32> {
        let mut n: u32 = 0;
        unsafe { check(ffi::rnp_signature_get_expiration(self.handle, &mut n))? };
        Ok(n)
    }

    /// Key flags word (ORed `RNP_KEY_USAGE_*`).
    pub fn key_flags(&self) -> Result<u32> {
        let mut n: u32 = 0;
        unsafe { check(ffi::rnp_signature_get_key_flags(self.handle, &mut n))? };
        Ok(n)
    }

    /// Key expiration recorded in this signature, if any. `0` means no
    /// expiration.
    pub fn key_expiration(&self) -> Result<u32> {
        let mut n: u32 = 0;
        unsafe { check(ffi::rnp_signature_get_key_expiration(self.handle, &mut n))? };
        Ok(n)
    }

    /// Whether this signature marks its UID as the primary UID.
    pub fn primary_uid(&self) -> Result<bool> {
        let mut b: bool = false;
        unsafe { check(ffi::rnp_signature_get_primary_uid(self.handle, &mut b))? };
        Ok(b)
    }

    /// Features bitmask (MDC, AEAD, v5 keys). See `RNP_KEY_FEATURE_*`.
    pub fn features(&self) -> Result<u32> {
        let mut n: u32 = 0;
        unsafe { check(ffi::rnp_signature_get_features(self.handle, &mut n))? };
        Ok(n)
    }

    /// Hex keyid of the signer.
    pub fn keyid(&self) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_signature_get_keyid(self.handle, &mut raw))?;
            cstr_to_string(raw)
        }
    }

    /// Hex fingerprint of the signer.
    pub fn key_fprint(&self) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_signature_get_key_fprint(self.handle, &mut raw))?;
            cstr_to_string(raw)
        }
    }

    /// Hex fingerprint of the signer (alias).
    pub fn signer_keyid(&self) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_signature_get_keyid(self.handle, &mut raw))?;
            cstr_to_string(raw)
        }
    }

    /// Preferred symmetric algorithms (cipher names).
    pub fn preferred_ciphers(&self) -> Result<Vec<String>> {
        preferred_list(self.handle, PreferredKind::Cipher)
    }

    /// Preferred hash algorithms (hash names).
    pub fn preferred_hashes(&self) -> Result<Vec<String>> {
        preferred_list(self.handle, PreferredKind::Hash)
    }

    /// Preferred compression algorithms (compression names).
    pub fn preferred_compressions(&self) -> Result<Vec<String>> {
        preferred_list(self.handle, PreferredKind::Compression)
    }

    /// Preferred key server URL, if any.
    pub fn key_server(&self) -> Result<Option<String>> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_signature_get_key_server(self.handle, &mut raw))?;
            Ok(cstr_to_optional_string(raw))
        }
    }

    /// Key server preferences bitmask.
    pub fn key_server_prefs(&self) -> Result<u32> {
        let mut n: u32 = 0;
        unsafe { check(ffi::rnp_signature_get_key_server_prefs(self.handle, &mut n))? };
        Ok(n)
    }

    /// Trust level (0..=255 per RFC 4880 §5.2.3.13).
    pub fn trust_level(&self) -> Result<u8> {
        let mut level: u8 = 0;
        let mut amount: u8 = 0;
        unsafe { check(ffi::rnp_signature_get_trust_level(self.handle, &mut level, &mut amount))? };
        Ok(level)
    }

    /// Trust amount (0..=255).
    pub fn trust_amount(&self) -> Result<u8> {
        let mut level: u8 = 0;
        let mut amount: u8 = 0;
        unsafe { check(ffi::rnp_signature_get_trust_level(self.handle, &mut level, &mut amount))? };
        Ok(amount)
    }

    /// Revocation reason `(code, text)`, if this is a revocation signature.
    /// Returns `None` if the signature is not a revocation.
    pub fn revocation_reason(&self) -> Result<Option<(String, String)>> {
        let mut code_raw: *mut c_char = ptr::null_mut();
        let mut reason_raw: *mut c_char = ptr::null_mut();
        unsafe {
            let code = ffi::rnp_signature_get_revocation_reason(
                self.handle,
                &mut code_raw,
                &mut reason_raw,
            );
            if code == error::NOT_FOUND {
                return Ok(None);
            }
            check(code)?;
            let c = cstr_to_optional_string(code_raw).unwrap_or_default();
            let r = cstr_to_optional_string(reason_raw).unwrap_or_default();
            Ok(Some((c, r)))
        }
    }

    /// Whether this signature is currently valid. Returns `Ok(true)` if so,
    /// else `Ok(false)` (or an `Err` for a hard parse failure).
    pub fn is_valid(&self) -> Result<bool> {
        let code = unsafe { ffi::rnp_signature_is_valid(self.handle, 0) };
        Ok(code == error::SUCCESS)
    }

    /// Number of subpackets on this signature.
    pub fn subpacket_count(&self) -> Result<usize> {
        let mut n: usize = 0;
        unsafe { check(ffi::rnp_signature_subpacket_count(self.handle, &mut n))? };
        Ok(n)
    }

    /// Borrow the subpacket at index `idx`.
    pub fn subpacket_at(&self, idx: usize) -> Result<Option<Subpacket>> {
        let mut handle: ffi::rnp_sig_subpacket_t = ptr::null_mut();
        unsafe {
            let code = ffi::rnp_signature_subpacket_at(self.handle, idx, &mut handle);
            if code == error::NOT_FOUND {
                return Ok(None);
            }
            check(code)?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(Subpacket::from_handle(handle)))
        }
    }

    /// All subpackets.
    pub fn subpackets(&self) -> Result<Vec<Subpacket>> {
        let n = self.subpacket_count()?;
        (0..n)
            .map(|i| self.subpacket_at(i)?.ok_or(error::Error::NullPointer))
            .collect()
    }

    /// Dump this signature packet as JSON. See
    /// [`crate::dump::JsonDumpFlags`].
    pub fn packet_to_json(&self, flags: crate::dump::JsonDumpFlags) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_signature_packet_to_json(
                self.handle,
                flags.bits(),
                &mut raw,
            ))?;
            cstr_to_string(raw)
        }
    }
}

impl<'parent> Drop for Signature<'parent> {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                let _ = ffi::rnp_signature_handle_destroy(self.handle);
            }
            self.handle = ptr::null_mut();
        }
    }
}

/// Borrowed handle to a subpacket on a [`Signature`].
pub struct Subpacket {
    handle: ffi::rnp_sig_subpacket_t,
}

impl Subpacket {
    pub(crate) fn from_handle(handle: ffi::rnp_sig_subpacket_t) -> Self {
        Subpacket { handle }
    }

    /// Subpacket type per OpenPGP (RFC 4880 / 9580 §5.2.3.1).
    pub fn typ(&self) -> Result<u8> {
        let mut t: u8 = 0;
        let mut hashed: bool = false;
        let mut critical: bool = false;
        unsafe { check(ffi::rnp_signature_subpacket_info(self.handle, &mut t, &mut hashed, &mut critical))? };
        Ok(t)
    }

    /// Whether the subpacket is in the hashed area (cryptographically
    /// protected) or the unhashed area (informational).
    pub fn is_hashed(&self) -> Result<bool> {
        let mut t: u8 = 0;
        let mut hashed: bool = false;
        let mut critical: bool = false;
        unsafe { check(ffi::rnp_signature_subpacket_info(self.handle, &mut t, &mut hashed, &mut critical))? };
        Ok(hashed)
    }

    /// Whether the subpacket has the critical bit set.
    pub fn is_critical(&self) -> Result<bool> {
        let mut t: u8 = 0;
        let mut hashed: bool = false;
        let mut critical: bool = false;
        unsafe { check(ffi::rnp_signature_subpacket_info(self.handle, &mut t, &mut hashed, &mut critical))? };
        Ok(critical)
    }

    /// Raw subpacket body bytes.
    pub fn data(&self) -> Result<Vec<u8>> {
        let mut ptr: *mut u8 = ptr::null_mut();
        let mut len: usize = 0;
        unsafe {
            check(ffi::rnp_signature_subpacket_data(self.handle, &mut ptr, &mut len))?;
            if ptr.is_null() {
                return Ok(Vec::new());
            }
            let v = std::slice::from_raw_parts(ptr, len).to_vec();
            ffi::rnp_buffer_destroy(ptr as *mut _);
            Ok(v)
        }
    }
}

impl Drop for Subpacket {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                let _ = ffi::rnp_signature_subpacket_destroy(self.handle);
            }
            self.handle = ptr::null_mut();
        }
    }
}

enum PreferredKind {
    Cipher,
    Hash,
    Compression,
}

fn preferred_list(sig: ffi::rnp_signature_handle_t, kind: PreferredKind) -> Result<Vec<String>> {
    let mut count: usize = 0;
    unsafe {
        match kind {
            PreferredKind::Cipher => {
                check(ffi::rnp_signature_get_preferred_alg_count(sig, &mut count))?
            }
            PreferredKind::Hash => {
                check(ffi::rnp_signature_get_preferred_hash_count(sig, &mut count))?
            }
            PreferredKind::Compression => {
                check(ffi::rnp_signature_get_preferred_zalg_count(sig, &mut count))?
            }
        }
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            match kind {
                PreferredKind::Cipher => {
                    check(ffi::rnp_signature_get_preferred_alg(sig, i, &mut raw))?
                }
                PreferredKind::Hash => {
                    check(ffi::rnp_signature_get_preferred_hash(sig, i, &mut raw))?
                }
                PreferredKind::Compression => {
                    check(ffi::rnp_signature_get_preferred_zalg(sig, i, &mut raw))?
                }
            }
            out.push(cstr_to_optional_string(raw).unwrap_or_default());
        }
    }
    Ok(out)
}
