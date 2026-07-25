//! OpenPGP key handles.
//!
//! A [`Key`] wraps an `rnp_key_handle_t`. It borrows the [`Context`](crate::Context)
//! it came from for its lifetime. Child handles ([`crate::Uid`], [`crate::Subkey`],
//! [`crate::Signature`]) borrow the `Key` for *their* lifetime.

use crate::context::Context;
use crate::error::{self, check, Result};
use crate::ffi;
use crate::ops::{cstr_to_optional_string, cstr_to_string, Input, Output};
use crate::keygen::{Cipher, Compression, Hash};
use std::ffi::CString;
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
        let input = Input::from_memory(bytes)?;
        unsafe {
            check(ffi::rnp_load_keys(
                self.ffi,
                fmt_c.as_ptr(),
                input.as_ptr(),
                flags.bits(),
            ))
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

/// Flags for `Key::export`. Wraps the `RNP_KEY_EXPORT_*` constants.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExportFlags(pub u32);

impl ExportFlags {
    pub const ARMORED: Self = Self(ffi::RNP_KEY_EXPORT_ARMORED as u32);
    pub const PUBLIC: Self = Self(ffi::RNP_KEY_EXPORT_PUBLIC as u32);
    pub const SECRET: Self = Self(ffi::RNP_KEY_EXPORT_SECRET as u32);
    pub const SUBKEYS: Self = Self(ffi::RNP_KEY_EXPORT_SUBKEYS as u32);
    pub const BASE64: Self = Self(ffi::RNP_KEY_EXPORT_BASE64 as u32);

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

/// Flags for `Key::remove`. Wraps the `RNP_KEY_REMOVE_*` constants.
#[derive(Clone, Copy, Debug, Default)]
pub struct RemoveFlags(pub u32);

impl RemoveFlags {
    pub const PUBLIC: Self = Self(ffi::RNP_KEY_REMOVE_PUBLIC as u32);
    pub const SECRET: Self = Self(ffi::RNP_KEY_REMOVE_SECRET as u32);
    pub const SUBKEYS: Self = Self(ffi::RNP_KEY_REMOVE_SUBKEYS as u32);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for RemoveFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Flags for `Key::remove_signatures`. Wraps the `RNP_KEY_SIGNATURE_*`
/// selector constants.
#[derive(Clone, Copy, Debug, Default)]
pub struct RemoveSignaturesFlags(pub u32);

impl RemoveSignaturesFlags {
    pub const INVALID: Self = Self(ffi::RNP_KEY_SIGNATURE_INVALID as u32);
    pub const UNKNOWN_KEY: Self = Self(ffi::RNP_KEY_SIGNATURE_UNKNOWN_KEY as u32);
    pub const NON_SELF_SIG: Self = Self(ffi::RNP_KEY_SIGNATURE_NON_SELF_SIG as u32);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for RemoveSignaturesFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Flags for `Context::unload_keys`. Wraps the `RNP_KEY_UNLOAD_*` constants.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnloadFlags(pub u32);

impl UnloadFlags {
    pub const PUBLIC: Self = Self(ffi::RNP_KEY_UNLOAD_PUBLIC as u32);
    pub const SECRET: Self = Self(ffi::RNP_KEY_UNLOAD_SECRET as u32);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for UnloadFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

// ---------------------------------------------------------------------------
// Key methods — read-only inspection
// ---------------------------------------------------------------------------

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
        let output = Output::to_memory()?;
        unsafe {
            check(ffi::rnp_key_export(self.handle, output.as_ptr(), flags.bits()))?;
        }
        output.into_bytes()
    }

    /// Export an Autocrypt-compatible form of this key (a single-UID
    /// public key suitable for inclusion in email headers). `subkey` may
    /// be `None` for the primary-only case.
    pub fn export_autocrypt(
        &self,
        subkey: Option<&Key<'_>>,
        uid: Option<&str>,
        flags: ExportFlags,
    ) -> Result<Vec<u8>> {
        let subkey_handle = subkey.map(|k| k.handle).unwrap_or(self.handle);
        let uid_c = uid
            .map(|s| CString::new(s).map_err(|_| error::Error::NulByte))
            .transpose()?;
        let uid_ptr = uid_c.as_ref().map(|c| c.as_ptr()).unwrap_or(ptr::null());
        let output = Output::to_memory()?;
        unsafe {
            check(ffi::rnp_key_export_autocrypt(
                self.handle,
                subkey_handle,
                uid_ptr,
                output.as_ptr(),
                flags.bits(),
            ))?;
        }
        output.into_bytes()
    }

    /// Export a revocation certificate for this key. The hash algorithm
    /// and revocation reason are configured by `hash` and `reason`; the
    /// password (if needed) is obtained from the configured password
    /// provider.
    pub fn export_revocation(
        &self,
        flags: ExportFlags,
        reason: RevocationReason,
        hash: Hash,
    ) -> Result<Vec<u8>> {
        let reason_c = CString::new(reason.code_str()).unwrap();
        let hash_c = CString::new(hash.as_str()).unwrap();
        let reason_text_c = match &reason.reason {
            Some(s) => Some(CString::new(s.as_str()).map_err(|_| error::Error::NulByte)?),
            None => None,
        };
        let reason_text_ptr = reason_text_c
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(ptr::null());
        let output = Output::to_memory()?;
        unsafe {
            check(ffi::rnp_key_export_revocation(
                self.handle,
                output.as_ptr(),
                flags.bits(),
                hash_c.as_ptr(),
                reason_c.as_ptr(),
                reason_text_ptr,
            ))?;
        }
        output.into_bytes()
    }

    // --- scalar identifiers ----------------------------------------------

    /// The key's primary user id, if any.
    pub fn primary_uid(&self) -> Result<Option<String>> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            let code = ffi::rnp_key_get_primary_uid(self.handle, &mut raw);
            if code == error::NOT_FOUND {
                return Ok(None);
            }
            check(code)?;
            Ok(cstr_to_optional_string(raw))
        }
    }

    /// Algorithm name (e.g. `"RSA"`, `"EDDSA"`).
    pub fn alg(&self) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_get_alg(self.handle, &mut raw))?;
            cstr_to_string(raw)
        }
    }

    /// Key size in bits. Returns 0 for curves where the concept doesn't apply.
    pub fn bits(&self) -> Result<u32> {
        let mut n: u32 = 0;
        unsafe { check(ffi::rnp_key_get_bits(self.handle, &mut n))? };
        Ok(n)
    }

    /// DSA `q` size in bits. Only meaningful for DSA keys.
    pub fn dsa_qbits(&self) -> Result<u32> {
        let mut n: u32 = 0;
        unsafe { check(ffi::rnp_key_get_dsa_qbits(self.handle, &mut n))? };
        Ok(n)
    }

    /// Curve name (e.g. `"NIST P-256"`, `"Ed25519"`). Returns `None` if the
    /// algorithm is not curve-based.
    pub fn curve(&self) -> Result<Option<String>> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            let code = ffi::rnp_key_get_curve(self.handle, &mut raw);
            if code == error::NOT_FOUND {
                return Ok(None);
            }
            check(code)?;
            Ok(cstr_to_optional_string(raw))
        }
    }

    /// Key version (4 or 6 in current OpenPGP; legacy v3 possible).
    pub fn version(&self) -> Result<u32> {
        let mut n: u32 = 0;
        unsafe { check(ffi::rnp_key_get_version(self.handle, &mut n))? };
        Ok(n)
    }

    /// Creation time, seconds since the Unix epoch.
    pub fn creation(&self) -> Result<u32> {
        let mut n: u32 = 0;
        unsafe { check(ffi::rnp_key_get_creation(self.handle, &mut n))? };
        Ok(n)
    }

    /// Expiration in seconds from creation. `0` means no expiration.
    pub fn expiration(&self) -> Result<u32> {
        let mut n: u32 = 0;
        unsafe { check(ffi::rnp_key_get_expiration(self.handle, &mut n))? };
        Ok(n)
    }

    /// Hex keyid (e.g. `"014F7B24CD14F2A5"`).
    pub fn keyid(&self) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_get_keyid(self.handle, &mut raw))?;
            cstr_to_string(raw)
        }
    }

    /// Hex fingerprint (40 chars for v4 SHA-1, 64 for v6 SHA-256).
    pub fn fingerprint(&self) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_get_fprint(self.handle, &mut raw))?;
            cstr_to_string(raw)
        }
    }

    /// Hex key grip (librnp's SHA-1-based identifier used for keyring lookups).
    pub fn grip(&self) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_get_grip(self.handle, &mut raw))?;
            cstr_to_string(raw)
        }
    }

    /// Fingerprint of the primary key that this subkey belongs to. `None`
    /// if this is itself a primary.
    pub fn primary_fprint(&self) -> Result<Option<String>> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            let code = ffi::rnp_key_get_primary_fprint(self.handle, &mut raw);
            if code == error::NOT_FOUND {
                return Ok(None);
            }
            check(code)?;
            Ok(cstr_to_optional_string(raw))
        }
    }

    /// Grip of the primary key that this subkey belongs to. `None` if this
    /// is itself a primary.
    pub fn primary_grip(&self) -> Result<Option<String>> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            let code = ffi::rnp_key_get_primary_grip(self.handle, &mut raw);
            if code == error::NOT_FOUND {
                return Ok(None);
            }
            check(code)?;
            Ok(cstr_to_optional_string(raw))
        }
    }

    /// Key usage flags word (ORed `RNP_KEY_USAGE_*` constants). Use
    /// [`Self::allows_usage`] to test individual usages.
    pub fn allows_usage(&self, usage: crate::keygen::KeyUsage) -> Result<bool> {
        let usage_c = CString::new(usage.as_str()).unwrap();
        let mut b: bool = false;
        unsafe { check(ffi::rnp_key_allows_usage(self.handle, usage_c.as_ptr(), &mut b))? };
        Ok(b)
    }

    // --- booleans --------------------------------------------------------

    pub fn have_secret(&self) -> Result<bool> {
        let mut b: bool = false;
        unsafe { check(ffi::rnp_key_have_secret(self.handle, &mut b))? };
        Ok(b)
    }

    pub fn have_public(&self) -> Result<bool> {
        let mut b: bool = false;
        unsafe { check(ffi::rnp_key_have_public(self.handle, &mut b))? };
        Ok(b)
    }

    pub fn is_primary(&self) -> Result<bool> {
        let mut b: bool = false;
        unsafe { check(ffi::rnp_key_is_primary(self.handle, &mut b))? };
        Ok(b)
    }

    pub fn is_sub(&self) -> Result<bool> {
        let mut b: bool = false;
        unsafe { check(ffi::rnp_key_is_sub(self.handle, &mut b))? };
        Ok(b)
    }

    pub fn is_valid(&self) -> Result<bool> {
        let mut b: bool = false;
        unsafe { check(ffi::rnp_key_is_valid(self.handle, &mut b))? };
        Ok(b)
    }

    pub fn is_revoked(&self) -> Result<bool> {
        let mut b: bool = false;
        unsafe { check(ffi::rnp_key_is_revoked(self.handle, &mut b))? };
        Ok(b)
    }

    pub fn is_locked(&self) -> Result<bool> {
        let mut b: bool = false;
        unsafe { check(ffi::rnp_key_is_locked(self.handle, &mut b))? };
        Ok(b)
    }

    pub fn is_protected(&self) -> Result<bool> {
        let mut b: bool = false;
        unsafe { check(ffi::rnp_key_is_protected(self.handle, &mut b))? };
        Ok(b)
    }

    /// Last time the key is valid, as a 32-bit seconds-since-epoch. Returns
    /// `u32::MAX` if the key never expires.
    pub fn valid_till(&self) -> Result<u32> {
        let mut n: u32 = 0;
        unsafe { check(ffi::rnp_key_valid_till(self.handle, &mut n))? };
        Ok(n)
    }

    /// Same as [`Self::valid_till`] but 64-bit (y2038-safe).
    pub fn valid_till64(&self) -> Result<u64> {
        let mut n: u64 = 0;
        unsafe { check(ffi::rnp_key_valid_till64(self.handle, &mut n))? };
        Ok(n)
    }

    // --- protection ------------------------------------------------------

    pub fn protection_type(&self) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_get_protection_type(self.handle, &mut raw))?;
            cstr_to_string(raw)
        }
    }

    pub fn protection_mode(&self) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_get_protection_mode(self.handle, &mut raw))?;
            cstr_to_string(raw)
        }
    }

    pub fn protection_cipher(&self) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_get_protection_cipher(self.handle, &mut raw))?;
            cstr_to_string(raw)
        }
    }

    pub fn protection_hash(&self) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_get_protection_hash(self.handle, &mut raw))?;
            cstr_to_string(raw)
        }
    }

    pub fn protection_iterations(&self) -> Result<usize> {
        let mut n: usize = 0;
        unsafe { check(ffi::rnp_key_get_protection_iterations(self.handle, &mut n))? };
        Ok(n)
    }

    // --- revocation ------------------------------------------------------

    /// Textual reason for the key's revocation, if any.
    pub fn revocation_reason(&self) -> Result<Option<String>> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            let code = ffi::rnp_key_get_revocation_reason(self.handle, &mut raw);
            if code == error::NOT_FOUND {
                return Ok(None);
            }
            check(code)?;
            Ok(cstr_to_optional_string(raw))
        }
    }

    /// Number of revokers (third-party keys authorized to revoke this key).
    pub fn revoker_count(&self) -> Result<usize> {
        let mut n: usize = 0;
        unsafe { check(ffi::rnp_key_get_revoker_count(self.handle, &mut n))? };
        Ok(n)
    }

    /// Revocation key specification at `idx` (a string of the form
    /// `"1:FINGERPRINT"`).
    pub fn revoker_at(&self, idx: usize) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_get_revoker_at(self.handle, idx, &mut raw))?;
            cstr_to_string(raw)
        }
    }

    // --- 25519 bit-tweak (v6 HKP fingerprint masking) --------------------

    pub fn is_25519_bits_tweaked(&self) -> Result<bool> {
        let mut b: bool = false;
        unsafe { check(ffi::rnp_key_25519_bits_tweaked(self.handle, &mut b))? };
        Ok(b)
    }

    pub fn set_25519_bits_tweak(&self) -> Result<()> {
        unsafe { check(ffi::rnp_key_25519_bits_tweak(self.handle)) }
    }

    // --- child-handle enumeration ----------------------------------------

    pub fn uid_count(&self) -> Result<usize> {
        let mut n: usize = 0;
        unsafe { check(ffi::rnp_key_get_uid_count(self.handle, &mut n))? };
        Ok(n)
    }

    pub fn uid_at(&self, idx: usize) -> Result<Option<crate::Uid<'_>>> {
        let mut handle: ffi::rnp_uid_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_get_uid_handle_at(self.handle, idx, &mut handle))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(crate::Uid::from_handle(handle)))
        }
    }

    pub fn uids(&self) -> Result<Vec<crate::Uid<'_>>> {
        let n = self.uid_count()?;
        (0..n)
            .map(|i| self.uid_at(i)?.ok_or(error::Error::NullPointer))
            .collect()
    }

    pub fn subkey_count(&self) -> Result<usize> {
        let mut n: usize = 0;
        unsafe { check(ffi::rnp_key_get_subkey_count(self.handle, &mut n))? };
        Ok(n)
    }

    pub fn subkey_at(&self, idx: usize) -> Result<Option<crate::Subkey<'_>>> {
        let mut handle: ffi::rnp_key_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_get_subkey_at(self.handle, idx, &mut handle))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(crate::Subkey::from_handle(handle)))
        }
    }

    pub fn subkeys(&self) -> Result<Vec<crate::Subkey<'_>>> {
        let n = self.subkey_count()?;
        (0..n)
            .map(|i| self.subkey_at(i)?.ok_or(error::Error::NullPointer))
            .collect()
    }

    pub fn signature_count(&self) -> Result<usize> {
        let mut n: usize = 0;
        unsafe { check(ffi::rnp_key_get_signature_count(self.handle, &mut n))? };
        Ok(n)
    }

    pub fn signature_at(&self, idx: usize) -> Result<Option<crate::Signature<'_>>> {
        let mut handle: ffi::rnp_signature_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_get_signature_at(self.handle, idx, &mut handle))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(crate::Signature::from_handle(handle)))
        }
    }

    pub fn signatures(&self) -> Result<Vec<crate::Signature<'_>>> {
        let n = self.signature_count()?;
        (0..n)
            .map(|i| self.signature_at(i)?.ok_or(error::Error::NullPointer))
            .collect()
    }

    // --- JSON ------------------------------------------------------------

    /// Find the default subkey for a given usage. Returns `None` if no
    /// subkey is suitable. Wraps `rnp_key_get_default_key`.
    pub fn default_key_for(&self, usage: crate::keygen::KeyUsage) -> Result<Option<Key<'_>>> {
        let usage_c = CString::new(usage.as_str()).unwrap();
        let mut handle: ffi::rnp_key_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_get_default_key(
                self.handle,
                usage_c.as_ptr(),
                0,
                &mut handle,
            ))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(Key::from_handle(handle)))
        }
    }

    pub fn to_json(&self, flags: crate::dump::JsonFlags) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_to_json(self.handle, flags.bits(), &mut raw))?;
            cstr_to_string(raw)
        }
    }

    pub fn packets_to_json(
        &self,
        secret: bool,
        flags: crate::dump::JsonDumpFlags,
    ) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_key_packets_to_json(
                self.handle,
                secret,
                flags.bits(),
                &mut raw,
            ))?;
            cstr_to_string(raw)
        }
    }
}

