//! Verify operation: the typed surface over `rnp_op_verify_*`.
//!
//! [`VerifyOp`] owns the verify-operation handle; executing it produces a
//! [`VerifyResult`] that owns the per-signature / per-recipient / per-symenc
//! state for inspection. This is the surface decryption-result inspection
//! needs (phase 06 follow-up) and the one the existing
//! `signature::verify` should reuse.

use crate::context::Context;
use crate::error::{self, check, Result};
use crate::ffi;
use crate::ops::{cstr_to_optional_string, cstr_to_string, Input, Output};
use std::marker::PhantomData;
use std::os::raw::c_char;
use std::ptr;

/// Flags for [`VerifyOp::set_flags`]. Wraps `RNP_VERIFY_*`.
#[derive(Clone, Copy, Debug, Default)]
pub struct VerifyFlags(pub u32);

impl VerifyFlags {
    /// Don't inspect signatures during decrypt.
    pub const IGNORE_SIGS_ON_DECRYPT: Self =
        Self(ffi::RNP_VERIFY_IGNORE_SIGS_ON_DECRYPT as u32);
    /// Require all signatures to verify.
    pub const REQUIRE_ALL_SIGS: Self = Self(ffi::RNP_VERIFY_REQUIRE_ALL_SIGS as u32);
    /// Allow hidden (all-zero keyid) recipients.
    pub const ALLOW_HIDDEN_RECIPIENT: Self =
        Self(ffi::RNP_VERIFY_ALLOW_HIDDEN_RECIPIENT as u32);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for VerifyFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Builder over `rnp_op_verify_*`. Construct for inline or detached
/// verification, optionally set flags, then call [`VerifyOp::execute`] to
/// obtain a [`VerifyResult`].
pub struct VerifyOp<'ctx> {
    ctx: &'ctx Context,
    op: ffi::rnp_op_verify_t,
    _input: Input,
    // Output kept around so it lives until execute() flushes it.
    _output: Output,
    _phantom: PhantomData<&'ctx ()>,
}

impl<'ctx> VerifyOp<'ctx> {
    /// Begin inline verification. `signed_message` is the message produced
    /// by inline signing; `output` is where the embedded plaintext will be
    /// written (use `Output::to_null()` to discard).
    pub fn inline(
        ctx: &'ctx Context,
        signed_message: &[u8],
        output: Output,
    ) -> Result<Self> {
        let input = Input::from_memory(signed_message)?;
        let mut op: ffi::rnp_op_verify_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_verify_create(
                &mut op,
                ctx.ffi,
                input.as_ptr(),
                output.as_ptr(),
            ))?;
        }
        Ok(VerifyOp {
            ctx,
            op,
            _input: input,
            _output: output,
            _phantom: PhantomData,
        })
    }

    /// Begin detached verification. `signature` is the detached signature
    /// over `message`.
    pub fn detached(ctx: &'ctx Context, message: &[u8], signature: &[u8]) -> Result<Self> {
        let msg_input = Input::from_memory(message)?;
        let sig_input = Input::from_memory(signature)?;
        let null_out = Output::to_null()?;
        let mut op: ffi::rnp_op_verify_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_verify_detached_create(
                &mut op,
                ctx.ffi,
                msg_input.as_ptr(),
                sig_input.as_ptr(),
            ))?;
        }
        // Hold both inputs alive via a single owned Input that wraps both.
        // Simplest: leak them here — but we want RAII. So store them in a
        // small struct field. We use a tuple-Input trick: actually we just
        // discard sig_input's ownership to the op, which destroys it via
        // rnp_op_verify_destroy. Wrap the message input in _input for our
        // RAII; the sig input is destroyed by the op.
        std::mem::forget(sig_input);
        Ok(VerifyOp {
            ctx,
            op,
            _input: msg_input,
            _output: null_out,
            _phantom: PhantomData,
        })
    }

    pub fn set_flags(&mut self, flags: VerifyFlags) -> Result<()> {
        unsafe { check(ffi::rnp_op_verify_set_flags(self.op, flags.bits())) }
    }

    /// Execute the verification, returning the result for inspection.
    pub fn execute(self) -> Result<VerifyResult<'ctx>> {
        unsafe {
            check(ffi::rnp_op_verify_execute(self.op))?;
        }
        Ok(VerifyResult {
            ctx: self.ctx,
            op: self.op,
            _phantom: PhantomData,
        })
    }
}

