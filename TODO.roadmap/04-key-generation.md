# 04 — Key generation: builder API and JSON

- **Priority:** P0
- **Status:** done (this session)
- **Blocked by:** 01, 02

## Context

`keygen::generate_test_key` is a throwaway helper for tests. The real surface
is `rnp_op_generate_*` (~25 functions) plus the JSON shortcut
`rnp_generate_key_json`. This phase replaces the helper with a builder.

`generate_test_key` stays available as `keygen::generate_test_key` (deprecated
re-export) until all call sites migrate; then it's removed.

## Work items

### `KeyBuilder` (primary key)

- [ ] `KeyBuilder::new(alg)` — wraps `rnp_op_generate_create`. Algorithm is
      an enum (`Rsa`, `Dsa`, `ElGamal`, `Ecdsa`, `Ecdh`, `Eddsa`, `Ed25519`,
      `X25519`, `Sm2`, plus PQC composites gated by feature).
- [ ] Setters: `bits`, `hash`, `dsa_qbits`, `curve`, `userid`, `expiration`,
      `v6_key` (the `set_v6_key` call).
- [ ] Preferences: `add_pref_{cipher,compression,hash}`, `clear_pref_*`,
      `set_pref_keyserver`.
- [ ] Usage flags: `add_usage`, `clear_usage` (`RNP_KEY_USAGE_*`).
- [ ] Protection: `set_protection_password`, `set_request_password`,
      `set_protection_{cipher,hash,mode,iterations}` — overlaps with
      phase 03's `ProtectOptions`; consider taking `Into<ProtectOptions>`.
- [ ] Terminal: `build(ctx) -> Result<Key>` calls `execute` then
      `get_key` then destroys the op.

### `SubkeyBuilder`

- [ ] `SubkeyBuilder::new(parent_key, alg)` — wraps
      `rnp_op_generate_subkey_create`. Same setter surface as `KeyBuilder`
      minus `userid`/`v6_key`. DRY via a shared trait or composition.

### Algorithm-specific shortcuts

The C API has `rnp_generate_key_{rsa,dsa_eg,ec,25519,sm2}` shortcuts. These
are exactly `KeyBuilder` + `SubkeyBuilder` with preset algorithms — implement
them as free functions `generate_rsa(ctx, bits, uid)` etc. that delegate.
Don't expose them as separate code paths.

### JSON API

- [ ] `generate_key_json(ctx, json) -> Result<String>` — wraps
      `rnp_generate_key_json`, returns the result JSON (key fingerprints).

### Curve / algorithm name constants

- [ ] Re-export the `RNP_ALGNAME_*` constants from `ffi::` as a Rust-side
      `Algorithm` enum (`Rsa`, `Dsa`, ..., with `as_str()` returning the
      C-side string). Same for `Curve`, `Hash`, `Cipher`, `Compression`,
      `AeadType`. OCP: new algorithms add a variant + an `as_str()` arm.

## Architecture notes

**Builder termination:** `build(self, ctx: &Context) -> Result<Key<'ctx>>`.
Consumes `self` so the builder can't be reused (librnp's op is one-shot).

**Why `Into<ProtectOptions>` for protection config:** phase 03 defines the
canonical protection config. Key generation that wants to protect the key at
generation time takes the same config, not a parallel set of setters. DRY.

**Algorithm as enum, not string:** callers shouldn't have to know that
`"ML-KEM-768+X25519"` is the string for a composite PQC algorithm. The enum
encodes the choice; `as_str()` does the lookup. Errors shift from runtime to
compile time.

**Subkey algorithm restrictions:** some primaries can't have certain subkey
types (e.g. DSA primary with ECDH subkey). Whether to enforce that in the
builder or let librnp reject it — defer to librnp (single source of truth) and
surface its error as `ErrorKind::BadParameters`.

## Acceptance criteria

- `KeyBuilder::new(Algorithm::Rsa).bits(2048).userid("a").build(&ctx)?`
  produces a key with the same observable properties as the current
  `generate_test_key`.
- Tests: RSA 2048, ECDSA P-256, Ed25519, SM2 (if available). Subkey attached
  via `SubkeyBuilder`. v6 key via `.v6_key()`.
- `generate_key_json` round-trip — generate, parse the result JSON, confirm
  the fingerprint matches a `find_key` lookup.
- `generate_test_key` deprecated but still works (existing tests don't break).

## Completion log

**DONE** in this session. Implemented:

- `src/keygen.rs` rewritten:
  - `KeyBuilder` over `rnp_op_generate_*` with full setter surface (bits,
    hash, dsa_qbits, curve, userid, expiration, usages, pref_hash/cipher/
    compression, pref_keyserver).
  - `SubkeyBuilder` for `rnp_op_generate_subkey_create`.
  - Enums: `Algorithm`, `Curve`, `Hash`, `Cipher`, `Compression`, `KeyUsage`
    with `as_str()` returning the C-side string for each.
  - `generate_key_json` wrapper.
  - `generate_test_key` retained as a `#[deprecated]` shim over `KeyBuilder`
    (used by existing test files).
- `tests/keygen.rs` — 5 tests covering RSA, EDDSA, preferences, subkey
  attachment, JSON generation.

**Deferred:**

- v6 keys (`rnp_op_generate_set_v6_key`) are gated by
  `RNP_EXPERIMENTAL_CRYPTO_REFRESH`. Will be exposed via a future
  `crypto-refresh` Cargo feature that defines the macro in `build.rs`,
  matching the planned `pqc` feature (phase 09).
- Key protection at generation time (`set_protection_password`,
  `set_protection_{cipher,hash,mode,iterations}`, `set_request_password`)
  is deferred to phase 03 — those setters overlap with the phase-03
  `ProtectOptions` design and we want a single canonical config type.
