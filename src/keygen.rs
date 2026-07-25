//! Key generation.
//!
//! Currently provides a convenience function for the common case used in
//! tests: generate an unprotected RSA signing key with a given userid.
//! General key generation via the JSON API (`rnp_generate_key_json`) is a
//! TODO.

use crate::context::Context;
use crate::error::{self, check, Result};
use crate::ffi;
use crate::key::Key;
use std::ffi::CString;
use std::ptr;

/// Generate an unprotected RSA keypair (bits=2048) usable for signing.
///
/// The key is added to the context's keyring; the returned [`Key`] borrows the
/// context. The secret material is stored in cleartext (no password), which
/// makes this suitable for tests and throw-away identities only.
// TODO: generalize via rnp_generate_key_json / rnp_op_generate_*.
pub fn generate_test_key<'a>(ctx: &'a Context, userid: &str) -> Result<Key<'a>> {
    let uid_c = CString::new(userid).map_err(|_| error::Error::NulByte)?;
    let mut handle: ffi::rnp_key_handle_t = ptr::null_mut();
    unsafe {
        check(ffi::rnp_generate_key_rsa(
            ctx.ffi,
            2048,
            2048,
            uid_c.as_ptr(),
            ptr::null(), // no password — unprotected
            &mut handle,
        ))?;
        if handle.is_null() {
            return Err(error::Error::NullPointer);
        }
    }
    Ok(Key::from_handle(handle))
}