// ---------------------------------------------------------------------------
// Key methods — mutation
// ---------------------------------------------------------------------------

/// Reason for revoking a key or UID.
#[derive(Clone, Debug, Default)]
pub struct RevocationReason {
    pub code: RevocationCode,
    pub reason: Option<String>,
}

impl RevocationReason {
    pub fn new(code: RevocationCode) -> Self {
        RevocationReason { code, reason: None }
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    pub(crate) fn code_str(&self) -> &'static str {
        self.code.as_str()
    }
}

/// Revocation reason code. Values 0..=3 per RFC 4880 §5.2.3.23.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RevocationCode {
    /// `"0"` — no specific reason (default).
    #[default]
    NoReason,
    /// `"1"` — key superseded.
    Superseded,
    /// `"2"` — key material compromised.
    Compromised,
    /// `"3"` — key retired.
    Retired,
}

impl RevocationCode {
    pub fn as_str(self) -> &'static str {
        match self {
            RevocationCode::NoReason => "0",
            RevocationCode::Superseded => "1",
            RevocationCode::Compromised => "2",
            RevocationCode::Retired => "3",
        }
    }
}

/// Options for [`Key::protect`]. Defaults match librnp's internal defaults
/// (AES256 / SHA256 / iterated-and-salted).
#[derive(Default)]
pub struct ProtectOptions {
    pub(crate) password: Option<String>,
    pub(crate) cipher: Option<Cipher>,
    pub(crate) mode: Option<String>,
    pub(crate) hash: Option<Hash>,
    pub(crate) iterations: Option<usize>,
}

