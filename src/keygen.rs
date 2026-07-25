//! Key generation.
//!
//! Two builders over `rnp_op_generate_*`:
//!
//! - [`KeyBuilder`] for primary keys.
//! - [`SubkeyBuilder`] for subkeys (requires a primary key).
//!
//! Plus the [`Algorithm`] / [`Curve`] / [`Hash`] / [`Cipher`] / [`Compression`]
//! enums that replace the C-side string constants, and the JSON shortcut
//! [`generate_key_json`].
//!
//! ## Deprecated
//!
//! [`generate_test_key`] remains as a thin shim over [`KeyBuilder`] for
//! existing call sites. Prefer `KeyBuilder` directly.

use crate::context::Context;
use crate::error::{self, check, Result};
use crate::ffi;
use crate::key::Key;
use std::ffi::CString;
use std::ptr;

// ---------------------------------------------------------------------------
// Algorithm / parameter enums
// ---------------------------------------------------------------------------

/// Public-key algorithm. Maps to the `RNP_ALGNAME_*` string constants in
/// `rnp.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Algorithm {
    /// `"RSA"`. Used for primary or subkey; supports sign + encrypt (legacy).
    Rsa,
    /// `"ELGAMAL"`. Subkey only, encryption.
    ElGamal,
    /// `"DSA"`. Primary only, signing.
    Dsa,
    /// `"ECDH"`. Subkey only, encryption.
    Ecdh,
    /// `"ECDSA"`. Primary or subkey, signing.
    Ecdsa,
    /// `"EDDSA"`. Primary or subkey, signing (Ed25519 curve).
    Eddsa,
    /// `"SM2"`. Primary or subkey, sign + encrypt.
    Sm2,
}

impl Algorithm {
    pub fn as_str(self) -> &'static str {
        match self {
            Algorithm::Rsa => "RSA",
            Algorithm::ElGamal => "ELGAMAL",
            Algorithm::Dsa => "DSA",
            Algorithm::Ecdh => "ECDH",
            Algorithm::Ecdsa => "ECDSA",
            Algorithm::Eddsa => "EDDSA",
            Algorithm::Sm2 => "SM2",
        }
    }
}

/// PQC composite algorithms. Available only when the crate is built with
/// `--features pqc` and the linked librnp was built with `ENABLE_PQC=ON`.
///
/// These wrap the `RNP_ALGNAME_KYBER*` / `RNP_ALGNAME_DILITHIUM*` /
/// `RNP_ALGNAME_SPHINCSPLUS_*` string constants in `rnp.h`.
#[cfg(feature = "pqc")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PqcAlgorithm {
    // Key-encapsulation (encryption) composites.
    MlKem768X25519,
    MlKem1024X448,
    MlKem768P384,
    MlKem1024P521,
    MlKem768Bp384,
    MlKem1024Bp512,
    // Signature composites.
    MlDsa65Ed25519,
    MlDsa87Ed448,
    MlDsa65P384,
    MlDsa87P521,
    MlDsa65Bp384,
    MlDsa87Bp512,
    // Standalone SLH-DSA variants.
    SlhDsaShake128f,
    SlhDsaShake128s,
    SlhDsaShake256s,
}

#[cfg(feature = "pqc")]
impl PqcAlgorithm {
    pub fn as_str(self) -> &'static str {
        match self {
            PqcAlgorithm::MlKem768X25519 => "ML-KEM-768+X25519",
            PqcAlgorithm::MlKem1024X448 => "ML-KEM-1024+X448",
            PqcAlgorithm::MlKem768P384 => "ML-KEM-768+ECDH-P384",
            PqcAlgorithm::MlKem1024P521 => "ML-KEM-1024+ECDH-P521",
            PqcAlgorithm::MlKem768Bp384 => "ML-KEM-768+ECDH-BP384",
            PqcAlgorithm::MlKem1024Bp512 => "ML-KEM-1024+ECDH-BP512",
            PqcAlgorithm::MlDsa65Ed25519 => "ML-DSA-65+ED25519",
            PqcAlgorithm::MlDsa87Ed448 => "ML-DSA-87+ED448",
            PqcAlgorithm::MlDsa65P384 => "ML-DSA-65+ECDSA-P384",
            PqcAlgorithm::MlDsa87P521 => "ML-DSA-87+ECDSA-P521",
            PqcAlgorithm::MlDsa65Bp384 => "ML-DSA-65+ECDSA-BP384",
            PqcAlgorithm::MlDsa87Bp512 => "ML-DSA-87+ECDSA-BP512",
            PqcAlgorithm::SlhDsaShake128f => "SLH-DSA-SHAKE-128f",
            PqcAlgorithm::SlhDsaShake128s => "SLH-DSA-SHAKE-128s",
            PqcAlgorithm::SlhDsaShake256s => "SLH-DSA-SHAKE-256s",
        }
    }
}

