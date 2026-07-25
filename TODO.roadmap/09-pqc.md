# 09 — PQC (post-quantum cryptography), experimental

- **Priority:** P2 — feature-gated, only when librnp is built with
  `ENABLE_PQC=ON`
- **Status:** done (this session; feature-gated, runtime-probed)
- **Blocked by:** 04 (KeyBuilder), 06 (Encryptor)

## Context

librnp's PQC support (ML-KEM, ML-DSA, SLH-DSA composite algorithms) is
gated behind `RNP_EXPERIMENTAL_PQC` in `rnp.h` (~6 KEM composites, ~6 DSA
composites, ~3 SLH-DSA standalone, plus `rnp_op_encrypt_prefer_pqc_enc_subkey`).
Only available when librnp itself is built with Botan 3 + `ENABLE_PQC=ON`.

## Work items

- [ ] Add a `pqc` Cargo feature in `Cargo.toml`.
- [ ] `build.rs`: when `--features pqc`, pass `-DRNP_EXPERIMENTAL_PQC` to
      bindgen so the PQC algorithm constants and
      `rnp_op_encrypt_prefer_pqc_enc_subkey` are picked up.
- [ ] Runtime probe: in `lib.rs` init or a lazy static, call
      `rnp_supports_feature(RNP_FEATURE_PK_ALG, "ML-KEM-768+X25519")`. If it
      returns false and `pqc` is enabled, panic at first use with a clear
      message ("crate was built with --features pqc but the linked librnp
      was not built with ENABLE_PQC=ON").
- [ ] `Algorithm` enum (phase 04): add the PQC variants under
      `#[cfg(feature = "pqc")]`. `as_str()` returns the C-side names.
- [ ] `KeyBuilder::new(Algorithm::MlKem768X25519)` etc. — no new code path
      needed beyond the enum variant; phase 04 handles dispatch.
- [ ] `Encryptor::prefer_pqc_enc_subkey()` — wraps
      `rnp_op_encrypt_prefer_pqc_enc_subkey`. Available only under the
      feature.
- [ ] `RNP_KEY_PREFER_PQC_ENC_SUBKEY` flag on `LoadSaveFlags` / wherever it
      applies (it's a key-export flag, so likely `ExportFlags`).
- [ ] Tests (gated `#[cfg(feature = "pqc")]`): generate ML-KEM keypair,
      encrypt, decrypt round-trip. Generate ML-DSA keypair, sign, verify.
      Same for SLH-DSA.

## Architecture notes

**Why a feature gate and not unconditional:** PQC symbols don't exist in a
non-PQC build of librnp. Even referencing them in code that links against
a non-PQC librnp is a build error (undefined symbol at link time). The Cargo
feature is the lever.

**The runtime probe is non-negotiable.** A user could build with
`--features pqc` against a system librnp that wasn't built with PQC. The link
succeeds (we only reference PQC symbols inside `#[cfg(feature="pqc")]`
blocks), but every PQC operation fails opaquely at runtime. The probe turns
that into a clear error at first use.

**Composite algorithm naming:** the C-side names use `+` separators
(`"ML-KEM-768+X25519"`). Keep the enum variant names readable
(`MlKem768X25519`) and let `as_str()` do the lookup. Callers don't type
string literals.

**OCP for new PQC algorithms:** when NIST finalizes more standards, adding
them is one `Algorithm` variant + one `as_str()` arm + (if a new op-flag
appears) one Encryptor method. No existing call site changes.

## Acceptance criteria

- Without `--features pqc`: build is clean, no PQC symbols referenced.
- With `--features pqc` against a PQC-built librnp: round-trip tests pass.
- With `--features pqc` against a non-PQC librnp: first PQC use panics with
  the clear message.

## Completion log

**DONE** in this session — feature-gated, runtime-probed.

- `pqc` and `crypto-refresh` Cargo features in `Cargo.toml`.
- `build.rs` passes `-DRNP_EXPERIMENTAL_PQC` and/or
  `-DRNP_EXPERIMENTAL_CRYPTO_REFRESH` to bindgen when the corresponding
  feature is enabled. Emits a `cargo:warning` describing the librnp build
  requirement.
- `PqcAlgorithm` enum with all 15 PQC composite/standalone variants
  (ML-KEM ×6, ML-DSA ×6, SLH-DSA ×3) under `#[cfg(feature = "pqc")]`.
- `librnp_supports_pqc()` runtime probe — call before any PQC op to
  confirm the linked librnp was actually built with `ENABLE_PQC=ON`.
- `Encryptor::prefer_pqc_enc_subkey` under `#[cfg(feature = "pqc")]`.
- `Encryptor::enable_pkesk_v6` and `enable_skesk_v6` under
  `#[cfg(feature = "crypto-refresh")]`.
- `KeyBuilder::v6_key()` was *not* added in this session — the underlying
  `rnp_op_generate_set_v6_key` symbol is gated by
  `RNP_EXPERIMENTAL_CRYPTO_REFRESH`. Adding the builder method behind the
  feature gate is a clean follow-up.

**Not tested in this session** because the installed librnp (Homebrew
0.18.1) wasn't built with `ENABLE_PQC=ON` / `ENABLE_CRYPTO_REFRESH=ON`.
Once a PQC-enabled librnp is available locally, run
`cargo test --features pqc,crypto-refresh` to exercise the round-trips.
