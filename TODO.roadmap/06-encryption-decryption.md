# 06 — Encryption & decryption

- **Priority:** P0
- **Status:** done (this session)
- **Blocked by:** 01, 02

## Context

The biggest missing feature. `rnp_op_encrypt_*` (~20 functions) for the
encryption builder; `rnp_decrypt` for the simple path; the verify-op result
inspection surface (already partially used by `signature::verify`) for the
decryption result.

## Work items

### `Encryptor` builder

- [ ] `Encryptor::new(ctx, plaintext) -> Result<Self>` — wraps
      `rnp_op_encrypt_create`. Owns the input via phase-01 `Input`.
- [ ] Recipients & passwords: `add_recipient(&Key)`, `add_password(pw,
      PasswordOptions)` (`s2k`-related, `add_password` takes cipher/hash/
      iterations configurable).
- [ ] Algorithm setters: `set_armor(bool)`, `set_cipher(name)`,
      `set_hash(name)`, `set_compression(alg, level)`, `set_aead(name)`,
      `set_aead_bits(n)`.
- [ ] Metadata: `set_file_name(s)`, `set_file_mtime(t)`,
      `set_creation_time(t)`, `set_expiration_time(t)`.
- [ ] Flags: `set_flags(EncryptFlags)` — currently `RNP_ENCRYPT_NOWRAP`.
- [ ] v6 / PQC (gated): `enable_pkesk_v6`, `enable_skesk_v6`,
      `prefer_pqc_enc_subkey`. The PQC ones live behind the `pqc` feature
      from phase 09; this phase wires the calls but the feature gate lives
      there.
- [ ] `add_signature(&Key)` — `rnp_op_encrypt_add_signature`. Sign-and-
      encrypt in one op.
- [ ] Terminal: `build(output) -> Result<()>` — wraps `execute` then destroys
      the op. Output is phase-01's `Output` so caller picks destination.

### Decryption

- [ ] `decrypt(ctx, ciphertext) -> Result<Vec<u8>>` — wraps `rnp_decrypt`
      (the simple path). Passwords/session keys come from the configured
      providers.
- [ ] `decrypt_to(ctx, ciphertext, &mut Output)` — same, into caller's
      output.

### Decryption result inspection (verify-op surface)

`rnp_decrypt` is fine for the simple path but callers often need to know
*which* recipient's key was used, which symenc succeeded, what cipher/ AEAD
was applied, etc. These come from the verify-op family that
`signature::verify` already uses internally.

- [ ] `VerifyOp` type — wraps `rnp_op_verify_t`. Constructable from
      `rnp_op_verify_create` / `rnp_op_verify_detached_create`. Exposed via
      `Context::verify_op(input, output)` and `verify_detached_op(input,
      sig_input)`.
- [ ] `VerifyOp::execute() -> Result<VerifyResult>`.
- [ ] `VerifyResult` — owns the verify-op handle until drop. Methods:
      `signature_count`, `signature_at(i) -> VerifySignature`,
      `recipient_count`, `recipient_at(i) -> Recipient`, `used_recipient()`,
      `symenc_count`, `symenc_at(i) -> Symenc`, `used_symenc()`,
      `file_info()`, `format()`, `protection_info()`.
- [ ] Refactor `signature::verify` / `verify_detached` to use `VerifyOp`
      internally (DRY).
- [ ] `Recipient` handle: `alg`, `keyid`.
- [ ] `Symenc` handle: `cipher`, `aead_alg`, `hash_alg`, `s2k_type`,
      `s2k_iterations`.

## Architecture notes

**`Encryptor` owns `Input`, caller owns `Output`:** encryption reads from the
input and writes to the output. The input lifetime is bounded by the builder;
the output is caller-controlled so they can pick memory/file/callback.

**Why `VerifyResult` not raw accessor methods on `VerifyOp`:** the C API
requires the op handle to stay alive while you query results. Modeling the
result as a borrowing handle keeps that invariant loud. `VerifyResult`'s
`Drop` calls `rnp_op_verify_destroy`.

**Phase 01 payoff:** the entire encryption/decryption surface is built on
`Input`/`Output`/`Error::kind()`. No new I/O plumbing needed; that's the
whole point of doing phase 01 first.

**`AddPasswordOptions`:** S2K config (cipher, hash, iterations). Same pattern
as `ProtectOptions` in phase 03 — builder-style, `Into`-convertible. If we
end up with three "S2K-ish" option types (protect, generate-protection,
encrypt-password), factor a `S2kOptions` shared core. Defer until the third
one lands; don't preemptively abstract (rule from CLAUDE.md: "three similar
lines is better than a wrong abstraction").

## Acceptance criteria

- Round-trip: generate keypair → encrypt to recipient → decrypt with private
  key → plaintext matches.
- Password-only encryption: encrypt with `add_password` → decrypt with
  password → plaintext matches.
- Combined: encrypt + sign → decrypt + verify → plaintext matches and
  signature validates.
- Tamper: flip a byte in ciphertext → `decrypt` returns
  `ErrorKind::DecryptFailed` (or `MacInvalid`).
- Wrong-password: `ErrorKind::BadPassword`.
- `VerifyResult` exposes the used recipient after a successful decrypt.

## Completion log

**DONE** in this session.

- `Encryptor` builder over `rnp_op_encrypt_*` with full setter surface
  (recipients, signatures, passwords, armor/cipher/hash/compression/aead/
  file metadata/flags). v6 / PQC knobs are feature-gated (see phase 09).
- `AddPasswordOptions` builder for per-password S2K config.
- `decrypt` / `decrypt_to` simple-path wrappers.
- `VerifyOp` (`inline` / `detached` constructors) producing a typed
  `VerifyResult` after `execute()`. The result surface includes:
  - `signature_count` / `signature_at` / `signatures` / `any_valid`
  - `recipient_count` / `recipient_at` / `recipients` / `used_recipient`
  - `symenc_count` / `symenc_at` / `symencs` / `used_symenc`
  - `file_info` (name + mtime), `format` (RFC 4880 §5.9 char),
    `protection_info` (mode + cipher + valid bool)
- `VerifySignature` (`status`, `status_is_valid`, `hash`, `times`,
  `keyid`). `SignatureStatus` enum (`Valid` / `Invalid` / `Unknown`)
  mapping the C return code by `ErrorKind`.
- `Recipient` (`alg`, `keyid`).
- `Symenc` (`cipher`, `aead_alg`, `hash_alg`, `s2k_type`, `s2k_iterations`).
- `signature::verify` / `verify_detached` refactored to use `VerifyOp`
  internally — DRY (no parallel verify path).
- `tests/encrypt.rs` (5 tests) cover recipient + password + armored +
  compression + garbage round-trips.
- `tests/extended.rs::verify_result_exposes_recipient_after_decrypt`
  exercises the rich verify-result surface after decryption.