impl ProtectOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn password(mut self, p: impl Into<String>) -> Self {
        self.password = Some(p.into());
        self
    }

    pub fn cipher(mut self, c: Cipher) -> Self {
        self.cipher = Some(c);
        self
    }

    pub fn mode(mut self, m: impl Into<String>) -> Self {
        self.mode = Some(m.into());
        self
    }

    pub fn hash(mut self, h: Hash) -> Self {
        self.hash = Some(h);
        self
    }

    pub fn iterations(mut self, n: usize) -> Self {
        self.iterations = Some(n);
        self
    }
}

/// Options for [`Key::add_uid`].
#[derive(Default)]
pub struct AddUidOptions {
    pub(crate) hash: Option<Hash>,
    pub(crate) key_flags: Option<u32>,
    pub(crate) key_expiration: Option<u32>,
    pub(crate) primary: bool,
}

impl AddUidOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn hash(mut self, h: Hash) -> Self {
        self.hash = Some(h);
        self
    }

    pub fn key_flags(mut self, flags: u32) -> Self {
        self.key_flags = Some(flags);
        self
    }

    pub fn key_expiration(mut self, seconds: u32) -> Self {
        self.key_expiration = Some(seconds);
        self
    }

    pub fn primary(mut self, p: bool) -> Self {
        self.primary = p;
        self
    }
}

