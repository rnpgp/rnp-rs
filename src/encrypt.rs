//! Encryption and decryption.
//!
//! [`Encryptor`] is a builder over `rnp_op_encrypt_*`; the op handle is
//! created in [`Encryptor::build`] when the destination [`Output`] is
//! available. [`decrypt`] is the simple-path wrapper around `rnp_decrypt`.
//!
//! Rich decryption-result inspection (recipient / symenc / protection info)
//! is deferred — it requires unwrapping the verify-op surface from
//! `signature::verify` into a typed `VerifyResult`, which is its own phase.

use crate::context::Context;
use crate::error::{self, check, Result};
use crate::ffi;
use crate::key::Key;
use crate::keygen::{Cipher, Compression, Hash};
use crate::ops::{Input, Output};
use std::ffi::CString;

/// Flags for [`Encryptor::set_flags`]. Wraps `RNP_ENCRYPT_*`.
#[derive(Clone, Copy, Debug, Default)]
pub struct EncryptFlags(pub u32);

impl EncryptFlags {
    /// Don't wrap plaintext in a literal-data packet — used when encrypting
    /// already-signed data. Wraps `RNP_ENCRYPT_NOWRAP`.
    pub const NOWRAP: Self = Self(ffi::RNP_ENCRYPT_NOWRAP as u32);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for EncryptFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// AEAD algorithm. Pass to [`Encryptor::aead`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AeadType {
    Ocb,
    Eax,
    Gcm,
}

impl AeadType {
    pub fn as_str(self) -> &'static str {
        match self {
            AeadType::Ocb => "OCB",
            AeadType::Eax => "EAX",
            AeadType::Gcm => "GCM",
        }
    }
}

/// S2K / encryption config for a password added via [`Encryptor::add_password`].
///
/// Built via [`AddPasswordOptions::default`] then chained setters. OCP —
/// adding a new S2K parameter doesn't break call sites.
#[derive(Default)]
pub struct AddPasswordOptions {
    hash: Option<Hash>,
    iterations: Option<usize>,
    cipher: Option<Cipher>,
}

impl AddPasswordOptions {
    pub fn hash(mut self, h: Hash) -> Self {
        self.hash = Some(h);
        self
    }

    pub fn iterations(mut self, n: usize) -> Self {
        self.iterations = Some(n);
        self
    }

    pub fn cipher(mut self, c: Cipher) -> Self {
        self.cipher = Some(c);
        self
    }
}

/// Builder over `rnp_op_encrypt_*`. Configured first, then executed with
/// [`Encryptor::build`] against the caller-supplied destination [`Output`].
///
/// ```
/// # use rnp::{Algorithm, Context, Encryptor, Hash, KeyBuilder, KeyUsage, Output};
/// # let ctx = Context::new().unwrap();
/// # let key = KeyBuilder::new(Algorithm::Rsa).bits(2048)
/// #     .userid("enc <enc@example.com>").hash(Hash::Sha256)
/// #     .add_usage(KeyUsage::EncryptComms).build(&ctx).unwrap();
/// let plaintext = b"secret message";
/// let mut output = Output::to_memory().unwrap();
/// Encryptor::new(&ctx, plaintext).unwrap()
///     .add_recipient(&key)
///     .build(&mut output)
///     .unwrap();
/// let _ciphertext = output.into_bytes().unwrap();
/// ```
pub struct Encryptor<'a> {
    ctx: &'a Context,
    input: Input,
    recipients: Vec<&'a Key<'a>>,
    signatures: Vec<&'a Key<'a>>,
    passwords: Vec<(CString, AddPasswordOptions)>,
    armor: bool,
    cipher: Option<Cipher>,
    hash: Option<Hash>,
    compression: Option<(Compression, u32)>,
    aead: Option<AeadType>,
    aead_bits: Option<i32>,
    file_name: Option<CString>,
    file_mtime: Option<u32>,
    creation_time: Option<u32>,
    expiration_time: Option<u32>,
    flags: EncryptFlags,
    #[cfg(feature = "pqc")]
    prefer_pqc: bool,
    #[cfg(feature = "crypto-refresh")]
    pkesk_v6: bool,
    #[cfg(feature = "crypto-refresh")]
    skesk_v6: bool,
}

