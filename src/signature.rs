//! Signing and verification.
//!
//! Two surfaces:
//!
//! - **Free-function shortcuts**: [`sign`], [`sign_detached`], [`sign_cleartext`],
//!   [`verify`], [`verify_detached`]. One-liners for the common case.
//! - **Builder**: [`Signer`] for multi-signer, per-signer hash, or output
//!   destination control. The free functions are thin shims over `Signer`.
//!
//! [`generate_revocation_certificate`] produces a standalone revocation
//! certificate (RFC 4880 type 0x20) without needing the secret key.

use crate::algorithm::Hash;
use crate::context::Context;
use crate::error::{check, Result};
use crate::ffi;
use crate::key::Key;
use crate::ops::{Input, Output};
use crate::verify::{VerifyOp, VerifyResult};
use std::ffi::CString;
use std::ptr;

// ---------------------------------------------------------------------------
// Signing mode + builder
// ---------------------------------------------------------------------------

/// Signing mode. Selects how the signature is packaged with the message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Signature is embedded inline in the OpenPGP message stream.
    Inline,
    /// Only the signature bytes are produced; the message stays untouched.
    Detached,
    /// Cleartext-signed message: plaintext remains readable, signature is
    /// appended as an ASCII-armored block (RFC 4880 §7).
    Cleartext,
}

/// Per-signer configuration. Internal — collected by [`Signer::add_signer`]
/// / [`Signer::add_signer_with_hash`] and replayed at build time.
struct SignerSpec {
    handle: ffi::rnp_key_handle_t,
    hash: Option<Hash>,
    creation_time: Option<u32>,
    expiration_time: Option<u32>,
}

/// Builder over `rnp_op_sign_*`. Unifies inline / detached / cleartext
/// signing behind one surface — new modes add a [`Mode`] variant, not a
/// new top-level function.
///
/// Terminal methods:
/// - [`Signer::build`] writes to a caller-supplied [`Output`].
/// - [`Signer::build_to_memory`] returns the signed bytes.
///
/// ```
/// # use rnp::{Context, Hash, KeyBuilder, KeyUsage, Mode, Algorithm, Signer};
/// # let ctx = Context::new().unwrap();
/// # let key = KeyBuilder::new(Algorithm::Rsa).bits(2048)
/// #     .userid("alice <a@example.com>").add_usage(KeyUsage::Sign).build(&ctx).unwrap();
/// let signed = Signer::new(&ctx, b"hello", Mode::Detached)
///     .add_signer(&key)
///     .armor(true)
///     .hash(Hash::Sha256)
///     .build_to_memory()
///     .unwrap();
/// ```
pub struct Signer<'a, 'ctx> {
    ctx: &'ctx Context,
    message: &'a [u8],
    mode: Mode,
    signers: Vec<SignerSpec>,
    armor: bool,
    default_hash: Hash,
}

impl<'a, 'ctx> Signer<'a, 'ctx> {
    pub fn new(ctx: &'ctx Context, message: &'a [u8], mode: Mode) -> Self {
        Signer {
            ctx,
            message,
            mode,
            signers: Vec::new(),
            armor: false,
            default_hash: Hash::Sha256,
        }
    }

    /// Add a signer using the builder's default hash.
    pub fn add_signer(mut self, key: &Key<'_>) -> Self {
        self.signers.push(SignerSpec {
            handle: key.handle,
            hash: None,
            creation_time: None,
            expiration_time: None,
        });
        self
    }

    /// Add a signer with a per-signer hash override.
    pub fn add_signer_with_hash(mut self, key: &Key<'_>, hash: Hash) -> Self {
        self.signers.push(SignerSpec {
            handle: key.handle,
            hash: Some(hash),
            creation_time: None,
            expiration_time: None,
        });
        self
    }

    /// Add a signer with per-signer hash, creation time, and expiration
    /// overrides. Wraps `rnp_op_sign_signature_set_hash`,
    /// `rnp_op_sign_signature_set_creation`, and
    /// `rnp_op_sign_signature_set_expiration`.
    pub fn add_signer_with_options(
        mut self,
        key: &Key<'_>,
        hash: Hash,
        creation_time: Option<u32>,
        expiration_time: Option<u32>,
    ) -> Self {
        self.signers.push(SignerSpec {
            handle: key.handle,
            hash: Some(hash),
            creation_time,
            expiration_time,
        });
        self
    }

    /// Toggle ASCII-armored output. Default: binary.
    pub fn armor(mut self, on: bool) -> Self {
        self.armor = on;
        self
    }

    /// Set the default hash for signers added via [`Self::add_signer`].
    /// Signers added via [`Self::add_signer_with_hash`] keep their own hash.
    pub fn hash(mut self, h: Hash) -> Self {
        self.default_hash = h;
        self
    }

    /// Execute the signing operation, writing the output to `output`.
    pub fn build(self, output: &mut Output) -> Result<()> {
        if self.signers.is_empty() {
            return Err(crate::error::Error::NullPointer);
        }
        let input = Input::from_memory(self.message)?;
        self.execute(input.as_ptr(), output.as_ptr())
    }

    /// Execute the signing operation, returning the output bytes.
    pub fn build_to_memory(self) -> Result<Vec<u8>> {
        if self.signers.is_empty() {
            return Err(crate::error::Error::NullPointer);
        }
        let input = Input::from_memory(self.message)?;
        let output = Output::to_memory()?;
        self.execute(input.as_ptr(), output.as_ptr())?;
        output.into_bytes()
    }