/// Helper: leak a `CString` so the C side can read it for the duration of a
/// call. Currently unused — kept for future callbacks that need a stable
/// pointer beyond a single FFI call.
#[allow(dead_code)]
fn leak_cstring(c: std::ffi::CString) -> *const c_char {
    let ptr = c.as_ptr();
    std::mem::forget(c);
    ptr
}

impl<'ctx> Key<'ctx> {
    /// Protect (encrypt) the secret material with `options.password`.
    pub fn protect(&self, options: &ProtectOptions) -> Result<()> {
        let pw = options
            .password
            .as_ref()
            .ok_or(error::Error::NullPointer)?;
        let pw_c = CString::new(pw.as_str()).map_err(|_| error::Error::NulByte)?;
        let cipher_c = options
            .cipher
            .map(|c| CString::new(c.as_str()).unwrap())
            .unwrap_or_else(|| CString::new("AES256").unwrap());
        // Cipher mode for secret-key protection: CFB (default), CBC, or OCB.
        // Distinct from the S2K type.
        let mode_c = options
            .mode
            .as_ref()
            .map(|m| CString::new(m.as_str()).unwrap())
            .unwrap_or_else(|| CString::new("CFB").unwrap());
        let hash_c = options
            .hash
            .map(|h| CString::new(h.as_str()).unwrap())
            .unwrap_or_else(|| CString::new("SHA256").unwrap());
        unsafe {
            check(ffi::rnp_key_protect(
                self.handle,
                pw_c.as_ptr(),
                cipher_c.as_ptr(),
                mode_c.as_ptr(),
                hash_c.as_ptr(),
                options.iterations.unwrap_or(0),
            ))?;
        }
        Ok(())
    }

