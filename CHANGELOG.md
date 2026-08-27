# Changelog

All notable changes to this project are documented in this file. The
format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

The per-phase TODO files under
[`TODO.roadmap/`](https://github.com/rnpgp/rnp-rs/tree/main/TODO.roadmap)
are the detailed audit trail for each feature area.

## [Unreleased]

### Added

- Streaming I/O: `Input::from_reader` / `Output::to_writer` bridge any
  `std::io::Read` / `Write` into librnp operations (network streams, pipes,
  large files) without buffering, with panic-safe thunks, io-error
  surfacing, and writer discard semantics
- `*_with_input` constructors (`Signer`, `Encryptor`, `VerifyOp`),
  `decrypt_from_input`, and `load_keys_from_input` /
  `import_keys_from_input` / `import_signatures_from_input` for streaming
  key material
- `Output::into_writer` / `Input::into_reader` reclaim the stream object;
  `WriterOutcome` reports flush/discard outcomes

### Fixed

- Leak: `VerifyOp::detached` forgot the signature input, but upstream never
  destroys op inputs — the op now owns all streams and destroys them in the
  canonical order (op first, streams after)
- `Output::into_bytes` returned `BAD_PARAMETERS` for a memory output with
  zero bytes written; now yields an empty buffer
- Doctest executables could not find the Homebrew dylib at runtime
  (`.cargo/config.toml` now sets `DYLD_LIBRARY_PATH` for rustdoc)
- Drop gitignored Cargo.lock from include; ship tests/examples/benches by @[object]

### Added

- Botan 3.13.0 + mixed-graph duplicate-Botan detection (post-split port) by @[object]
- Pregenerated bindings — cross builds without host libclang ([#69](https://github.com/rnpgp/rnp-rs/pull/69)) by @[object]

### Fixed

- Hold Botan at 3.12.0 — 3.13.0 transient PKESK decrypt failure (rnp-rs#79, rnpgp/rnp#2465) by @[object]
- Cross-build hardening — toolchain passthrough, no CLI build, no json-c apps by @[object]
- Add flate/ureq to typos allowlist (crate names, not typos) by @[object]

### Other

- Split workspace into rnp-src / rnp-sys / rnp (botan-rs pattern) by @[object]
- Cargo fmt by @[object]
- Collapse nested if into let-chain (clippy collapsible_if) by @[object]
- Cargo fmt by @[object]
- Fix vendored setup guide and add step-by-step tutorial by @[object]

### Fixed

- Keep linking stdc++ alongside Windows system libs by @[object]
- Link advapi32/ws2_32/crypt32 in vendored mode by @[object]

### Other

- Release v0.1.8 by @[object]
- Release v0.1.8 by @[object]
- Update prebuilt static libraries for all platforms ([#46](https://github.com/rnpgp/rnp-rs/pull/46)) by @[object]
- Fetch chore/prebuilt-update before force-with-lease by @[object]
- Switch minimal to --disable-modules for robustness by @[object]
- Expand minimal Botan module list to satisfy librnp by @[object]
- Fix zlib download URL (zlib.net blocks CI) by @[object]
- Always branch off main for the commit job by @[object]
- Add minimal Botan variant (vendored-minimal feature) by @[object]
- Bundle zlib + bzip2 statically too by @[object]
- Bundle Botan statically — fully self-contained prebuilts by @[object]
- Fix typos flagged by crate-ci/typos by @[object]

### Other

- Release v0.1.8 by @[object]
- Update prebuilt static libraries for all platforms ([#46](https://github.com/rnpgp/rnp-rs/pull/46)) by @[object]
- Fetch chore/prebuilt-update before force-with-lease by @[object]
- Switch minimal to --disable-modules for robustness by @[object]
- Expand minimal Botan module list to satisfy librnp by @[object]
- Fix zlib download URL (zlib.net blocks CI) by @[object]
- Always branch off main for the commit job by @[object]
- Add minimal Botan variant (vendored-minimal feature) by @[object]
- Bundle zlib + bzip2 statically too by @[object]
- Bundle Botan statically — fully self-contained prebuilts by @[object]
- Fix typos flagged by crate-ci/typos by @[object]

### Other

- Update prebuilt static libraries for all platforms ([#46](https://github.com/rnpgp/rnp-rs/pull/46)) by @[object]
- Fetch chore/prebuilt-update before force-with-lease by @[object]
- Switch minimal to --disable-modules for robustness by @[object]
- Expand minimal Botan module list to satisfy librnp by @[object]
- Fix zlib download URL (zlib.net blocks CI) by @[object]
- Always branch off main for the commit job by @[object]
- Add minimal Botan variant (vendored-minimal feature) by @[object]
- Bundle zlib + bzip2 statically too by @[object]
- Bundle Botan statically — fully self-contained prebuilts by @[object]
- Fix typos flagged by crate-ci/typos by @[object]

### Added

- Close remaining API gaps for librnp parity by @[object]

### Other

- Drop null merge_group and add explicit permissions by @[object]

### Fixed

- Add -include cstring for GCC 13 in release.yml ([#37](https://github.com/rnpgp/rnp-rs/pull/37)) by @[object]

### Other

- Update prebuilt static libraries for all platforms ([#36](https://github.com/rnpgp/rnp-rs/pull/36)) by @[object]

### Other

- Update prebuilt static libraries for all platforms ([#30](https://github.com/rnpgp/rnp-rs/pull/30)) by @[object]

### Other

- Add prebuilt static libs for x86_64-unknown-linux-gnu ([#26](https://github.com/rnpgp/rnp-rs/pull/26)) by @[object]

### Other

- Update all GitHub Actions to latest stable versions ([#20](https://github.com/rnpgp/rnp-rs/pull/20)) by @[object]
- Add rnp-sexp 0.1 from crates.io as dev-dependency ([#19](https://github.com/rnpgp/rnp-rs/pull/19)) by @[object]

### Other

- Release v0.1.1 ([#13](https://github.com/rnpgp/rnp-rs/pull/13)) by @[object]

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