/// Result of a verify (or decrypt) operation. Owns the verify-op handle
/// until drop; methods read out the per-signature / per-recipient state.
pub struct VerifyResult<'ctx> {
    ctx: &'ctx Context,
    op: ffi::rnp_op_verify_t,
    _phantom: PhantomData<&'ctx ()>,
}

impl<'ctx> VerifyResult<'ctx> {
    /// Borrow the underlying context (for follow-up `find_key` etc.).
    pub fn context(&self) -> &Context {
        self.ctx
    }

    /// Number of signatures found in the verified stream.
    pub fn signature_count(&self) -> Result<usize> {
        let mut n: usize = 0;
        unsafe { check(ffi::rnp_op_verify_get_signature_count(self.op, &mut n))? };
        Ok(n)
    }

    /// Borrow the per-signature result at `idx`.
    pub fn signature_at(&self, idx: usize) -> Result<Option<VerifySignature>> {
        let mut handle: ffi::rnp_op_verify_signature_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_verify_get_signature_at(self.op, idx, &mut handle))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(VerifySignature { handle }))
        }
    }

    /// All per-signature results.
    pub fn signatures(&self) -> Result<Vec<VerifySignature>> {
        let n = self.signature_count()?;
        (0..n)
            .map(|i| self.signature_at(i)?.ok_or(error::Error::NullPointer))
            .collect()
    }

    /// True iff at least one signature verified successfully.
    pub fn any_valid(&self) -> Result<bool> {
        let sigs = self.signatures()?;
        Ok(sigs.iter().any(|s| s.status_is_valid()))
    }

    /// Number of public-key recipients in the verified stream.
    pub fn recipient_count(&self) -> Result<usize> {
        let mut n: usize = 0;
        unsafe { check(ffi::rnp_op_verify_get_recipient_count(self.op, &mut n))? };
        Ok(n)
    }

    pub fn recipient_at(&self, idx: usize) -> Result<Option<Recipient>> {
        let mut handle: ffi::rnp_recipient_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_verify_get_recipient_at(self.op, idx, &mut handle))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(Recipient { handle }))
        }
    }

    pub fn recipients(&self) -> Result<Vec<Recipient>> {
        let n = self.recipient_count()?;
        (0..n)
            .map(|i| self.recipient_at(i)?.ok_or(error::Error::NullPointer))
            .collect()
    }

    /// The recipient that was actually used for decryption, if any.
    pub fn used_recipient(&self) -> Result<Option<Recipient>> {
        let mut handle: ffi::rnp_recipient_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_verify_get_used_recipient(self.op, &mut handle))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(Recipient { handle }))
        }
    }

    /// Number of symmetric-only encrypted session keys (password-based
    /// recipients) in the verified stream.
    pub fn symenc_count(&self) -> Result<usize> {
        let mut n: usize = 0;
        unsafe { check(ffi::rnp_op_verify_get_symenc_count(self.op, &mut n))? };
        Ok(n)
    }

    pub fn symenc_at(&self, idx: usize) -> Result<Option<Symenc>> {
        let mut handle: ffi::rnp_symenc_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_verify_get_symenc_at(self.op, idx, &mut handle))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(Symenc { handle }))
        }
    }

    pub fn symencs(&self) -> Result<Vec<Symenc>> {
        let n = self.symenc_count()?;
        (0..n)
            .map(|i| self.symenc_at(i)?.ok_or(error::Error::NullPointer))
            .collect()
    }

    /// The symenc that was actually used for decryption, if any.
    pub fn used_symenc(&self) -> Result<Option<Symenc>> {
        let mut handle: ffi::rnp_symenc_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_verify_get_used_symenc(self.op, &mut handle))?;
        }
        if handle.is_null() {
            Ok(None)
        } else {
            Ok(Some(Symenc { handle }))
        }
    }

    /// File metadata embedded in the literal-data packet, if any.
    pub fn file_info(&self) -> Result<Option<FileInfo>> {
        let mut name_raw: *mut c_char = ptr::null_mut();
        let mut mtime: u32 = 0;
        unsafe {
            let code = ffi::rnp_op_verify_get_file_info(self.op, &mut name_raw, &mut mtime);
            if code == error::NOT_FOUND {
                return Ok(None);
            }
            check(code)?;
            let name = cstr_to_optional_string(name_raw).unwrap_or_default();
            Ok(Some(FileInfo { name, mtime }))
        }
    }

    /// Format character of the verified message (`'b'` binary, `'t'` text,
    /// `'u'` UTF-8, etc. — see RFC 4880 §5.9). Returns `'\0'` if unknown.
    pub fn format(&self) -> Result<char> {
        let mut c: std::os::raw::c_char = 0;
        unsafe {
            check(ffi::rnp_op_verify_get_format(self.op, &mut c))?;
        }
        Ok(c as u8 as char)
    }

    /// Protection info `(mode, cipher, valid)` of the encrypted stream.
    /// `mode` is one of `"none"`, `"cfb"`, `"cfb-mdc"`, `"aead-ocb"`,
    /// `"aead-eax"`. `valid` is true iff MDC/AEAD integrity was verified.
    pub fn protection_info(&self) -> Result<(String, String, bool)> {
        let mut mode_raw: *mut c_char = ptr::null_mut();
        let mut cipher_raw: *mut c_char = ptr::null_mut();
        let mut valid: bool = false;
        unsafe {
            check(ffi::rnp_op_verify_get_protection_info(
                self.op,
                &mut mode_raw,
                &mut cipher_raw,
                &mut valid,
            ))?;
            let mode = cstr_to_string(mode_raw)?;
            let cipher = cstr_to_string(cipher_raw)?;
            Ok((mode, cipher, valid))
        }
    }
}