/// Runtime probe: confirm that the linked librnp actually supports PQC.
/// Build-time `--features pqc` only makes the symbols visible to bindgen —
/// the binary must also have been compiled with `ENABLE_PQC=ON`. Call this
/// before any PQC operation.
#[cfg(feature = "pqc")]
pub fn librnp_supports_pqc() -> bool {
    crate::security::supports_feature(
        crate::security::FeatureType::PublicKeyAlgorithm,
        "ML-KEM-768+X25519",
    )
    .unwrap_or(false)
}

/// Elliptic curve name. Pass to [`KeyBuilder::curve`] / [`SubkeyBuilder::curve`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve {
    /// `"NIST P-256"` (secp256r1).
    P256,
    /// `"NIST P-384"` (secp384r1).
    P384,
    /// `"NIST P-521"` (secp521r1).
    P521,
    /// `"Ed25519"` — used with `Algorithm::Eddsa`.
    Ed25519,
    /// `"Curve25519"` — used with `Algorithm::Ecdh`.
    Curve25519,
    /// `"brainpoolP256r1"`.
    Bp256,
    /// `"brainpoolP384r1"`.
    Bp384,
    /// `"brainpoolP512r1"`.
    Bp512,
    /// `"secp256k1"` — Bitcoin curve.
    Secp256k1,
    /// `"SM2 P-256"`.
    Sm2P256,
}

impl Curve {
    pub fn as_str(self) -> &'static str {
        match self {
            Curve::P256 => "NIST P-256",
            Curve::P384 => "NIST P-384",
            Curve::P521 => "NIST P-521",
            Curve::Ed25519 => "Ed25519",
            Curve::Curve25519 => "Curve25519",
            Curve::Bp256 => "brainpoolP256r1",
            Curve::Bp384 => "brainpoolP384r1",
            Curve::Bp512 => "brainpoolP512r1",
            Curve::Secp256k1 => "secp256k1",
            Curve::Sm2P256 => "SM2 P-256",
        }
    }
}

/// Hash algorithm. Pass to [`KeyBuilder::hash`] / [`SubkeyBuilder::hash`] and
/// signature operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hash {
    Sha1,
    Sha224,
    Sha256,
    Sha384,
    Sha512,
    Sha3_256,
    Sha3_512,
    Md5,
    Ripemd160,
    Sm3,
}

impl Hash {
    pub fn as_str(self) -> &'static str {
        match self {
            Hash::Sha1 => "SHA1",
            Hash::Sha224 => "SHA224",
            Hash::Sha256 => "SHA256",
            Hash::Sha384 => "SHA384",
            Hash::Sha512 => "SHA512",
            Hash::Sha3_256 => "SHA3-256",
            Hash::Sha3_512 => "SHA3-512",
            Hash::Md5 => "MD5",
            Hash::Ripemd160 => "RIPEMD160",
            Hash::Sm3 => "SM3",
        }
    }
}

/// Symmetric cipher. Used in protection, encryption, AEAD.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cipher {
    Idea,
    Tripledes,
    Cast5,
    Blowfish,
    Aes128,
    Aes192,
    Aes256,
    Twofish,
    Camellia128,
    Camellia192,
    Camellia256,
    Sm4,
}

