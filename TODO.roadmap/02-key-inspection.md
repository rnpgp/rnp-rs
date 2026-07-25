# 02 — Key inspection: read-only getters and child-handle types

- **Priority:** P0 — needed by anyone inspecting a key after load
- **Status:** done (this session)
- **Blocked by:** 01

## Context

`Key` today exposes `export` and `primary_uid`. The C API has ~35 `rnp_key_get_*`
accessors plus three child-handle families: `Uid`, `Signature`, and `Subkey`.
This phase covers all read-only inspection — no mutation, no generation.

## Work items

### Key scalar getters (`rnp_key_get_*`)

- [ ] `alg`, `bits`, `curve`, `dsa_qbits`, `version`, `creation`, `expiration`
- [ ] `keyid`, `fprint`, `grip`, `primary_fprint`, `primary_grip`
- [ ] `protection_mode`, `protection_type`, `protection_cipher`,
      `protection_hash`, `protection_iterations`
- [ ] `revocation_reason`, `revocation_signature`
- [ ] `allows_usage` (key flags word)
- [ ] Booleans: `is_primary`, `is_sub`, `is_locked`, `is_protected`,
      `have_public`, `have_secret`, `is_valid`, `is_revoked`, `is_compromised`,
      `is_expired`, `is_superseded`, `is_retired`
- [ ] `valid_till`, `valid_till64`
- [ ] Revocation revokers: `get_revoker_count`, `get_revoker_at`

### Keyring counts

- [ ] `rnp_get_public_key_count`, `rnp_get_secret_key_count` on `Context`.

### UID handle (`rnp_uid_*`)

- [ ] `Uid<'key>` type with `from_handle` + `Drop`.
- [ ] `Key::uids()` returning an iterator or `Vec<Uid>`.
- [ ] `Uid::data`, `Uid::uid_type` (`UserId` / `UserAttr`), `Uid::is_primary`,
      `Uid::is_valid`, `Uid::is_revoked`, `Uid::revocation_signature`,
      `Uid::signatures()`.
- [ ] `rnp_uid_remove` lives in phase 03 (mutation).

### Subkey handle

- [ ] `Subkey<'key>` type. A subkey *is* a key, so model it as
      `Subkey<'key>(Key<'key>)` (newtype) rather than duplicating the getter
      surface. Deref or explicit forwarding — pick whichever the borrow checker
      is happiest with.
- [ ] `Key::subkeys()` returning `Vec<Subkey>`.

### Signature handle (`rnp_signature_*`)

- [ ] `Signature<'key>` type. Wraps `rnp_signature_handle_t`.
- [ ] Full getter surface: `alg`, `creation`, `expiration`, `features`,
      `hash_alg`, `key_expiration`, `key_flags`, `key_fprint`, `key_server`,
      `key_server_prefs`, `keyid`, `preferred_{alg,hash,zalg}` (with counts),
      `primary_uid`, `revocation_reason`, `revoker`, `signer`, `trust_level`,
      `type`.
- [ ] `Signature::is_valid`, `signature_error_at` / `signature_error_count`
      (from phase 08's verification surface, but the handles themselves can
      land here).
- [ ] Subpackets: `Subpacket` type. `subpacket_count`, `subpacket_at`,
      `subpacket_find`, `subpacket_info`, `subpacket_data`, `subpacket_destroy`.
- [ ] `Signature::to_json`, `Signature::packet_to_json` (depends on phase 07).

### Identifier iteration

- [ ] `IdentifierIterator` over `rnp_identifier_iterator_{create,next,destroy}`,
      iterating by `userid` / `keyid` / `fingerprint` / `grip`.

## Architecture notes

**Module layout:** create `src/key/` as a directory module. Move the existing
`Key` and `KeyIdentifier` into `src/key/{handle,identifier}.rs`. New child
handles get their own files: `src/key/{uid,subkey,signature,subpacket}.rs`.
Re-export the public surface from `src/key/mod.rs`.

**Newtype for `Subkey`:** A subkey in librnp is a full `rnp_key_handle_t` — it
has every getter a primary key has. Modeling `Subkey` as a thin wrapper around
`Key` (rather than a parallel type) is DRY and avoids the trap of "I added a
getter to `Key` but forgot `Subkey`."

**Lifetime chain:** `Key<'ctx>` borrows `Context`. `Uid<'key>`, `Subkey<'key>`,
`Signature<'key>` borrow `Key<'ctx>`, so their lifetime parameter is `'key`.
The invariant: a child handle is destroyed before its parent. `Drop` is enough
to enforce this in Rust's borrow discipline.

**Iterator ergonomics:** `Key::uids()` returns `Vec<Uid>` rather than a custom
iterator — librnp requires you to call `get_uid_count` first anyway, and a
`Vec` is simpler to use. If allocation becomes hot later, swap for a lazy
iterator without changing call sites.

**`Error::Rnp` with `kind`:** Phase 01's categorized error means `get_*`
returning `RNP_ERROR_NOT_FOUND` can be matched by callers as
`err.kind() == ErrorKind::NotFound`. Don't add bespoke `Option` returns for
each getter — `Result<T>` plus a `NotFound` kind is consistent.

## Acceptance criteria

- Every read-only `rnp_key_get_*`, `rnp_uid_get_*`, `rnp_signature_get_*` is
  wrapped.
- Round-trip test: generate RSA key → load → inspect every field → compare
  with expected values.
- `tests/key_inspect.rs` covers at least: alg, bits, creation, keyid, fprint,
  grip, primary_uid, uid count, uid data, subkey count, subkey alg, signature
  count, signature creation/hash.
- No public API leaks `ffi::*` types.

## Completion log

**DONE** in this session.

- All scalar getters on `Key`: `alg`, `bits`, `curve` (Option), `version`,
  `creation`, `expiration`, `keyid`, `fingerprint`, `grip`, `dsa_qbits`,
  `primary_fprint` (Option), `primary_grip` (Option), `allows_usage` (per
  `KeyUsage`), `valid_till`, `valid_till64`, `protection_{type,mode,cipher,
  hash,iterations}`, `revocation_reason`, `revoker_{count,at}`, `have_{public,
  secret}`, `is_{primary,sub,valid,revoked,locked,protected}`, `25519_bits_
  {tweaked,tweak}`.
- `Uid<'key>` with `uid_type`, `data`, `data_string`, `is_{primary,valid,
  revoked}`, `signature_count`.
- `Subkey<'key>` as newtype over `Key` via `Deref`.
- `Signature<'parent>` handle with `sig_type`, `alg`, `hash_alg`,
  `creation`, `expiration`, `features`, `key_flags`, `key_expiration`,
  `primary_uid`, `keyid`, `key_fprint`, `signer_keyid`, `key_server`,
  `key_server_prefs`, `trust_{level,amount}`, `revocation_reason`,
  `preferred_{ciphers,hashes,compressions}`, `is_valid`, `packet_to_json`,
  and full subpacket enumeration.
- `Subpacket` with `typ`, `is_hashed`, `is_critical`, `data`.
- `IdentifierIterator` over `rnp_identifier_iterator_*` (lazily yielding
  `String`s by `userid`/`keyid`/`grip`/`fingerprint`).
- `Context::public_key_count` / `secret_key_count`.
- `tests/extended.rs` covers the full getter surface, UID enumeration,
  signature enumeration, subpacket inspection, and identifier iteration.
