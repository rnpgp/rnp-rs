# rnp-rs

Idiomatic Rust binding to the [RNP](https://github.com/rnpgp/rnp) OpenPGP
(RFC 9580) C library.

RNP is the C++ OpenPGP implementation that powers Mozilla Thunderbird.
This crate provides a thin, safe Rust wrapper over its public C FFI
(declared in `include/rnp/rnp.h`).

## Status

The crate wraps ~250 of librnp's ~309 public functions across all major
OpenPGP concerns: signing, verification, encryption, decryption, key
generation, keyring management, ASCII armor, packet dumps, security
profile, and feature queries. PQC and crypto-refresh (RFC 9580 v6)
support is feature-gated. MSRV is **Rust 1.88** (let-chains in
`build.rs`).

## Cargo features

| Feature          | Default | Description                                                                                                                                                          |
|------------------|---------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `vendored`       | off     | Statically link the bundled `librnp.a` + `libsexpp.a` + `libjson-c.a` + `libbotan-3.a` + `libz.a` + `libbz2.a` from `prebuilt/<target>/`. Fully self-contained — no system crypto deps. Botan backend, full module set. |
| `vendored-minimal` | off   | Implies `vendored`. Botan backend, minimal module set (drops PQC, SM2/3/4, Twofish/Blowfish/CAST5/IDEA, Ed448/X448, Brainpool, RIPEMD-160, TLS). ~23% smaller `libbotan-3.a`. |
| `vendored-openssl3` | off | Implies `vendored`. Switches the backend to OpenSSL 3.x — bundles `libcrypto.a` + `libssl.a`. FIPS-capable. Incompatible with `pqc` (OpenSSL has no PQC). |
| `vendored-libressl` | off | Implies `vendored`. Switches the backend to LibreSSL (OpenSSL 1.1 API fork). Bundles `libcrypto.a` + `libssl.a`. Smallest crypto footprint. Incompatible with `pqc`. |
| `pqc`            | off     | Pass `-DRNP_EXPERIMENTAL_PQC` to bindgen so PQC algorithm constants and `Encryptor::prefer_pqc_enc_subkey` are exposed. Requires librnp built with `ENABLE_PQC=ON`. Forces Botan backend. |
| `crypto-refresh` | off     | Pass `-DRNP_EXPERIMENTAL_CRYPTO_REFRESH` to bindgen so v6 keys, crypto-refresh algorithm names, and v6 PKESK/SKESK are exposed.                                       |
| `logging`        | off     | Gate `Context::set_log_fd` / `set_log_file` for directing librnp's diagnostic output.                                                                                |

## Quick start

```rust
use rnp::{Algorithm, Context, Hash, KeyBuilder, KeyUsage};

let ctx = Context::new()?;
let key = KeyBuilder::new(Algorithm::Rsa)
    .bits(2048)
    .userid("alice <alice@example.com>")
    .hash(Hash::Sha256)
    .add_usage(KeyUsage::Sign)
    .add_usage(KeyUsage::Certify)
    .build(&ctx)?;

let message = b"hello, world";
let signed = rnp::sign(&ctx, message, &key)?;
let result = rnp::verify(&ctx, &signed)?;
assert!(result.any_valid()?);
# Ok::<(), rnp::Error>(())
```

See `examples/` for end-to-end demos of sign/verify, encrypt/decrypt,
key generation, and multi-signer signatures.

## Feature coverage

Each subsection below lists the Rust surface and the librnp C functions
it wraps.

### Context, password provider, and key provider

`Context::new()` is the entry point — it owns an `rnp_ffi_t` and ties
the lifetimes of every derived handle to it. A `Context` accepts a
`PasswordProvider` trait object (used by `rnp_request_password` and any
operation that needs to unlock a key) and a `KeyProvider` trait object
(used by `rnp_ffi_set_key_provider` for verify/decrypt key lookup).

```rust
use rnp::{Context, KeyIdentifier, PasswordProvider, RequestedKeyType};

struct MyPw;
impl PasswordProvider for MyPw {
    fn request(&mut self, _keyid: Option<&[u8]>, _ctx: &Context) -> rnp::Result<Vec<u8>> {
        Ok(b"hunter2".to_vec())
    }
}

let mut ctx = Context::new()?;
ctx.set_password_provider(MyPw);
```

| Rust                                          | C function                                  |
|-----------------------------------------------|---------------------------------------------|
| `Context::new` / `Drop`                       | `rnp_ffi_create` / `rnp_ffi_destroy`        |
| `Context::set_password_provider`              | `rnp_ffi_set_password_provider`             |
| `Context::set_key_provider`                   | `rnp_ffi_set_key_provider`                  |
| `Context::borrow_ffi`                         | (re-entrant thunk plumbing)                 |
| `Context::set_log_file` / `set_log_fd`        | `rnp_ffi_set_log_fd` / `set_log_file`       |
| `Context::load_keys` / `save_keys`            | `rnp_load_keys` / `rnp_save_keys`           |
| `Context::unload_keys`                        | `rnp_unload_keys`                           |
| `Context::import_keys` / `import_signatures`  | `rnp_import_keys` / `rnp_import_signatures` |
| `Context::find_key`                           | `rnp_locate_key`                            |
| `Context::default_homedir` / `detect_homedir` | `rnp_get_default_homedir` / `detect_*`      |

### Key generation

`KeyBuilder` is the primary surface; `SubkeyBuilder` builds subkeys off
a primary in one composite call. Both wrap `rnp_op_generate_*`.

```rust
use rnp::{Algorithm, Context, Hash, KeyBuilder, KeyUsage, SubkeyBuilder};

let ctx = Context::new()?;
let key = KeyBuilder::new(Algorithm::Rsa)
    .bits(3072)
    .userid("alice <alice@example.com>")
    .hash(Hash::Sha256)
    .add_usage(KeyUsage::Sign)
    .add_usage(KeyUsage::Certify)
    .add_pref_hash(Hash::Sha512)
    .clear_pref_hash()                  // reset and add a fresh set
    .add_pref_hash(Hash::Sha256)
    .add_subkey(
        SubkeyBuilder::new(Algorithm::Rsa)
            .bits(2048)
            .add_usage(KeyUsage::EncryptComms),
    )
    .build(&ctx)?;
```

| Rust                                            | C function                                |
|-------------------------------------------------|-------------------------------------------|
| `KeyBuilder::new` / `build`                     | `rnp_op_generate_create` / `execute`      |
| `KeyBuilder::bits` / `hash` / `dsa_qbits`       | `rnp_op_generate_set_{bits,hash,dsa_qbits}` |
| `KeyBuilder::curve`                             | `rnp_op_generate_set_curve`               |
| `KeyBuilder::userid`                            | `rnp_op_generate_set_userid`              |
| `KeyBuilder::expiration`                        | `rnp_op_generate_set_expiration`          |
| `KeyBuilder::add_usage` / `clear_usage`         | `rnp_op_generate_{add,clear}_usage`       |
| `KeyBuilder::add_pref_hash` / `clear_pref_hash` | `rnp_op_generate_{add,clear}_pref_hash`   |
| `KeyBuilder::add_pref_cipher` / `clear_pref_cipher`         | `rnp_op_generate_{add,clear}_pref_cipher`            |
| `KeyBuilder::add_pref_compression` / `clear_pref_compression` | `rnp_op_generate_{add,clear}_pref_compression`     |
| `KeyBuilder::pref_keyserver`                    | `rnp_op_generate_set_pref_keyserver`      |
| `KeyBuilder::protection`                        | `rnp_op_generate_set_protection_*`        |
| `KeyBuilder::request_password`                  | `rnp_op_generate_set_request_password`    |
| `KeyBuilder::v6` (crypto-refresh)               | `rnp_op_generate_set_v6_key`              |
| `SubkeyBuilder::build_with_ffi`                 | `rnp_op_generate_subkey_create`           |
| `generate_key_json`                             | `rnp_generate_key_json`                   |

Algorithm coverage: RSA, DSA, ElGamal, ECDSA/ECDH (NIST/Brainpool
curves), EdDSA, X25519, SM2, plus the PQC composites (`ML-KEM`,
`ML-DSA`, `SLH-DSA`) when built with the `pqc` feature.

### Key inspection

`Key<'ctx>` is a borrowed handle. All getters return `Result<T>` so a
deleted/expired/revoked underlying key surfaces as an error rather than
a panic.

```rust
# use rnp::*;
# let ctx = Context::new()?;
# let key = KeyBuilder::new(Algorithm::Rsa).bits(2048)
#     .userid("alice <a@example.com>").build(&ctx)?;
println!("fp: {}", key.fingerprint()?);
println!("alg: {:?}", key.alg()?);
println!("bits: {:?}", key.bits()?);
println!("subkeys: {}", key.subkey_count()?);
for uid in key.uids()? {
    println!("uid: {}", uid.data_string()?);
}
# Ok::<(), rnp::Error>(())
```

| Rust                                     | C function                                  |
|------------------------------------------|---------------------------------------------|
| `Key::alg` / `bits` / `curve` / `version`| `rnp_key_get_alg` / `get_bits` / `get_curve` / `get_version` |
| `Key::keyid` / `fingerprint` / `grip`    | `rnp_key_get_keyid` / `get_fprint` / `get_grip`             |
| `Key::primary_fprint` / `primary_grip`   | `rnp_key_get_primary_fprint` / `get_primary_grip`           |
| `Key::creation` / `expiration`           | `rnp_key_get_creation` / `get_expiration`                   |
| `Key::is_{locked,protected,primary,sub,valid,revoked,compromised,expired,superseded,retired}` | matching `rnp_key_is_*` |
| `Key::have_public` / `have_secret`       | `rnp_key_have_public` / `rnp_key_have_secret`               |
| `Key::protection_{mode,type,cipher,hash,iterations}` | `rnp_key_get_protection_*`                    |
| `Key::valid_till` / `valid_till_64`      | `rnp_key_valid_till` / `rnp_key_valid_till64`               |
| `Key::uid_count` / `uids`                | `rnp_key_get_uid_count` / `get_uid_handle_at`               |
| `Key::subkey_count` / `subkeys` / `subkey_at` | `rnp_key_get_subkey_count` / `get_subkey_at`            |
| `Key::signature_count` / `signatures`    | `rnp_key_get_signature_count` / `get_signature_at`          |
| `Key::default_key_for(usage)`            | `rnp_key_default_key_for`                                  |

### UID, Subkey, Signature handles

`Uid<'key>`, `Subkey<'key>`, and `Signature<'key>` borrow the parent
`Key`. They each carry a `Drop` that calls the matching
`rnp_*_handle_destroy`.

```rust
# use rnp::*;
# let ctx = Context::new()?;
# let key = KeyBuilder::new(Algorithm::Rsa).bits(2048)
#     .userid("alice <a@example.com>").build(&ctx)?;
let uid = key.uids()?.into_iter().next().unwrap();
println!("primary? {}", uid.is_primary()?);
println!("signatures on uid: {}", uid.signature_count()?);

let sig = key.signatures()?.into_iter().next().unwrap();
for sp in sig.subpackets()? {
    println!("subpacket: {:?}", sp.subpacket_type());
}
# Ok::<(), rnp::Error>(())
```

| Rust                                              | C function                                                |
|---------------------------------------------------|-----------------------------------------------------------|
| `Uid::uid_type` / `data` / `data_string`          | `rnp_uid_get_type` / `rnp_uid_get_data`                   |
| `Uid::is_primary` / `is_valid` / `is_revoked`     | `rnp_uid_is_primary` / `is_valid` / `is_revoked`          |
| `Uid::signature_count` / `signatures` / `signature_at` | `rnp_uid_get_signature_count` / `get_signature_at`   |
| `Uid::revocation_signature`                       | `rnp_uid_get_revocation_signature`                        |
| `Uid::remove(&key)`                               | `rnp_uid_remove`                                          |
| `Subkey` (newtype over `Key`)                     | `rnp_key_get_subkey_at`                                   |
| `Signature::sig_type` / `sig_type_enum`           | `rnp_signature_get_type`                                  |
| `Signature::alg` / `hash` / `keyid` / `creation`  | `rnp_signature_get_*`                                     |
| `Signature::signer_key`                           | `rnp_signature_get_signer`                                |
| `Signature::subpacket_count` / `subpackets` / `find_subpacket` | `rnp_signature_subpacket_{count,at,find}`    |
| `Signature::export` / `remove_from_key`           | `rnp_signature_export` / `rnp_signature_remove`           |
| `Subpacket` accessors                             | `rnp_signature_subpacket_info` / `_data`                  |

### Key mutation

```rust
# use rnp::*;
# let ctx = Context::new()?;
# let key = KeyBuilder::new(Algorithm::Rsa).bits(2048)
#     .userid("alice <a@example.com>").build(&ctx)?;
key.add_uid("alias <a@example.com>", &AddUidOptions::new().hash(Hash::Sha256))?;
key.protect(&ProtectOptions::new().password("hunter2"))?;
key.lock()?;
key.unlock(|_| Ok(b"hunter2".to_vec()))?;
# Ok::<(), rnp::Error>(())
```

| Rust                                              | C function                                |
|---------------------------------------------------|-------------------------------------------|
| `Key::protect` / `unprotect` / `lock` / `unlock`  | `rnp_key_{protect,unprotect,lock,unlock}` |
| `Key::add_uid`                                    | `rnp_key_add_uid`                         |
| `Key::revoke`                                     | `rnp_key_revoke`                          |
| `Key::set_expiration`                             | `rnp_key_set_expiration`                  |
| `Key::remove`                                     | `rnp_key_remove`                          |
| `Key::remove_signatures`                          | `rnp_key_remove_signatures`               |
| `Key::export_revocation`                          | `rnp_key_export_revocation`               |
| `Key::export_autocrypt`                           | `rnp_key_export_autocrypt`                |
| `Key::export`                                     | `rnp_key_export`                          |
| `CertificationBuilder` / `DirectSignatureBuilder` / `RevocationSignatureBuilder` | `rnp_key_{certification,direct_signature,revocation_signature}_create` |

The three signature-creation builders share a single
`SignatureSetterOps` trait — adding a new subpacket setter adds one
method to the trait and a delegating impl per concrete builder.

### Sign and verify

Three signing modes (inline / detached / cleartext) all flow through
one `Signer` builder, so new output formats are a `Mode` variant rather
than a new top-level function.

```rust
# use rnp::*;
# let ctx = Context::new()?;
# let k1 = KeyBuilder::new(Algorithm::Rsa).bits(2048).userid("a")
#     .add_usage(KeyUsage::Sign).build(&ctx)?;
# let k2 = KeyBuilder::new(Algorithm::Rsa).bits(2048).userid("b")
#     .add_usage(KeyUsage::Sign).build(&ctx)?;
let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as u32;
let sig = Signer::new(&ctx, b"doc", Mode::Detached)
    .add_signer(&k1)
    .add_signer_with_options(&k2, Hash::Sha384, Some(now), Some(604800))
    .armor(true)
    .build_to_memory()?;
# Ok::<(), rnp::Error>(())
```

| Rust                                          | C function                                |
|-----------------------------------------------|-------------------------------------------|
| `sign` / `sign_detached` / `sign_cleartext`   | `rnp_op_sign_*_create` + `execute`        |
| `Signer::add_signer` / `_with_hash` / `_with_options` | `rnp_op_sign_add_signature`        |
| `Signer::hash` (default) / per-signer         | `rnp_op_sign_set_hash` / `_signature_set_hash` |
| `Signer::armor`                               | `rnp_op_sign_set_armor`                   |
| `add_signer_with_options` (creation/expiration) | `rnp_op_sign_signature_set_creation_time` / `_set_expiration_time` |
| `verify` / `verify_detached`                  | `rnp_op_verify_*_create` + `execute`      |
| `VerifyResult::signature_count` / `signature_at` | `rnp_op_verify_get_signature_count` / `_at` |
| `VerifySignature::status` / `key` / `keyid`   | `rnp_signature_get_status` etc.           |
| `VerifyOp::flags`                             | `rnp_op_verify_set_flags`                 |
| `generate_revocation_certificate[_with]`      | `rnp_key_export_revocation`               |

### Encrypt and decrypt

```rust
# use rnp::*;
# let ctx = Context::new()?;
# let key = KeyBuilder::new(Algorithm::Rsa).bits(2048)
#     .userid("enc <enc@example.com>")
#     .add_usage(KeyUsage::EncryptComms).build(&ctx)?;
let mut ct = Output::to_memory()?;
Encryptor::new(&ctx, b"secret")?
    .add_recipient(&key)
    .armor(true)
    .aead(AeadType::Ocb)
    .build(&mut ct)?;
let ciphertext = ct.into_bytes()?;

let result = Decryptor::new(&ctx, &ciphertext).build()?;
assert_eq!(result.plaintext(), b"secret");
# Ok::<(), rnp::Error>(())
```

| Rust                                              | C function                                |
|---------------------------------------------------|-------------------------------------------|
| `Encryptor::new` / `build`                        | `rnp_op_encrypt_create` + `execute`       |
| `Encryptor::add_recipient` / `add_password` / `add_signature` | `rnp_op_encrypt_add_recipient` / `_add_password` / `_add_signature` |
| `Encryptor::{cipher,hash,compression,aead,aead_bits,armor,file_name,file_mtime,creation_time,expiration_time,flags}` | matching `rnp_op_encrypt_set_*` |
| `Encryptor::prefer_pqc_enc_subkey` (pqc)          | `rnp_op_encrypt_prefer_pqc_enc_subkey`    |
| `Encryptor::enable_pkesk_v6` / `enable_skesk_v6`  | `rnp_op_encrypt_set_*` v6 PKESK/SKESK     |
| `Decryptor::new` / `build`                        | `rnp_decrypt` (and verify-op plumbing)    |
| `DecryptResult::{recipient,symenc,protection_info,file_info,format}` | `rnp_decrypt_get_*`            |
| `Recipient` / `Symenc` handles                    | `rnp_recipient_*` / `rnp_symenc_*`        |

### ASCII armor, packet dumps, JSON serialization

```rust
# use rnp::*;
let armored = enarmor(b"\x88\x01...", ContentType::Binary)?;
let raw = dearmor(&armored)?;
let json = dump_packets_to_json(&ctx, &raw, JsonDumpFlags::empty())?;
let key_json = key.to_json(JsonFlags::empty())?;
# Ok::<(), rnp::Error>(())
```

| Rust                                              | C function                                  |
|---------------------------------------------------|---------------------------------------------|
| `enarmor` / `dearmor` / `dearmor_bytes` / `armor_bytes` | `rnp_armor_*` / `rnp_dearmor_*`       |
| `guess_contents`                                  | `rnp_guess_contents`                        |
| `dump_packets_to_output` / `_to_json` / `_bytes_to_json` | `rnp_dump_packets_to_*`              |
| `Key::to_json` / `Signature::to_json`             | `rnp_key_to_json` / `rnp_signature_packet_to_json` |
| `DumpFlags` / `JsonDumpFlags` / `JsonFlags`       | `RNP_DUMP_*` / `RNP_JSON_*` bit constants   |

### Security profile and feature queries

```rust
# use rnp::*;
let ctx = Context::new()?;
let rule = SecurityRule::new(
    SecurityLevel::Default,
    SecurityFlags::empty(),
    FeatureType::PublicKey,
    "RSA",
)?;
ctx.add_security_rule(&rule)?;
let iters = calculate_iterations(Hash::Sha256, 100_000_000)?;
let features = supported_features()?;
assert!(supports_feature(FeatureType::HashAlgorithm, "SHA256")?);
# Ok::<(), rnp::Error>(())
```

| Rust                                              | C function                                |
|---------------------------------------------------|-------------------------------------------|
| `Context::add_security_rule` / `get_security_rule` / `remove_security_rule` | `rnp_add_security_rule` / `rnp_get_security_rule` / `rnp_remove_security_rule` |
| `SecurityLevel` / `SecurityFlags`                 | `RNP_SECURITY_*` constants                |
| `supports_feature` / `supported_features`         | `rnp_supports_feature` / `rnp_supported_features` |
| `calculate_iterations`                            | `rnp_calculate_iterations`                |
| `request_password`                                | `rnp_request_password` (returns `SecretString`) |
| `Context::set_timestamp`                          | `rnp_set_timestamp`                       |

### Secret-string hygiene

`SecretString` is a `Vec<u8>` wrapper that zeros its bytes on `Drop`
via `rnp_buffer_clear`. `request_password` returns one; user-supplied
passwords that you want to scrub should also live in a `SecretString`.

### PQC and crypto-refresh

Gated on the `pqc` and `crypto-refresh` Cargo features. The crate
probes the linked librnp at runtime (`librnp_supports_pqc`) so a binary
that enables PQC still degrades cleanly on a non-PQC librnp.

```rust
#[cfg(feature = "pqc")]
{
    if rnp::librnp_supports_pqc()? {
        let key = KeyBuilder::new(Algorithm::Rsa)
            .bits(2048)
            .userid("alice")
            .add_pqc_encryption_subkey(PqcAlgorithm::MlKem768X25519)
            .add_pqc_signing_subkey(PqcAlgorithm::MlDsa65Ed25519)
            .build(&ctx)?;
    }
}
```

### Version and debug

```rust
println!("librnp {}", rnp::version_string());
let (maj, min, patch) = rnp::version::decompose();
println!("backend: {}", rnp::version::backend_string());

// Emit librnp diagnostic output to stderr (or a named file).
rnp::version::enable_debug("stderr");
rnp::version::disable_debug();
```

| Rust                                       | C function                            |
|--------------------------------------------|---------------------------------------|
| `version` / `decompose`                    | `rnp_version` / `_major` / `_minor` / `_patch` |
| `version_string` / `version_string_full`   | `rnp_version_string` / `_full`        |
| `version_for` / `version_commit_timestamp` | `rnp_version_for` / `_commit_timestamp` |
| `backend_string` / `backend_version`       | `rnp_backend_string` / `_version`     |
| `enable_debug` / `disable_debug`           | `rnp_enable_debug` / `rnp_disable_debug` |

## Linking

`rnp-rs` links against a **system-installed** `librnp` (`-lrnp`) by
default. Use `--features vendored` to skip that requirement entirely.

### macOS (Homebrew)

```sh
brew install rnp
```

### Linux

```sh
# Fedora
sudo dnf install librnp-devel

# Debian/Ubuntu (if packaged)
sudo apt install librnp-dev
```

### Pointing at a source checkout

```sh
RNP_INCLUDE_DIR=/path/to/rnp/install/include \
RNP_LIB_DIR=/path/to/rnp/install/lib \
cargo build
```

### Vendored (no system librnp)

Four backend variants:

```sh
cargo build --features vendored              # Botan, full module set
cargo build --features vendored-minimal      # Botan, minimal modules
cargo build --features vendored-openssl3     # OpenSSL 3.x
cargo build --features vendored-libressl     # LibreSSL (OpenSSL 1.1 API)
```

`build.rs` selects the right `prebuilt/<target>-<suffix>/` subdir based on
the feature combination. Each subdir contains `librnp.a` + `libsexpp.a` +
`libjson-c.a` + `libz.a` + `libbz2.a` plus the backend's crypto lib
(`libbotan-3.a` for Botan variants, `libcrypto.a` + `libssl.a` for
OpenSSL/LibreSSL). Fully self-contained — no system crypto deps.

**Backend picker**

- **`vendored`** (Botan, full): the only option that supports PQC
  (`ML-KEM`, `ML-DSA`, `SLH-DSA`). Largest footprint. Use when you need
  RFC 9580 crypto-refresh + PQC, or just want maximum algorithm coverage.
- **`vendored-minimal`** (Botan, minimal): same Botan codebase but ~23%
  smaller `libbotan-3.a` — drops TLS, PQC, SM2/3/4, legacy symmetric
  ciphers. Good default for non-PQC use.
- **`vendored-openssl3`**: OpenSSL 3.x backend. FIPS-capable. Smaller
  than Botan. Choose when targeting FIPS environments or where OpenSSL
  is the approved crypto provider.
- **`vendored-libressl`**: LibreSSL backend (OpenSSL 1.1 API). Smallest
  crypto footprint. Choose for size-constrained deployments that don't
  need PQC or FIPS.

`pqc` is incompatible with everything except plain `vendored` — the
build emits a clear `compile_error!` if you combine it with any other
backend.

**Linux libc**: prebuilts ship for both `gnu` (glibc) and `musl`
targets. `cargo build --target x86_64-unknown-linux-musl --features
vendored` works out of the box — the build script keys the prebuilt
selection off the full target triple.

Pre-builts are produced by `.github/workflows/prebuild.yml` for
x86_64/aarch64 Linux (gnu + musl) and macOS, across all four backends.

## Architecture

Three layers, top to bottom:

1. **`src/ffi.rs`** — bindgen-generated `include!` of `$OUT_DIR/bindings.rs`. The module is `#![allow(non_camel_case_types, non_snake_case, dead_code)]` because we mirror the C API verbatim.
2. **`src/error.rs`** — `Error` (snafu), `Result<T>`, and `check(rnp_result_t)` that maps non-zero to `Error::Rnp { code, message }` via `rnp_result_to_string`.
3. **Safe wrappers** — one directory module per OpenPGP concern:
   - `context.rs`, `key/`, `uid.rs`, `subkey.rs`, `keygen/`, `signature.rs`, `encrypt/`, `verify/`, `armor.rs`, `dump.rs`, `security.rs`, `signature_handle/`, `key_signature_builder/`, `callbacks/`, `secret.rs`, `ops/` (IO + string-conversion helpers).

Every handle type borrows its parent (via `PhantomData`) and owns a
`Drop` that calls the matching `rnp_*_handle_destroy`. Never add
`unsafe impl Send` — librnp handles are not thread-safe.

## License

BSD-2-Clause, matching the upstream RNP project. See `LICENSE.md`.
