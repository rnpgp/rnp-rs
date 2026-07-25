//! Signing and verification.
//!
//! Covers the two operations Confium needs first: signing a message with a
//! secret key, and verifying a (possibly-detached) signature with a public
//! key. Both wrap `rnp_op_sign_*` / `rnp_op_verify_*`.

use crate::context::Context;
use crate::error::{self, check, Result};
use crate::ffi;
use crate::key::Key;
use std::ptr;

/// Sign `message` with `signing_key`, producing an inline-signed OpenPGP
/// message (the signature is embedded in the output).
///
/// The signing key must be present in the context's keyring and (if secret
/// material is encrypted) unlocked via a configured password provider.
pub fn sign(ctx: &Context, message: &[u8], signing_key: &Key<'_>) -> Result<Vec<u8>> {
    sign_detached_or_inline(ctx, message, signing_key, Mode::Inline)
}

/// Sign `message`, producing only the detached signature bytes.
pub fn sign_detached(
    ctx: &Context,
    message: &[u8],
    signing_key: &Key<'_>,
) -> Result<Vec<u8>> {
    sign_detached_or_inline(ctx, message, signing_key, Mode::Detached)
}

fn sign_detached_or_inline(
    ctx: &Context,
    message: &[u8],
    signing_key: &Key<'_>,
    mode: Mode,
) -> Result<Vec<u8>> {
    let mut input: ffi::rnp_input_t = ptr::null_mut();
    let mut output: ffi::rnp_output_t = ptr::null_mut();
    let mut op: ffi::rnp_op_sign_t = ptr::null_mut();

    unsafe {
        check(ffi::rnp_input_from_memory(
            &mut input,
            message.as_ptr(),
            message.len(),
            true,
        ))?;
        check(ffi::rnp_output_to_memory(&mut output, 0))?;

        // Create the operation. If creation fails we still own input/output
        // and must clean them up.
        let create_res = match mode {
            Mode::Inline => {
                check(ffi::rnp_op_sign_create(&mut op, ctx.ffi, input, output))
            }
            Mode::Detached => {
                check(ffi::rnp_op_sign_detached_create(&mut op, ctx.ffi, input, output))
            }
        };
        if let Err(e) = create_res {
            cleanup_sign(op, input, output);
            return Err(e);
        }

        // Attach the signing key. We discard the per-signature handle: it is
        // owned by the op and destroyed with it.
        if let Err(e) = check(ffi::rnp_op_sign_add_signature(op, signing_key.handle, ptr::null_mut())) {
            cleanup_sign(op, input, output);
            return Err(e);
        }

        // Default to a non-armored, SHA-256 signature. Override the hash so
        // behavior is deterministic regardless of librnp defaults.
        let _ = ffi::rnp_op_sign_set_hash(op, cstr(b"SHA256\0").as_ptr());
        let _ = ffi::rnp_op_sign_set_armor(op, false);

        let exec_res = check(ffi::rnp_op_sign_execute(op));

        // Destroy the op first so any in-flight output is flushed.
        let _ = ffi::rnp_op_sign_destroy(op);
        let _ = ffi::rnp_input_destroy(input);

        let drain = drain_memory_output(output);
        exec_res.and(drain)
    }
}

/// Verify an inline-signed message produced by [`sign`]. Returns `true` if at
/// least one signature over the embedded data verified successfully.
pub fn verify(ctx: &Context, signed_message: &[u8]) -> Result<bool> {
    // Inline verification: input is the signed message, output is discarded.
    let mut input: ffi::rnp_input_t = ptr::null_mut();
    let mut output: ffi::rnp_output_t = ptr::null_mut();
    let mut op: ffi::rnp_op_verify_t = ptr::null_mut();
    unsafe {
        check(ffi::rnp_input_from_memory(
            &mut input,
            signed_message.as_ptr(),
            signed_message.len(),
            true,
        ))?;
        check(ffi::rnp_output_to_null(&mut output))?;
        if let Err(e) = check(ffi::rnp_op_verify_create(&mut op, ctx.ffi, input, output)) {
            let _ = ffi::rnp_input_destroy(input);
            let _ = ffi::rnp_output_destroy(output);
            return Err(e);
        }
        let verified = verify_op_status(op);
        let _ = ffi::rnp_op_verify_destroy(op);
        let _ = ffi::rnp_input_destroy(input);
        let _ = ffi::rnp_output_destroy(output);
        verified
    }
}