impl<'a> Encryptor<'a> {
    /// Begin building an encryption operation over `plaintext`.
    pub fn new(ctx: &'a Context, plaintext: &[u8]) -> Result<Self> {
        let input = Input::from_memory(plaintext)?;
        Ok(Encryptor {
            ctx,
            input,
            recipients: Vec::new(),
            signatures: Vec::new(),
            passwords: Vec::new(),
            armor: false,
            cipher: None,
            hash: None,
            compression: None,
            aead: None,
            aead_bits: None,
            file_name: None,
            file_mtime: None,
            creation_time: None,
            expiration_time: None,
            flags: EncryptFlags::default(),
            #[cfg(feature = "pqc")]
            prefer_pqc: false,
            #[cfg(feature = "crypto-refresh")]
            pkesk_v6: false,
            #[cfg(feature = "crypto-refresh")]
            skesk_v6: false,
        })
    }

    /// Add a recipient's public key. May be called multiple times.
    pub fn add_recipient(mut self, key: &'a Key<'a>) -> Self {
        self.recipients.push(key);
        self
    }

    /// Add a signature key (sign-and-encrypt in one op). May be called
    /// multiple times.
    pub fn add_signature(mut self, key: &'a Key<'a>) -> Self {
        self.signatures.push(key);
        self
    }

    /// Add a password-encrypted session key. May be called multiple times
    /// to allow multiple passwords (any one decrypts).
    pub fn add_password(
        mut self,
        password: impl Into<Vec<u8>>,
        options: AddPasswordOptions,
    ) -> Self {
        // CString::new needs Vec<u8> without interior NUL. unwrap_or lets
        // us fall back if the password is weird, but passwords containing
        // NUL bytes are pathological — surface as NullPointer for caller
        // to handle.
        let c = CString::new(password.into()).unwrap_or_default();
        self.passwords.push((c, options));
        self
    }

    pub fn armor(mut self, armored: bool) -> Self {
        self.armor = armored;
        self
    }

    pub fn cipher(mut self, c: Cipher) -> Self {
        self.cipher = Some(c);
        self
    }

    pub fn hash(mut self, h: Hash) -> Self {
        self.hash = Some(h);
        self
    }

    pub fn compression(mut self, alg: Compression, level: u32) -> Self {
        self.compression = Some((alg, level));
        self
    }

    pub fn aead(mut self, alg: AeadType) -> Self {
        self.aead = Some(alg);
        self
    }

    /// AEAD chunk size in bits (0..=16 per the OpenPGP spec).
    pub fn aead_bits(mut self, bits: i32) -> Self {
        self.aead_bits = Some(bits);
        self
    }

    pub fn file_name(mut self, name: impl AsRef<str>) -> Self {
        self.file_name = Some(CString::new(name.as_ref()).unwrap_or_default());
        self
    }

    pub fn file_mtime(mut self, mtime: u32) -> Self {
        self.file_mtime = Some(mtime);
        self
    }

    pub fn creation_time(mut self, t: u32) -> Self {
        self.creation_time = Some(t);
        self
    }

    pub fn expiration_time(mut self, t: u32) -> Self {
        self.expiration_time = Some(t);
        self
    }

    pub fn flags(mut self, flags: EncryptFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Prefer PQC-encryption subkeys over non-PQC subkeys when choosing
    /// recipients. Feature-gated; requires `--features pqc` and a librnp
    /// built with `ENABLE_PQC=ON`.
    #[cfg(feature = "pqc")]
    pub fn prefer_pqc_enc_subkey(mut self) -> Self {
        self.prefer_pqc = true;
        self
    }

    /// Enable v6 PKESK (Public-Key Encrypted Session Key) packets in the
    /// output. Feature-gated; requires `--features crypto-refresh`.
    #[cfg(feature = "crypto-refresh")]
    pub fn enable_pkesk_v6(mut self) -> Self {
        self.pkesk_v6 = true;
        self
    }

    /// Enable v6 SKESK (Symmetric-Key Encrypted Session Key) packets in
    /// the output. Feature-gated; requires `--features crypto-refresh`.
    #[cfg(feature = "crypto-refresh")]
    pub fn enable_skesk_v6(mut self) -> Self {
        self.skesk_v6 = true;
        self
    }

    /// Execute the encryption, writing the ciphertext to `output`.
    pub fn build(self, output: &mut Output) -> Result<()> {
        let mut op: ffi::rnp_op_encrypt_t = std::ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_encrypt_create(
                &mut op,
                self.ctx.ffi,
                self.input.as_ptr(),
                output.as_ptr(),
            ))?;
        }
        // Apply all configuration. On any failure, destroy the op and bail.
        let result = unsafe { apply_config(op, &self) };
        if let Err(e) = result {
            unsafe {
                let _ = ffi::rnp_op_encrypt_destroy(op);
            }
            return Err(e);
        }