impl Cipher {
    pub fn as_str(self) -> &'static str {
        match self {
            Cipher::Idea => "IDEA",
            Cipher::Tripledes => "TRIPLEDES",
            Cipher::Cast5 => "CAST5",
            Cipher::Blowfish => "BLOWFISH",
            Cipher::Aes128 => "AES128",
            Cipher::Aes192 => "AES192",
            Cipher::Aes256 => "AES256",
            Cipher::Twofish => "TWOFISH",
            Cipher::Camellia128 => "CAMELLIA128",
            Cipher::Camellia192 => "CAMELLIA192",
            Cipher::Camellia256 => "CAMELLIA256",
            Cipher::Sm4 => "SM4",
        }
    }
}

/// Compression algorithm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    Zip,
    Zlib,
    Bzip2,
}

impl Compression {
    pub fn as_str(self) -> &'static str {
        match self {
            Compression::Zip => "ZIP",
            Compression::Zlib => "ZLIB",
            Compression::Bzip2 => "BZIP2",
        }
    }
}

/// Key usage flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyUsage {
    Certify,
    Sign,
    EncryptComms,
    EncryptStorage,
}

impl KeyUsage {
    pub fn as_str(self) -> &'static str {
        match self {
            KeyUsage::Certify => "certify",
            KeyUsage::Sign => "sign",
            KeyUsage::EncryptComms => "encrypt",
            KeyUsage::EncryptStorage => "encrypt",
        }
    }
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Builder over `rnp_op_generate_*` for primary keys.
///
/// Terminal method [`KeyBuilder::build`] consumes `self`, executes the
/// generation, and returns the resulting [`Key`].
///
/// ```
/// # use rnp::{Algorithm, Context, Hash, KeyBuilder, KeyUsage};
/// # let ctx = Context::new().unwrap();
/// let key = KeyBuilder::new(Algorithm::Rsa)
///     .bits(2048)
///     .userid("alice <alice@example.com>")
///     .hash(Hash::Sha256)
///     .add_usage(KeyUsage::Sign)
///     .add_usage(KeyUsage::Certify)
///     .build(&ctx)
///     .unwrap();
/// ```
pub struct KeyBuilder {
    alg: Algorithm,
    bits: Option<u32>,
    hash: Option<Hash>,
    dsa_qbits: Option<u32>,
    curve: Option<Curve>,
    userid: Option<String>,
    expiration: Option<u32>,
    usages: Vec<KeyUsage>,
    pref_hashes: Vec<Hash>,
    pref_ciphers: Vec<Cipher>,
    pref_compressions: Vec<Compression>,
    pref_keyserver: Option<String>,
    // TODO(phase-09 or later): gate v6 behind a `crypto-refresh` Cargo feature
    // that passes -DRNP_EXPERIMENTAL_CRYPTO_REFRESH to bindgen in build.rs,
    // matching how PQC will be gated. For now v6 keys are not exposed.
}

impl KeyBuilder {
    pub fn new(alg: Algorithm) -> Self {
        KeyBuilder {
            alg,
            bits: None,
            hash: None,
            dsa_qbits: None,
            curve: None,
            userid: None,
            expiration: None,
            usages: Vec::new(),
            pref_hashes: Vec::new(),
            pref_ciphers: Vec::new(),
            pref_compressions: Vec::new(),
            pref_keyserver: None,
        }
    }

    pub fn bits(mut self, n: u32) -> Self {
        self.bits = Some(n);
        self
    }

    pub fn hash(mut self, h: Hash) -> Self {
        self.hash = Some(h);
        self
    }

    pub fn dsa_qbits(mut self, n: u32) -> Self {
        self.dsa_qbits = Some(n);
        self
    }

    pub fn curve(mut self, c: Curve) -> Self {
        self.curve = Some(c);
        self
    }

    pub fn userid(mut self, uid: impl Into<String>) -> Self {
        self.userid = Some(uid.into());
        self
    }

    pub fn expiration(mut self, seconds: u32) -> Self {
        self.expiration = Some(seconds);
        self
    }

    pub fn add_usage(mut self, u: KeyUsage) -> Self {
        self.usages.push(u);
        self
    }

    pub fn add_pref_hash(mut self, h: Hash) -> Self {
        self.pref_hashes.push(h);
        self
    }

    pub fn add_pref_cipher(mut self, c: Cipher) -> Self {
        self.pref_ciphers.push(c);
        self
    }

