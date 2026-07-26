//! Key generation.
//!
//! Two builders over `rnp_op_generate_*`:
//!
//! - [`KeyBuilder`](crate::KeyBuilder) for primary keys (in [`primary`]).
//! - [`SubkeyBuilder`](crate::SubkeyBuilder) for subkeys (in [`subkey`]).
//!
//! Plus the [`generate_key_json`] shortcut and the deprecated
//! [`generate_test_key`] shim.
//!
//! ## Module layout
//!
//! | Sub-module | Concern                                               |
//! |------------|-------------------------------------------------------|
//! | `primary`  | `KeyBuilder` + `apply_setters`                        |
//! | `subkey`   | `SubkeyBuilder` + `apply_subkey_setters`              |
//!
//! The algorithm/curve/hash/cipher/compression/key-usage enums live in
//! [`crate::algorithm`] and are re-exported here for backward compatibility.

use crate::context::Context;
use crate::error::{self, check, Result};
use crate::ffi;
use crate::key::Key;
use std::ffi::CString;

// Backward compatibility: re-export the algorithm enums from this module.
pub use crate::algorithm::{Algorithm, Cipher, Compression, Curve, Hash, KeyUsage};
#[cfg(feature = "pqc")]
pub use crate::algorithm::{librnp_supports_pqc, PqcAlgorithm};

pub(crate) mod primary;
pub(crate) mod subkey;

pub use primary::KeyBuilder;
pub use subkey::SubkeyBuilder;

// ---------------------------------------------------------------------------
// Shared protection config (used by both primary and subkey builders).
// ---------------------------------------------------------------------------

/// Protection configuration shared between `Key::protect` (phase 03) and
/// `KeyBuilder::protection` (phase 12). One canonical type — adding a new
/// field here updates both call sites. OCP via composition.
#[derive(Default, Clone)]
pub(crate) struct ProtectConfig {
    pub password: Option<String>,
    pub cipher: Option<Cipher>,
    pub mode: Option<String>,
    pub hash: Option<Hash>,
    pub iterations: Option<usize>,
}

impl ProtectConfig {
    /// Build from a user-supplied `ProtectOptions`.
    pub(crate) fn from_options(opts: &crate::key::ProtectOptions) -> Self {
        ProtectConfig {
            password: opts.password.clone(),
            cipher: opts.cipher,
            mode: opts.mode.clone(),
            hash: opts.hash,
            iterations: opts.iterations,
        }
    }
}

/// Apply a `ProtectConfig` to a `rnp_op_generate_t`. Shared by primary-key
/// and subkey generation.
pub(crate) unsafe fn apply_protection(
    op: ffi::rnp_op_generate_t,
    cfg: &ProtectConfig,
) -> Result<()> {
    unsafe {
        if let Some(pw) = &cfg.password {
            let c = CString::new(pw.as_str()).map_err(|_| error::Error::NulByte)?;
            check(ffi::rnp_op_generate_set_protection_password(op, c.as_ptr()))?;
        }
        if let Some(c2) = cfg.cipher {
            let cs = CString::new(c2.as_str()).unwrap();
            check(ffi::rnp_op_generate_set_protection_cipher(op, cs.as_ptr()))?;
        }
        if let Some(h) = cfg.hash {
            let cs = CString::new(h.as_str()).unwrap();
            check(ffi::rnp_op_generate_set_protection_hash(op, cs.as_ptr()))?;
        }
        if let Some(m) = &cfg.mode {
            let cs = CString::new(m.as_str()).unwrap();
            check(ffi::rnp_op_generate_set_protection_mode(op, cs.as_ptr()))?;
        }
        if let Some(it) = cfg.iterations {
            check(ffi::rnp_op_generate_set_protection_iterations(
                op,
                it.try_into().unwrap_or(u32::MAX),
            ))?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// JSON shortcut
// ---------------------------------------------------------------------------

/// Generate one or more keys from a JSON description. Returns the result
/// JSON (containing the generated key fingerprints).
///
/// See `rnp_generate_key_json` in librnp for the input JSON schema.
pub fn generate_key_json(ctx: &Context, json: &str) -> Result<String> {
    let c = CString::new(json).map_err(|_| error::Error::NulByte)?;
    crate::ffi_safe::call_for_string(|out| unsafe {
        ffi::rnp_generate_key_json(ctx.ffi, c.as_ptr(), out)
    })
}

// ---------------------------------------------------------------------------
// Deprecated test helper (kept as a pub(crate) shim over KeyBuilder for any
// in-crate callers; not exported beyond the crate boundary).
// ---------------------------------------------------------------------------

/// Generate an unprotected RSA-2048 keypair. New code should use
/// [`KeyBuilder`](crate::KeyBuilder) directly.
#[deprecated(note = "use KeyBuilder instead")]
#[allow(dead_code)]
pub(crate) fn generate_test_key<'a>(ctx: &'a Context, userid: &str) -> Result<Key<'a>> {
    KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid(userid)
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .add_usage(KeyUsage::Certify)
        .build(ctx)
}
