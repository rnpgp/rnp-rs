//! Idiomatic Rust binding to the RNP OpenPGP C library (`librnp`).
//!
//! RNP is the OpenPGP implementation that powers Mozilla Thunderbird. This
//! crate provides a thin, idiomatic Rust wrapper over the public C FFI
//! declared in `<rnp/rnp.h>`.

pub mod armor;
pub mod callbacks;
pub mod context;
pub mod dump;
pub mod encrypt;
pub mod error;
pub mod ffi;
pub mod key;
pub mod keygen;
pub mod keyring;
pub mod key_signature_builder;
pub mod ops;
pub mod secret;
pub mod security;
pub mod signature;
pub mod signature_handle;
pub mod subkey;
pub mod uid;
pub mod verify;
pub mod version;

pub use armor::{armor_bytes, dearmor, dearmor_bytes, enarmor, guess_contents, ContentType};
pub use callbacks::{KeyProvider, KeyRequestOutcome, RequestedKeyType};
pub use context::{Context, KeyringFormat, PasswordProvider};
pub use dump::{
    dump_packets_bytes_to_json, dump_packets_to_json, dump_packets_to_output, DumpFlags,
    JsonDumpFlags, JsonFlags,
};
pub use encrypt::{decrypt, decrypt_to, AddPasswordOptions, AeadType, EncryptFlags, Encryptor};
pub use error::{Error, ErrorKind, Result};
pub use key::{
    AddUidOptions, ExportFlags, Key, KeyIdentifier, LoadSaveFlags, ProtectOptions, RemoveFlags,
    RemoveSignaturesFlags, RevocationCode, RevocationReason, UnloadFlags,
};
pub use keygen::{
    generate_key_json, Algorithm, Cipher, Compression, Curve, Hash, KeyBuilder, KeyUsage,
    SubkeyBuilder,
};
#[cfg(feature = "pqc")]
pub use keygen::{librnp_supports_pqc, PqcAlgorithm};
#[allow(deprecated)]
pub use keygen::generate_test_key;
pub use keyring::{IdentifierIterator, IdentifierKind};
pub use key_signature_builder::{
    CertificationBuilder, ConfiguredBuilder, DirectSignatureBuilder, RevocationSignatureBuilder,
    SignatureSetterOps,
};
pub use ops::{
    call_for_optional_string, call_for_string, cstr_to_optional_string, cstr_to_string,
    ArmorType, Input, Output, OutputFileFlags,
};
pub use secret::{zero_string_bytes, SecretString};
pub use security::{
    calculate_iterations, request_password, supports_feature, supported_features, FeatureType,
    SecurityFlags, SecurityLevel, SecurityRule,
};
pub use signature::{sign, sign_detached, verify, verify_detached};
pub use signature_handle::{Signature, Subpacket, SubpacketType};
pub use subkey::Subkey;
pub use uid::{Uid, UidType};
pub use verify::{
    FileInfo, Recipient, SignatureStatus, Symenc, VerifyFlags, VerifyOp, VerifyResult,
    VerifySignature,
};

/// librnp version string, e.g. `"0.18.1"`.
pub fn version_string() -> String {
    crate::version::version_string()
}