    pub fn add_pref_compression(mut self, c: Compression) -> Self {
        self.pref_compressions.push(c);
        self
    }

    pub fn pref_keyserver(mut self, s: impl Into<String>) -> Self {
        self.pref_keyserver = Some(s.into());
        self
    }

    /// Execute the generation. The returned [`Key`] borrows `ctx` for its
    /// lifetime (matching the rest of the crate's lifetime discipline).
    pub fn build<'ctx>(self, ctx: &'ctx Context) -> Result<Key<'ctx>> {
        let alg_c = CString::new(self.alg.as_str()).unwrap();
        let mut op: ffi::rnp_op_generate_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_generate_create(&mut op, ctx.ffi, alg_c.as_ptr()))?;
        }

        // Apply all setters. If any fails we destroy the op and bail.
        let setters = unsafe { apply_setters(op, &self) };
        if let Err(e) = setters {
            unsafe {
                let _ = ffi::rnp_op_generate_destroy(op);
            }
            return Err(e);
        }

        unsafe {
            check(ffi::rnp_op_generate_execute(op))?;
            let mut handle: ffi::rnp_key_handle_t = ptr::null_mut();
            check(ffi::rnp_op_generate_get_key(op, &mut handle))?;
            let _ = ffi::rnp_op_generate_destroy(op);
            if handle.is_null() {
                return Err(error::Error::NullPointer);
            }
            Ok(Key::from_handle(handle))
        }
    }
}

