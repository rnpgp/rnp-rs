# API parity with librnp

`rnp-rs` tracks the public C surface of librnp (`rnp.h`) function by
function. Parity is measured at three levels:

1. **declared** — functions with `RNP_API` linkage in
   `vendor/rnp/include/rnp/rnp.h` (the vendored, pinned librnp version);
2. **bound** — functions present in the pregenerated `rnp-sys` bindings
   (`rnp-sys/bindings/bindings-<version>.rs`);
3. **exercised** — functions with at least one safe call site in this
   crate's `src/` (`ffi::rnp_*`).

## Current status

Against **librnp 0.18.1**:

| Level      | Count |
|------------|-------|
| declared   | 293   |
| bound      | 293   |
| exercised  | 287   |
| excluded (documented below) | 6 |

Every function is accounted for: 287 have a safe call site, 6 are
consciously excluded with reasons. Callback *typedefs*
(`rnp_input_reader_t`, `rnp_input_closer_t`, `rnp_output_writer_t`,
`rnp_output_closer_t`) are implemented as C thunks in
`src/ops/io/stream.rs`, not counted as functions.

## Keeping it honest

- `tests/ffi_parity.rs` re-runs the audit in CI. When bindings regenerate
  against a newer librnp, any new function fails the test until it is
  wrapped or added to the test's `EXCLUDED` list with a reason.
- `scripts/parity-audit.sh` prints the same numbers for humans:

  ```sh
  ./scripts/parity-audit.sh
  ```

## Exclusions

| Function | Reason |
|----------|--------|
| `rnp_op_generate_clear_usage` | `KeyBuilder` defers every setter to build time and replays its own vectors, so `KeyBuilder::clear_usage` (clearing the vector) is exactly the C-side clear. |
| `rnp_op_generate_clear_pref_hashes` | Same: `KeyBuilder::clear_pref_hash`. |
| `rnp_op_generate_clear_pref_ciphers` | Same: `KeyBuilder::clear_pref_cipher`. |
| `rnp_op_generate_clear_pref_compression` | Same: `KeyBuilder::clear_pref_compression`. |
| `rnp_key_sphincsplus_get_param` | Removed on librnp main; exists only in 0.18.1's experimental PQC surface. The `pqc`/`crypto-refresh` features compile librnp from HEAD, where the symbol no longer exists, so a wrapper could not be built for that flavor. SLH-DSA parameter sets are chosen via the `SLH-DSA-SHA2-*` algorithm names. |
| `rnp_op_generate_set_sphincsplus_param` | Same as above. |

## Where to find each C function in Rust

| C prefix / family | Rust surface |
|-------------------|--------------|
| `rnp_ffi_create/destroy`, `rnp_ffi_set_*_provider` | [`Context`](../src/context.rs), `callbacks` module |
| `rnp_input_*`, `rnp_output_*` | `Input` / `Output` (`ops::io`), incl. `from_reader` / `to_writer` streaming |
| `rnp_op_sign_*` | `Signer` builder + `sign` / `sign_detached` / `sign_cleartext` |
| `rnp_op_verify_*` | `VerifyOp` / `VerifyResult` / `VerifySignature` |
| `rnp_op_encrypt_*` | `Encryptor` builder |
| `rnp_decrypt` | `decrypt`, `decrypt_to`, `decrypt_from_input`, `Decryptor` |
| `rnp_op_generate_*` | `KeyBuilder` / `SubkeyBuilder`; one-call `generate_key_{rsa,dsa_eg,ec,sm2,25519,ex}` shorthands |
| `rnp_generate_key_*` (shorthands) | `keygen::generate_key_*` free functions |
| `rnp_key_get_*`, `rnp_key_is_*` | `Key` getters and predicates (`key::inspection`) |
| `rnp_key_{protect,unprotect,lock,unlock,revoke,...}` | `Key` mutators (`key::mutation`) |
| `rnp_key_signature_*` (create/set) | `CertificationBuilder`, `DirectSignatureBuilder`, `RevocationSignatureBuilder` |
| `rnp_uid_*` | `Uid` |
| `rnp_signature_*` | `Signature` (inspection, subpackets, errors) |
| `rnp_recipient_*`, `rnp_symenc_*` | `Recipient`, `Symenc` |
| `rnp_{load,save,import,unload}_*` | `Context::load_keys*`, `save_keys*`, `import_keys*`, `unload_keys` |
| `rnp_locate_key`, `rnp_identifier_iterator_*` | `find_key` / `KeyIdentifier`, `IdentifierIterator` |
| `rnp_enarmor`, `rnp_dearmor`, `rnp_guess_contents` | `enarmor` / `dearmor` / `guess_contents` |
| `rnp_dump_*`, `*_to_json` | `dump` module |
| `rnp_{add,get,remove}_security_rule` | `security` module |
| `rnp_version_*`, `rnp_backend_*` | `version` module (`rnp::version_string()`) |
| `rnp_buffer_*` | internal (`ffi_safe`); `SecretString` for zero-on-drop secrets |
| `rnp_enable_debug` / `rnp_disable_debug` | `enable_debug` / `disable_debug` (`version`) |
| `rnp_calculate_iterations`, `rnp_set_timestamp`, `rnp_request_password` | `security::calculate_iterations`, `set_timestamp`, `request_password` |
| `rnp_supported_features`, `rnp_supports_feature` | `supported_features`, `supports_feature` |

PQC-only entry points (e.g. `rnp_op_encrypt_prefer_pqc_enc_subkey`) are
gated behind the `pqc` Cargo feature, mirroring upstream's
`RNP_EXPERIMENTAL_PQC` build gate.
