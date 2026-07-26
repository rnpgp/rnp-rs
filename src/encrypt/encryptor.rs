//! [`Encryptor`] — builder over `rnp_op_encrypt_*`.

use crate::algorithm::{Cipher, Compression, Hash};
use crate::context::Context;
use crate::error::{check, Result};
use crate::ffi;
use crate::key::Key;
use crate::ops::{Input, Output};
use std::ffi::CString;
use std::ptr;

use super::{AddPasswordOptions, AeadType, EncryptFlags};

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
    pub(crate) ctx: &'a Context,
    pub(crate) input: Input,
    pub(crate) recipients: Vec<&'a Key<'a>>,
    pub(crate) signatures: Vec<&'a Key<'a>>,
    pub(crate) passwords: Vec<(CString, AddPasswordOptions)>,
    pub(crate) armor: bool,
    pub(crate) cipher: Option<Cipher>,
    pub(crate) hash: Option<Hash>,
    pub(crate) compression: Option<(Compression, u32)>,
    pub(crate) aead: Option<AeadType>,
    pub(crate) aead_bits: Option<i32>,
    pub(crate) file_name: Option<CString>,
    pub(crate) file_mtime: Option<u32>,
    pub(crate) creation_time: Option<u32>,
    pub(crate) expiration_time: Option<u32>,
    pub(crate) flags: EncryptFlags,
    #[cfg(feature = "pqc")]
    pub(crate) prefer_pqc: bool,
    #[cfg(feature = "crypto-refresh")]
    pub(crate) pkesk_v6: bool,
    #[cfg(feature = "crypto-refresh")]
    pub(crate) skesk_v6: bool,
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
        let mut op: ffi::rnp_op_encrypt_t = ptr::null_mut();
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

pub(crate) unsafe fn apply_config(op: ffi::rnp_op_encrypt_t, e: &Encryptor<'_>) -> Result<()> {
    unsafe {
        for key in &e.recipients {
            check(ffi::rnp_op_encrypt_add_recipient(op, key.handle))?;
        }
        for key in &e.signatures {
            check(ffi::rnp_op_encrypt_add_signature(op, key.handle, ptr::null_mut()))?;
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
            check(ffi::rnp_op_encrypt_set_compression(op, cs.as_ptr(), level as i32))?;
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
