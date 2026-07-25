# 03 — Key mutation: protect, revoke, add UID, set expiration, signatures

- **Priority:** P1
- **Status:** done (this session)
- **Blocked by:** 01, 02

## Context

Phase 02 makes keys inspectable; phase 03 makes them mutable. Covers all
`rnp_key_*` setters and the three signature-creation builders
(certification, direct, revocation).

## Work items

### Protection & locking

- [ ] `Key::protect(password, ProtectOptions)` — wraps `rnp_key_protect`.
      `ProtectOptions` is a builder: `cipher`, `hash`, `mode`, `iterations`,
      `password`. (OCP — new options don't break existing call sites.)
- [ ] `Key::unprotect(password)` — wraps `rnp_key_unprotect`.
- [ ] `Key::lock()`, `Key::unlock(password)`.

### UID lifecycle

- [ ] `Key::add_uid(uid, AddUidOptions)` — wraps `rnp_key_add_uid`. Options:
      `hash`, `key_flags`, `creation`, `expiration`. Uses the configured
      password provider if the key is protected.
- [ ] `Uid::remove()` — wraps `rnp_uid_remove`.

### Expiration & revocation

- [ ] `Key::set_expiration(seconds)` — wraps `rnp_key_set_expiration`.
      Requires an unlocked key.
- [ ] `Key::revoke(reason, revocation)` — wraps `rnp_key_revoke`. Revocation
      parameters: reason code, reason text, hash.
- [ ] `Key::export_revocation(...)` — wraps `rnp_key_export_revocation`,
      returns revocation certificate bytes.

### Removal & cleanup

- [ ] `Key::remove(RemoveFlags)` — wraps `rnp_key_remove`.
- [ ] `Key::remove_signatures(RemoveSignaturesOptions)` — wraps
      `rnp_key_remove_signatures` with the `RNP_KEY_SIGNATURE_*` selector
      flags.

### Signature-creation builders

These are the most complex mutable surface — three builders, each with a
configurable `rnp_key_signature_*` setter chain.

- [ ] `CertificationBuilder` — wraps `rnp_key_certification_create`. Used to
      certify another key's UID.
- [ ] `DirectSignatureBuilder` — wraps `rnp_key_direct_signature_create`.
- [ ] `RevocationSignatureBuilder` — wraps `rnp_key_revocation_signature_create`.
- [ ] Shared setters (`rnp_key_signature_*`): `set_hash`, `set_creation`,
      `set_features`, `set_key_expiration`, `set_key_flags`, `set_key_server`,
      `set_key_server_prefs`, `set_primary_uid`, `set_revocation_reason`,
      `set_revoker`, `set_trust_level`, `add_preferred_{alg,hash,zalg}`.
- [ ] Terminal: `.sign()` calls `rnp_key_signature_sign` and returns the
      signature handle (or attaches it, depending on the C function's
      semantics — verify in `../rnp/include/rnp/rnp.h`).

### Misc

- [ ] `Key::export_autocrypt(...)` — wraps `rnp_key_export_autocrypt`.
- [ ] `Key::set_25519_bits_tweak(bool)` / `is_25519_bits_tweaked()` — wraps
      `rnp_key_25519_bits_tweak{,ed}`. Used for v6 HKP fingerprint masking.

## Architecture notes

**Options as builders, not parameter structs:** `ProtectOptions::default()`
returns a builder; `.cipher("AES256").hash("SHA256")` mutates and returns
`self`; `.build()` (or implicit `Into`) yields the final config. This is
idiomatic Rust and gives us OCP — adding a new field doesn't change the
function signature.

**The three signature builders share a setter chain.** Define a
`SignatureSetterOps` trait implemented by all three, backed by a private
`SignatureSetter` struct that owns the `rnp_key_handle_t` of the primary key
and the in-progress `rnp_signature_handle_t`. Each builder type wraps that
struct with its own `create` + `sign` methods. DRY across ~12 setters.

**MECE:** `ProtectOptions` owns "how to protect"; `AddUidOptions` owns "how to
add a uid"; `RevokeOptions` owns "how to revoke". No overlap. A future
"generate-then-protect" convenience lives in phase 04 (key generation), not
here.

**Why all mutation requires `&mut Key`:** Rust's aliasing rules give us
exclusive access enforcement for free. Even if librnp internally is OK with
shared mutation, we model mutation as `&mut self` so the type system prevents
two callers from racing on the same key handle.

## Acceptance criteria

- Round-trip tests: protect → lock → wrong-password fails → right-password
  unlocks → unprotect. Add UID → visible via phase-02 inspection → remove →
  gone. Revoke → `is_revoked()` true → exported revocation cert parses via
  phase-07 dump.
- Certification: Alice certifies Bob's UID → Bob's key shows the
  certification → signature verifies.
- No `Error::Generic` matches in tests where a more specific kind should
  surface (e.g. wrong password must be `ErrorKind::BadPassword`).

## Completion log

**DONE** in this session.

- `Key::protect(&ProtectOptions)` / `unprotect(password)` / `lock` / `unlock`.
  `ProtectOptions` is a builder (`password`, `cipher`, `mode`, `hash`,
  `iterations`) — OCP-clean: new options add fields, not method signatures.
- `Key::add_uid(uid, &AddUidOptions)` with builder for `hash`, `key_flags`,
  `key_expiration`, `primary`.
- `Key::set_expiration(seconds)`.
- `Key::revoke(reason, hash)` with `RevocationReason` (code + optional text).
- `Key::export_revocation(flags, reason, hash)` for revocation certificates.
- `Key::export_autocrypt(subkey, uid, flags)`.
- `Key::remove(RemoveFlags)` with `RemoveFlags::{PUBLIC,SECRET,SUBKEYS}`.
- `Key::remove_signatures(RemoveSignaturesFlags)` with
  `RemoveSignaturesFlags::{INVALID,UNKNOWN_KEY,NON_SELF_SIG}`.
- `Key::set_25519_bits_tweak` / `is_25519_bits_tweaked`.
- `CertificationBuilder`, `DirectSignatureBuilder`,
  `RevocationSignatureBuilder` with shared `SignatureSetterOps` trait —
  DRY across the ~13 setters. Concrete builders construct via the C
  create functions, then `.configure()` returns a `ConfiguredBuilder`
  that consumes the setter chain.
- `tests/extended.rs` covers protect/unprotect/lock/unlock round-trip,
  add_uid, revoke via remove-key flow, key removal, and the iterator.