    /// Remove the secret-key protection. `password` is required if the key
    /// is protected; pass `None` to use the configured password provider.
    pub fn unprotect(&self, password: Option<&str>) -> Result<()> {
        let pw_c = password.map(|p| CString::new(p).map_err(|_| error::Error::NulByte)).transpose()?;
        let pw_ptr = pw_c.as_ref().map(|c| c.as_ptr()).unwrap_or(ptr::null());
        unsafe { check(ffi::rnp_key_unprotect(self.handle, pw_ptr)) }
    }

    /// Lock the key — discard the in-memory decrypted secret material.
    pub fn lock(&self) -> Result<()> {
        unsafe { check(ffi::rnp_key_lock(self.handle)) }
    }

    /// Unlock the key (decrypt secret material in memory).
    pub fn unlock(&self, password: Option<&str>) -> Result<()> {
        let pw_c = password.map(|p| CString::new(p).map_err(|_| error::Error::NulByte)).transpose()?;
        let pw_ptr = pw_c.as_ref().map(|c| c.as_ptr()).unwrap_or(ptr::null());
        unsafe { check(ffi::rnp_key_unlock(self.handle, pw_ptr)) }
    }

    /// Add a user ID to this key. The key must be unlocked if protected.
    pub fn add_uid(&self, uid: &str, options: &AddUidOptions) -> Result<()> {
        let uid_c = CString::new(uid).map_err(|_| error::Error::NulByte)?;
        let hash_c = options
            .hash
            .map(|h| CString::new(h.as_str()).unwrap())
            .unwrap_or_else(|| CString::new("SHA256").unwrap());
        unsafe {
            check(ffi::rnp_key_add_uid(
                self.handle,
                uid_c.as_ptr(),
                hash_c.as_ptr(),
                options.key_expiration.unwrap_or(0),
                options.key_flags.unwrap_or(0) as u8,
                options.primary,
            ))?;
        }
        Ok(())
    }