    fn execute(self, input: ffi::rnp_input_t, output: ffi::rnp_output_t) -> Result<()> {
        let mut op: ffi::rnp_op_sign_t = ptr::null_mut();
        unsafe {
            let create = match self.mode {
                Mode::Inline => check(ffi::rnp_op_sign_create(
                    &mut op,
                    self.ctx.ffi,
                    input,
                    output,
                )),
                Mode::Detached => check(ffi::rnp_op_sign_detached_create(
                    &mut op,
                    self.ctx.ffi,
                    input,
                    output,
                )),
                Mode::Cleartext => check(ffi::rnp_op_sign_cleartext_create(
                    &mut op,
                    self.ctx.ffi,
                    input,
                    output,
                )),
            };
            if let Err(e) = create {
                if !op.is_null() {
                    let _ = ffi::rnp_op_sign_destroy(op);
                }
                return Err(e);
            }

            for spec in &self.signers {
                let mut sig_handle: ffi::rnp_op_sign_signature_t = ptr::null_mut();
                check(ffi::rnp_op_sign_add_signature(op, spec.handle, &mut sig_handle))?;
                let hash = spec.hash.unwrap_or(self.default_hash);
                let hash_c = CString::new(hash.as_str()).unwrap();
                let _ = ffi::rnp_op_sign_signature_set_hash(sig_handle, hash_c.as_ptr());
                if let Some(ct) = spec.creation_time {
                    let _ = ffi::rnp_op_sign_signature_set_creation_time(sig_handle, ct);
                }
                if let Some(et) = spec.expiration_time {
                    let _ = ffi::rnp_op_sign_signature_set_expiration_time(sig_handle, et);
                }
            }

            let hash_c = CString::new(self.default_hash.as_str()).unwrap();
            let _ = ffi::rnp_op_sign_set_hash(op, hash_c.as_ptr());
            let _ = ffi::rnp_op_sign_set_armor(op, self.armor);

            let exec_res = check(ffi::rnp_op_sign_execute(op));
            let _ = ffi::rnp_op_sign_destroy(op);
            exec_res?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Free-function shortcuts
// ---------------------------------------------------------------------------

/// Sign `message` with `signing_key`, producing an inline-signed OpenPGP
/// message (the signature is embedded in the output). Equivalent to
/// `Signer::new(ctx, message, Mode::Inline).add_signer(signing_key).build_to_memory()`.
pub fn sign(ctx: &Context, message: &[u8], signing_key: &Key<'_>) -> Result<Vec<u8>> {
    Signer::new(ctx, message, Mode::Inline)
        .add_signer(signing_key)
        .build_to_memory()
}

/// Sign `message`, producing only the detached signature bytes.
pub fn sign_detached(
    ctx: &Context,
    message: &[u8],
    signing_key: &Key<'_>,
) -> Result<Vec<u8>> {
    Signer::new(ctx, message, Mode::Detached)
        .add_signer(signing_key)
        .build_to_memory()
}

/// Sign `message` as a cleartext-signed message. The plaintext remains
/// human-readable; the signature is appended as an ASCII-armored block.
pub fn sign_cleartext(
    ctx: &Context,
    message: &[u8],
    signing_key: &Key<'_>,
) -> Result<Vec<u8>> {
    Signer::new(ctx, message, Mode::Cleartext)
        .add_signer(signing_key)
        .build_to_memory()
}

// ---------------------------------------------------------------------------
// Verify (free functions; VerifyOp is the rich surface)
// ---------------------------------------------------------------------------

/// Verify an inline-signed message. Returns a [`VerifyResult`] — callers
/// who just need "did at least one signature verify?" use
/// `result.any_valid()`.
pub fn verify<'ctx>(ctx: &'ctx Context, signed_message: &[u8]) -> Result<VerifyResult<'ctx>> {
    let null_out = Output::to_null()?;
    let op = VerifyOp::inline(ctx, signed_message, null_out)?;
    op.execute()
}

/// Verify a detached signature against a message. Returns a
/// [`VerifyResult`] — use `result.any_valid()` for the boolean shortcut.
pub fn verify_detached<'ctx>(
    ctx: &'ctx Context,
    message: &[u8],
    signature: &[u8],
) -> Result<VerifyResult<'ctx>> {
    let op = VerifyOp::detached(ctx, message, signature)?;
    op.execute()
}

// ---------------------------------------------------------------------------
// Revocation certificate
// ---------------------------------------------------------------------------

/// Generate a standalone revocation certificate for `key`. The returned
/// bytes are a complete, publishable revocation certificate (RFC 4880
/// type 0x20) that can be uploaded to keyservers or stored for escrow
/// without needing the secret key.
///
/// This is the convenience wrapper for
/// [`Key::export_revocation`](crate::Key::export_revocation). It uses
/// SHA-256 and no armor by default; pass `ExportFlags::ARMORED` for
/// ASCII-armored output.
pub fn generate_revocation_certificate(
    _ctx: &Context,
    key: &Key<'_>,
    reason: crate::key::RevocationReason,
) -> Result<Vec<u8>> {
    key.export_revocation(crate::key::ExportFlags::default(), reason, Hash::Sha256)
}

/// Like [`generate_revocation_certificate`] but with explicit flags and
/// hash. Use when you need armored output or a non-default hash.
pub fn generate_revocation_certificate_with(
    key: &Key<'_>,
    flags: crate::key::ExportFlags,
    reason: crate::key::RevocationReason,
    hash: Hash,
) -> Result<Vec<u8>> {
    key.export_revocation(flags, reason, hash)
}