/// Verify a detached signature against a message. Returns `true` if the
/// signature is valid.
pub fn verify_detached(
    ctx: &Context,
    message: &[u8],
    signature: &[u8],
) -> Result<bool> {
    let mut msg_input: ffi::rnp_input_t = ptr::null_mut();
    let mut sig_input: ffi::rnp_input_t = ptr::null_mut();
    let mut op: ffi::rnp_op_verify_t = ptr::null_mut();
    unsafe {
        check(ffi::rnp_input_from_memory(
            &mut msg_input,
            message.as_ptr(),
            message.len(),
            true,
        ))?;
        check(ffi::rnp_input_from_memory(
            &mut sig_input,
            signature.as_ptr(),
            signature.len(),
            true,
        ))?;
        if let Err(e) = check(ffi::rnp_op_verify_detached_create(
            &mut op,
            ctx.ffi,
            msg_input,
            sig_input,
        )) {
            let _ = ffi::rnp_input_destroy(msg_input);
            let _ = ffi::rnp_input_destroy(sig_input);
            return Err(e);
        }
        let verified = verify_op_status(op);
        let _ = ffi::rnp_op_verify_destroy(op);
        let _ = ffi::rnp_input_destroy(msg_input);
        let _ = ffi::rnp_input_destroy(sig_input);
        verified
    }
}

/// Inspect the result of a completed verify op: count signatures, and require
/// at least one to be valid.
///
/// `rnp_op_verify_execute` returns `RNP_SUCCESS` for inline ops when at least
/// one signature is valid (or data decrypted cleanly), but for robustness we
/// walk the per-signature statuses explicitly.
unsafe fn verify_op_status(op: ffi::rnp_op_verify_t) -> Result<bool> {
    // Edition 2024: wrap body so unsafe ops are admitted.
    unsafe {
        let exec = check(ffi::rnp_op_verify_execute(op));
        // Whether execute failed or not, walk signature handles: a "no
        // signatures found" outcome shouldn't be reported as a valid verify.
        let mut count: usize = 0;
        let _ = ffi::rnp_op_verify_get_signature_count(op, &mut count);
        let mut any_valid = false;
        for i in 0..count {
            let mut sig: ffi::rnp_op_verify_signature_t = ptr::null_mut();
            if ffi::rnp_op_verify_get_signature_at(op, i, &mut sig) != error::SUCCESS {
                continue;
            }
            if sig.is_null() {
                continue;
            }
            // rnp_op_verify_signature_get_status returns RNP_SUCCESS for valid.
            if ffi::rnp_op_verify_signature_get_status(sig) == error::SUCCESS {
                any_valid = true;
            }
        }
        if any_valid {
            return Ok(true);
        }
        // No valid signatures. If execute itself errored, surface that;
        // otherwise report a clean "signature invalid".
        match exec {
            Ok(()) => Err(error::Error::SignatureInvalid),
            Err(e) => Err(e),
        }
    }
}

#[derive(Clone, Copy)]
enum Mode {
    Inline,
    Detached,
}

unsafe fn drain_memory_output(output: ffi::rnp_output_t) -> Result<Vec<u8>> {
    unsafe {
        let mut buf: *mut u8 = ptr::null_mut();
        let mut len: usize = 0;
        let res = check(ffi::rnp_output_memory_get_buf(output, &mut buf, &mut len, true));
        let out = if res.is_ok() {
            if buf.is_null() || len == 0 {
                Ok(Vec::new())
            } else {
                let v = std::slice::from_raw_parts(buf, len).to_vec();
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

unsafe fn cleanup_sign(
    op: ffi::rnp_op_sign_t,
    input: ffi::rnp_input_t,
    output: ffi::rnp_output_t,
) {
    unsafe {
        if !op.is_null() {
            let _ = ffi::rnp_op_sign_destroy(op);
        }
        let _ = ffi::rnp_input_destroy(input);
        let _ = ffi::rnp_output_destroy(output);
    }
}

fn cstr(bytes: &[u8]) -> std::ffi::CString {
    // Caller includes the trailing NUL; trim if present then build.
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    std::ffi::CString::new(&bytes[..end]).expect("static C string without NUL")
}