    /// Revoke this key. The hash algorithm and reason are configured; if
    /// the secret key is locked, the password is obtained from the password
    /// provider.
    pub fn revoke(&self, reason: RevocationReason, hash: Hash) -> Result<()> {
        let reason_c = CString::new(reason.code_str()).unwrap();
        let hash_c = CString::new(hash.as_str()).unwrap();
        let reason_text_c = match &reason.reason {
            Some(s) => Some(CString::new(s.as_str()).map_err(|_| error::Error::NulByte)?),
            None => None,
        };
        let reason_text_ptr = reason_text_c
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(ptr::null());
        unsafe {
            check(ffi::rnp_key_revoke(
                self.handle,
                0,
                hash_c.as_ptr(),
                reason_c.as_ptr(),
                reason_text_ptr,
            ))?;
        }
        Ok(())
    }

    /// Update the key's expiration. `seconds` is from creation. Requires
    /// an unlocked key.
    pub fn set_expiration(&self, seconds: u32) -> Result<()> {
        unsafe { check(ffi::rnp_key_set_expiration(self.handle, seconds)) }
    }

    /// Remove the key from its keyring.
    pub fn remove(&self, flags: RemoveFlags) -> Result<()> {
        unsafe { check(ffi::rnp_key_remove(self.handle, flags.bits())) }
    }

    /// Remove signatures matching `flags`. The optional callback is not
    /// surfaced — callers can re-inspect remaining signatures after the call.
    pub fn remove_signatures(&self, flags: RemoveSignaturesFlags) -> Result<()> {
        unsafe {
            check(ffi::rnp_key_remove_signatures(
                self.handle,
                flags.bits(),
                None,
                ptr::null_mut(),
            ))
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

#[allow(unused_imports)]
use Compression as _CompressionAlias;