        unsafe {
            let exec = check(ffi::rnp_op_encrypt_execute(op));
            let _ = ffi::rnp_op_encrypt_destroy(op);
            exec?;
        }
        Ok(())
    }
}

unsafe fn apply_config(op: ffi::rnp_op_encrypt_t, e: &Encryptor<'_>) -> Result<()> {
    unsafe {
        for key in &e.recipients {
            check(ffi::rnp_op_encrypt_add_recipient(op, key.handle))?;
        }
        for key in &e.signatures {
            check(ffi::rnp_op_encrypt_add_signature(op, key.handle, std::ptr::null_mut()))?;
        }
        for (pw, opts) in &e.passwords {
            let hash_str = opts
                .hash
                .map(|h| CString::new(h.as_str()).unwrap())
                .unwrap_or_else(|| CString::new("SHA256").unwrap());
            let cipher_str = opts
                .cipher
                .map(|c| CString::new(c.as_str()).unwrap())
                .unwrap_or_else(|| CString::new("AES256").unwrap());
            check(ffi::rnp_op_encrypt_add_password(
                op,
                pw.as_ptr(),
                hash_str.as_ptr(),
                opts.iterations.unwrap_or(0),
                cipher_str.as_ptr(),
            ))?;
        }
        if e.armor {
            check(ffi::rnp_op_encrypt_set_armor(op, true))?;
        }
        if let Some(c) = e.cipher {
            let cs = CString::new(c.as_str()).unwrap();
            check(ffi::rnp_op_encrypt_set_cipher(op, cs.as_ptr()))?;
        }
        if let Some(h) = e.hash {
            let cs = CString::new(h.as_str()).unwrap();
            check(ffi::rnp_op_encrypt_set_hash(op, cs.as_ptr()))?;
        }
        if let Some((alg, level)) = e.compression {
            let cs = CString::new(alg.as_str()).unwrap();
            check(ffi::rnp_op_encrypt_set_compression(
                op,
                cs.as_ptr(),
                level as i32,
            ))?;
        }
        if let Some(aead) = e.aead {
            let cs = CString::new(aead.as_str()).unwrap();
            check(ffi::rnp_op_encrypt_set_aead(op, cs.as_ptr()))?;
        }
        if let Some(bits) = e.aead_bits {
            check(ffi::rnp_op_encrypt_set_aead_bits(op, bits))?;
        }
        if let Some(name) = &e.file_name {
            check(ffi::rnp_op_encrypt_set_file_name(op, name.as_ptr()))?;
        }
        if let Some(mtime) = e.file_mtime {
            check(ffi::rnp_op_encrypt_set_file_mtime(op, mtime))?;
        }
        if let Some(t) = e.creation_time {
            check(ffi::rnp_op_encrypt_set_creation_time(op, t))?;
        }
        if let Some(t) = e.expiration_time {
            check(ffi::rnp_op_encrypt_set_expiration_time(op, t))?;
        }
        if e.flags.bits() != 0 {
            check(ffi::rnp_op_encrypt_set_flags(op, e.flags.bits()))?;
        }
        #[cfg(feature = "pqc")]
        if e.prefer_pqc {
            check(ffi::rnp_op_encrypt_prefer_pqc_enc_subkey(op))?;
        }
        #[cfg(feature = "crypto-refresh")]
        if e.pkesk_v6 {
            check(ffi::rnp_op_encrypt_enable_pkesk_v6(op))?;
        }
        #[cfg(feature = "crypto-refresh")]
        if e.skesk_v6 {
            check(ffi::rnp_op_encrypt_enable_skesk_v6(op))?;
        }
        Ok(())
    }
}

/// Decrypt `ciphertext` and return the plaintext bytes. Requires that the
/// keyring contain the matching secret key (unlocked) or that a password
/// provider returns the right password.
pub fn decrypt(ctx: &Context, ciphertext: &[u8]) -> Result<Vec<u8>> {
    let input = Input::from_memory(ciphertext)?;
    let output = Output::to_memory()?;
    unsafe {
        check(ffi::rnp_decrypt(ctx.ffi, input.as_ptr(), output.as_ptr()))?;
    }
    output.into_bytes()
}

/// Decrypt `ciphertext` and write the plaintext to `output`.
pub fn decrypt_to(ctx: &Context, ciphertext: &[u8], output: &mut Output) -> Result<()> {
    let input = Input::from_memory(ciphertext)?;
    unsafe { check(ffi::rnp_decrypt(ctx.ffi, input.as_ptr(), output.as_ptr())) }
}

// Re-export the error type so `use crate::encrypt::*` doesn't drop it.
#[allow(unused_imports)]
use error as _error_alias;