impl<'ctx> Drop for VerifyResult<'ctx> {
    fn drop(&mut self) {
        if !self.op.is_null() {
            unsafe {
                let _ = ffi::rnp_op_verify_destroy(self.op);
            }
            self.op = ptr::null_mut();
        }
    }
}

/// Per-signature verification status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureStatus {
    Valid,
    Invalid,
    Unknown,
}

/// Per-signature result extracted from a [`VerifyResult`].
pub struct VerifySignature {
    handle: ffi::rnp_op_verify_signature_t,
}

impl VerifySignature {
    /// True iff the signature was valid.
    pub fn status_is_valid(&self) -> bool {
        let code = unsafe { ffi::rnp_op_verify_signature_get_status(self.handle) };
        code == error::SUCCESS
    }

    /// Categorize the verification outcome.
    pub fn status(&self) -> SignatureStatus {
        let code = unsafe { ffi::rnp_op_verify_signature_get_status(self.handle) };
        if code == error::SUCCESS {
            SignatureStatus::Valid
        } else {
            // Map by category: Sig-validation range (0x14…) → Invalid;
            // Crypto range with no-signatures / unknown-signature → Unknown;
            // everything else → Invalid.
            match error::ErrorKind::from_code(code) {
                error::ErrorKind::NoSignaturesFound
                | error::ErrorKind::SignatureUnknown
                | error::ErrorKind::SigNoSignerKey
                | error::ErrorKind::SigNoSignerId => SignatureStatus::Unknown,
                _ => SignatureStatus::Invalid,
            }
        }
    }

    /// Hash algorithm used (e.g. `"SHA256"`).
    pub fn hash(&self) -> Result<String> {
        crate::ops::call_for_string(|raw| unsafe {
            ffi::rnp_op_verify_signature_get_hash(self.handle, raw)
        })
    }

