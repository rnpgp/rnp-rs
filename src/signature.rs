//! Signing and verification (high-level convenience functions).
//!
//! The richer verify-op surface lives in [`crate::verify`] — these free
//! functions are thin wrappers for the common "did it verify?" question.

use crate::context::Context;
use crate::error::{self, check, Result};
use crate::ffi;
use crate::key::Key;
use crate::ops::{Input, Output};
use crate::verify::{VerifyOp, VerifyResult};

/// Sign `message` with `signing_key`, producing an inline-signed OpenPGP
/// message (the signature is embedded in the output).
pub fn sign(ctx: &Context, message: &[u8], signing_key: &Key<'_>) -> Result<Vec<u8>> {
    sign_impl(ctx, message, signing_key, Mode::Inline)
}

/// Sign `message`, producing only the detached signature bytes.
pub fn sign_detached(
    ctx: &Context,
    message: &[u8],
    signing_key: &Key<'_>,
) -> Result<Vec<u8>> {
    sign_impl(ctx, message, signing_key, Mode::Detached)
}

fn sign_impl(
    ctx: &Context,
    message: &[u8],
    signing_key: &Key<'_>,
    mode: Mode,
) -> Result<Vec<u8>> {
    let input = Input::from_memory(message)?;
    let output = Output::to_memory()?;

    let mut op: ffi::rnp_op_sign_t = std::ptr::null_mut();
    unsafe {
        let create = match mode {
            Mode::Inline => check(ffi::rnp_op_sign_create(
                &mut op,
                ctx.ffi,
                input.as_ptr(),
                output.as_ptr(),
            )),
            Mode::Detached => check(ffi::rnp_op_sign_detached_create(
                &mut op,
                ctx.ffi,
                input.as_ptr(),
                output.as_ptr(),
            )),
        };
        if let Err(e) = create {
            if !op.is_null() {
                let _ = ffi::rnp_op_sign_destroy(op);
            }
            return Err(e);
        }

        check(ffi::rnp_op_sign_add_signature(
            op,
            signing_key.handle,
            std::ptr::null_mut(),
        ))?;

        let _ = ffi::rnp_op_sign_set_hash(op, cstr(b"SHA256\0").as_ptr());
        let _ = ffi::rnp_op_sign_set_armor(op, false);

        let exec_res = check(ffi::rnp_op_sign_execute(op));
        let _ = ffi::rnp_op_sign_destroy(op);
        exec_res?;
    }
    output.into_bytes()
}

/// Verify an inline-signed message. Returns `true` if at least one
/// signature over the embedded data verified successfully.
pub fn verify(ctx: &Context, signed_message: &[u8]) -> Result<bool> {
    let null_out = Output::to_null()?;
    let op = VerifyOp::inline(ctx, signed_message, null_out)?;
    verify_any_valid(op.execute()?)
}

/// Verify a detached signature against a message. Returns `true` if the
/// signature is valid.
pub fn verify_detached(
    ctx: &Context,
    message: &[u8],
    signature: &[u8],
) -> Result<bool> {
    let op = VerifyOp::detached(ctx, message, signature)?;
    verify_any_valid(op.execute()?)
}

fn verify_any_valid(result: VerifyResult<'_>) -> Result<bool> {
    if result.any_valid()? {
        Ok(true)
    } else {
        Err(error::no_signatures_error())
    }
}

#[derive(Clone, Copy)]
enum Mode {
    Inline,
    Detached,
}

fn cstr(bytes: &[u8]) -> std::ffi::CString {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    std::ffi::CString::new(&bytes[..end]).expect("static C string without NUL")
}
