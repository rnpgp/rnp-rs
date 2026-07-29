//! [`VerifyOp`] — builder over `rnp_op_verify_*`.

use crate::context::Context;
use crate::error::{Result, check};
use crate::ffi;
use crate::ops::{Input, Output};
use std::marker::PhantomData;
use std::ptr;

use super::VerifyResult;

/// Flags for [`VerifyOp::set_flags`]. Wraps `RNP_VERIFY_*`.
#[derive(Clone, Copy, Debug, Default)]
pub struct VerifyFlags(pub u32);

impl VerifyFlags {
    /// Don't inspect signatures during decrypt.
    pub const IGNORE_SIGS_ON_DECRYPT: Self = Self(ffi::RNP_VERIFY_IGNORE_SIGS_ON_DECRYPT as u32);
    /// Require all signatures to verify.
    pub const REQUIRE_ALL_SIGS: Self = Self(ffi::RNP_VERIFY_REQUIRE_ALL_SIGS as u32);
    /// Allow hidden (all-zero keyid) recipients.
    pub const ALLOW_HIDDEN_RECIPIENT: Self = Self(ffi::RNP_VERIFY_ALLOW_HIDDEN_RECIPIENT as u32);

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
    pub fn inline(ctx: &'ctx Context, signed_message: &[u8], output: Output) -> Result<Self> {
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
        // The op destroys both inputs via rnp_op_verify_destroy. We forget
        // sig_input so our Drop doesn't double-free; msg_input is kept
        // alive by self._input.
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
