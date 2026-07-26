# Changelog

All notable changes to this project are documented in this file. The
format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

The per-phase TODO files under
[`TODO.roadmap/`](https://github.com/rnpgp/rnp-rs/tree/main/TODO.roadmap)
are the detailed audit trail for each feature area.

## [Unreleased]

### Added

- Initial Rust binding to RNP C FFI by @[object]

### Fixed

- Inline build paths instead of step-level env ([#12](https://github.com/rnpgp/rnp-rs/pull/12)) by @[object]
- Build librnp outside repo working tree ([#11](https://github.com/rnpgp/rnp-rs/pull/11)) by @[object]
- Correct librnp tarball filename ([#10](https://github.com/rnpgp/rnp-rs/pull/10)) by @[object]
- Build librnp 0.18.1 from source in release workflow ([#9](https://github.com/rnpgp/rnp-rs/pull/9)) by @[object]
- Rename package to rnp-rs (lib name unchanged) ([#7](https://github.com/rnpgp/rnp-rs/pull/7)) by @[object]
- Remove invalid allow-dirty field ([#4](https://github.com/rnpgp/rnp-rs/pull/4)) by @[object]

### Other

- Release v0.1.1 ([#5](https://github.com/rnpgp/rnp-rs/pull/5)) by @[object]
- Remove TODO.roadmap/ from main ([#3](https://github.com/rnpgp/rnp-rs/pull/3)) by @[object]
- Restructure src/ into focused submodules + add builders + remove threshold by @[object]
- Expand API: full key management, encryption, error model ([#1](https://github.com/rnpgp/rnp-rs/pull/1)) by @[object]

### Added

- Initial Rust binding to RNP C FFI by @[object]

### Fixed

- Remove invalid allow-dirty field ([#4](https://github.com/rnpgp/rnp-rs/pull/4)) by @[object]

### Other

- Remove TODO.roadmap/ from main ([#3](https://github.com/rnpgp/rnp-rs/pull/3)) by @[object]
- Restructure src/ into focused submodules + add builders + remove threshold by @[object]
- Expand API: full key management, encryption, error model ([#1](https://github.com/rnpgp/rnp-rs/pull/1)) by @[object]

### Added

- Idiomatic Rust binding to librnp's public C FFI (`include/rnp/rnp.h`).
- Three signing modes: inline (`sign`), detached (`sign_detached`), and
  cleartext (`sign_cleartext`) with matching `verify` / `verify_detached`.
- Encryption builder (`Encryptor`) with recipients, passwords, AEAD,
  compression, armor, file metadata, v6 PKESK/SKESK (feature-gated),
  PQC subkey preference (feature-gated).
- Decryption with rich result inspection (`VerifyResult` exposing
  recipients, symenc, protection info, file info).
- Full key generation via `KeyBuilder` / `SubkeyBuilder` /
  `generate_key_json` covering RSA, DSA, ElGamal, ECDSA, ECDH, EdDSA,
  SM2; preferences, expiration, usage, protection at generation time,
  v6 keys (feature-gated).
- Key inspection: 30+ scalar getters, UID enumeration, subkey
  enumeration, signature enumeration with subpacket inspection.
- Key mutation: protect / unprotect / lock / unlock / add_uid / revoke
  / set_expiration / remove / remove_signatures / export_revocation /
  export_autocrypt / 25519 bits tweak.
- Three signature-creation builders (certification, direct, revocation)
  sharing a `SignatureSetterOps` trait for the setter chain.
- Keyring management: `save_keys`, `unload_keys`, `import_keys`,
  `import_signatures`, `default_homedir`, `detect_homedir_info`,
  `detect_key_format`, `public_key_count`, `secret_key_count`,
  `IdentifierIterator`.
- ASCII armor: `enarmor`, `dearmor`, `guess_contents` with typed
  `ArmorType` and `ContentType` enums.
- Packet dumps: `dump_packets_to_output`, `dump_packets_to_json`,
  `Key::to_json`, `Key::packets_to_json`, `Signature::packet_to_json`.
- Security profile: `add_security_rule`, `get_security_rule`,
  `remove_security_rule` with typed `SecurityLevel`, `SecurityFlags`,
  `FeatureType`.
- Feature queries: `supports_feature`, `supported_features`,
  `calculate_iterations`, `request_password`.
- Version helpers: `version`, `version_string`, `version_string_full`,
  `backend_string`, `backend_version`, `commit_timestamp`.
- PQC + crypto-refresh support behind feature flags (`pqc`,
  `crypto-refresh`) with runtime probe (`librnp_supports_pqc`).
- Vendored build via `vendored` Cargo feature — builds librnp from
  `vendor/rnp/` git submodule via CMake, statically links the result.
- Logging support behind `logging` Cargo feature (`Context::set_log_fd`,
  `set_log_file`).
- `KeyProvider` trait + callback thunk for dynamic key loading during
  verify/decrypt.
- `SecretString` with zero-on-drop via `rnp_buffer_clear` for password
  hygiene.
- Categorized `Error` with `ErrorKind` (27 variants, high-nibble
  fallback for unknown codes).
- Bidirectional `From<io::Error>` impls and `from_rnp_code(u32)`
  constructor for clean integration with stdlib and custom FFI callers.
- `FromStr` / `Display` impls for 9 model enums
  (`Algorithm`, `Curve`, `Hash`, `Cipher`, `Compression`, `KeyUsage`,
  `AeadType`, `ArmorType`, `FeatureType`) plus `KeyringFormat`.
- 240 integration tests across three configurations (system librnp,
  vendored static build, PQC-enabled librnp at HEAD).
- CI workflow (`.github/workflows/ci.yml`) with three jobs covering the
  matrix.
- `examples/` directory with `sign_verify`, `encrypt_decrypt`, `keygen`
  runnable examples.
- 40 per-phase TODO files under `TODO.roadmap/` documenting status,
  architecture notes, and completion logs for each feature area.

### Architecture

- Three-layer model: bindgen-generated `ffi::` → `error::check()` for
  result-code conversion → safe wrappers in per-concern modules.
- Lifetime discipline: `Key<'ctx>` borrows `Context`; child handles
  (`Uid<'key>`, `Subkey<'key>`, `Signature<'parent>`) borrow their
  parent. No `Send` / `Sync`.
- DRY: `Input` / `Output` RAII handles own all `rnp_input_*` /
  `rnp_output_*` destroyers. `cstr_to_string` /
  `call_for_string` helpers centralize C-buffer freeing.
- OCP: typed enums with `#[non_exhaustive]` for forward-compatible
  extension; builder pattern for all op-config; flag structs as
  thin wrappers around C constants.
- MECE: 25 modules, each owning one concern.

## [0.1.0] — Unreleased

Initial public release. See Added section above.