    /// Creation and expiration times of the signature.
    pub fn times(&self) -> Result<(u32, u32)> {
        let mut create: u32 = 0;
        let mut expires: u32 = 0;
        unsafe {
            check(ffi::rnp_op_verify_signature_get_times(
                self.handle,
                &mut create,
                &mut expires,
            ))?;
        }
        Ok((create, expires))
    }

    /// The full [`Signature`] handle, for inspection of subpackets, key
    /// flags, and other metadata. The returned handle borrows `self`.
    pub fn handle(&self) -> Result<crate::Signature<'_>> {
        let mut raw: ffi::rnp_signature_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_verify_signature_get_handle(
                self.handle,
                &mut raw,
            ))?;
        }
        if raw.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(crate::Signature::from_handle(raw))
    }

    /// The signing key, when present in the keyring. Returns `None` if the
    /// signer's key wasn't loaded. The returned [`Key`] borrows `self`.
    pub fn key(&self) -> Result<Option<crate::Key<'_>>> {
        let mut raw: ffi::rnp_key_handle_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_verify_signature_get_key(self.handle, &mut raw))?;
        }
        if raw.is_null() {
            Ok(None)
        } else {
            Ok(Some(crate::Key::from_handle(raw)))
        }
    }

    /// Hex keyid of the signer, drawn from the underlying signature
    /// handle. Empty when the signer's key is unavailable and the
    /// signature itself doesn't carry a keyid.
    pub fn keyid(&self) -> Result<String> {
        let sig = self.handle()?;
        sig.keyid()
    }
}

/// Public-key recipient extracted from a [`VerifyResult`].
pub struct Recipient {
    handle: ffi::rnp_recipient_handle_t,
}

impl Recipient {
    /// Encryption algorithm used for this recipient's session key.
    pub fn alg(&self) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_recipient_get_alg(self.handle, &mut raw))?;
            cstr_to_string(raw)
        }
    }

    /// Hex keyid of the recipient's key.
    pub fn keyid(&self) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_recipient_get_keyid(self.handle, &mut raw))?;
            cstr_to_string(raw)
        }
    }
}

/// Symmetric-only (password-based) recipient extracted from a [`VerifyResult`].
pub struct Symenc {
    handle: ffi::rnp_symenc_handle_t,
}

impl Symenc {
    /// Symmetric cipher name used for this symenc.
    pub fn cipher(&self) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_symenc_get_cipher(self.handle, &mut raw))?;
            cstr_to_string(raw)
        }
    }

    /// AEAD algorithm name, if AEAD was used.
    pub fn aead_alg(&self) -> Result<Option<String>> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            let code = ffi::rnp_symenc_get_aead_alg(self.handle, &mut raw);
            if code == error::NOT_FOUND || raw.is_null() {
                return Ok(None);
            }
            check(code)?;
            Ok(cstr_to_optional_string(raw))
        }
    }

    /// Hash algorithm used in the S2K derivation.
    pub fn hash_alg(&self) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_symenc_get_hash_alg(self.handle, &mut raw))?;
            cstr_to_string(raw)
        }
    }

    /// S2K type string (`"simple"`, `"salted"`, `"iterated and salted"`).
    pub fn s2k_type(&self) -> Result<String> {
        let mut raw: *mut c_char = ptr::null_mut();
        unsafe {
            check(ffi::rnp_symenc_get_s2k_type(self.handle, &mut raw))?;
            cstr_to_string(raw)
        }
    }

    /// Number of S2K iterations.
    pub fn s2k_iterations(&self) -> Result<u32> {
        let mut n: u32 = 0;
        unsafe { check(ffi::rnp_symenc_get_s2k_iterations(self.handle, &mut n))? };
        Ok(n)
    }
}

/// File metadata from the literal-data packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInfo {
    pub name: String,
    pub mtime: u32,
}