/// Apply all setters from `b` to the op handle.
unsafe fn apply_setters(op: ffi::rnp_op_generate_t, b: &KeyBuilder) -> Result<()> {
    unsafe {
        if let Some(n) = b.bits {
            check(ffi::rnp_op_generate_set_bits(op, n))?;
        }
        if let Some(h) = b.hash {
            let c = CString::new(h.as_str()).unwrap();
            check(ffi::rnp_op_generate_set_hash(op, c.as_ptr()))?;
        }
        if let Some(q) = b.dsa_qbits {
            check(ffi::rnp_op_generate_set_dsa_qbits(op, q))?;
        }
        if let Some(c) = b.curve {
            let cs = CString::new(c.as_str()).unwrap();
            check(ffi::rnp_op_generate_set_curve(op, cs.as_ptr()))?;
        }
        if let Some(uid) = &b.userid {
            let c = CString::new(uid.as_str()).map_err(|_| error::Error::NulByte)?;
            check(ffi::rnp_op_generate_set_userid(op, c.as_ptr()))?;
        }
        if let Some(exp) = b.expiration {
            check(ffi::rnp_op_generate_set_expiration(op, exp))?;
        }
        for u in &b.usages {
            let c = CString::new(u.as_str()).unwrap();
            check(ffi::rnp_op_generate_add_usage(op, c.as_ptr()))?;
        }
        for h in &b.pref_hashes {
            let c = CString::new(h.as_str()).unwrap();
            check(ffi::rnp_op_generate_add_pref_hash(op, c.as_ptr()))?;
        }
        for c2 in &b.pref_ciphers {
            let c = CString::new(c2.as_str()).unwrap();
            check(ffi::rnp_op_generate_add_pref_cipher(op, c.as_ptr()))?;
        }
        for cz in &b.pref_compressions {
            let c = CString::new(cz.as_str()).unwrap();
            check(ffi::rnp_op_generate_add_pref_compression(op, c.as_ptr()))?;
        }
        if let Some(ks) = &b.pref_keyserver {
            let c = CString::new(ks.as_str()).map_err(|_| error::Error::NulByte)?;
            check(ffi::rnp_op_generate_set_pref_keyserver(op, c.as_ptr()))?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Subkey builder
// ---------------------------------------------------------------------------

/// Builder for generating a subkey off an existing primary key.
///
/// Wraps `rnp_op_generate_subkey_create`. Same setter surface as
/// [`KeyBuilder`] minus `userid` and `v6` (those are primary-only).
pub struct SubkeyBuilder {
    alg: Algorithm,
    bits: Option<u32>,
    hash: Option<Hash>,
    dsa_qbits: Option<u32>,
    curve: Option<Curve>,
    expiration: Option<u32>,
    usages: Vec<KeyUsage>,
}

impl SubkeyBuilder {
    pub fn new(alg: Algorithm) -> Self {
        SubkeyBuilder {
            alg,
            bits: None,
            hash: None,
            dsa_qbits: None,
            curve: None,
            expiration: None,
            usages: Vec::new(),
        }
    }

    pub fn bits(mut self, n: u32) -> Self {
        self.bits = Some(n);
        self
    }

    pub fn hash(mut self, h: Hash) -> Self {
        self.hash = Some(h);
        self
    }

    pub fn dsa_qbits(mut self, n: u32) -> Self {
        self.dsa_qbits = Some(n);
        self
    }

    pub fn curve(mut self, c: Curve) -> Self {
        self.curve = Some(c);
        self
    }

    pub fn expiration(mut self, seconds: u32) -> Self {
        self.expiration = Some(seconds);
        self
    }

    pub fn add_usage(mut self, u: KeyUsage) -> Self {
        self.usages.push(u);
        self
    }

    /// Execute the subkey generation. The returned [`Key`] borrows `ctx`.
    pub fn build<'ctx>(self, ctx: &'ctx Context, primary: &Key<'_>) -> Result<Key<'ctx>> {
        let alg_c = CString::new(self.alg.as_str()).unwrap();
        let mut op: ffi::rnp_op_generate_t = ptr::null_mut();
        unsafe {
            check(ffi::rnp_op_generate_subkey_create(
                &mut op,
                ctx.ffi,
                primary.handle,
                alg_c.as_ptr(),
            ))?;
        }

        if let Err(e) = unsafe { apply_subkey_setters(op, &self) } {
            unsafe {
                let _ = ffi::rnp_op_generate_destroy(op);
            }
            return Err(e);
        }

        unsafe {
            check(ffi::rnp_op_generate_execute(op))?;
            let mut handle: ffi::rnp_key_handle_t = ptr::null_mut();
            check(ffi::rnp_op_generate_get_key(op, &mut handle))?;
            let _ = ffi::rnp_op_generate_destroy(op);
            if handle.is_null() {
                return Err(error::Error::NullPointer);
            }
            Ok(Key::from_handle(handle))
        }
    }
}

unsafe fn apply_subkey_setters(op: ffi::rnp_op_generate_t, b: &SubkeyBuilder) -> Result<()> {
    unsafe {
        if let Some(n) = b.bits {
            check(ffi::rnp_op_generate_set_bits(op, n))?;
        }
        if let Some(h) = b.hash {
            let c = CString::new(h.as_str()).unwrap();
            check(ffi::rnp_op_generate_set_hash(op, c.as_ptr()))?;
        }
        if let Some(q) = b.dsa_qbits {
            check(ffi::rnp_op_generate_set_dsa_qbits(op, q))?;
        }
        if let Some(c) = b.curve {
            let cs = CString::new(c.as_str()).unwrap();
            check(ffi::rnp_op_generate_set_curve(op, cs.as_ptr()))?;
        }
        if let Some(exp) = b.expiration {
            check(ffi::rnp_op_generate_set_expiration(op, exp))?;
        }
        for u in &b.usages {
            let c = CString::new(u.as_str()).unwrap();
            check(ffi::rnp_op_generate_add_usage(op, c.as_ptr()))?;
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
    let mut raw: *mut std::os::raw::c_char = ptr::null_mut();
    unsafe {
        check(ffi::rnp_generate_key_json(ctx.ffi, c.as_ptr(), &mut raw))?;
        if raw.is_null() {
            return Err(error::Error::NullPointer);
        }
        // SAFETY: raw was populated by librnp and is owned by us.
        crate::ops::cstr_to_string(raw)
    }
}

// ---------------------------------------------------------------------------
// Deprecated test helper (kept as a shim over KeyBuilder).
// ---------------------------------------------------------------------------

/// Generate an unprotected RSA-2048 keypair for tests. New code should use
/// [`KeyBuilder`] directly.
#[deprecated(note = "use KeyBuilder instead")]
pub fn generate_test_key<'a>(ctx: &'a Context, userid: &str) -> Result<Key<'a>> {
    KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid(userid)
        .hash(Hash::Sha256)
        .build(ctx)
}
