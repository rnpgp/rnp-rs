# 14 — Verify result: per-signature handle and signer key

- **Priority:** P2
- **Status:** done (this session)
- **Blocked by:** 06

## Context

`VerifySignature` currently exposes only `status()`, `hash()`, `times()`,
and a placeholder `keyid()` that returns an empty string. The C API
provides `rnp_op_verify_signature_get_handle` (returns the
`rnp_signature_handle_t` for full inspection) and
`rnp_op_verify_signature_get_key` (returns the signer's `rnp_key_handle_t`).
Surfacing these lets callers inspect the signing key without a separate
`find_key` round-trip.

## Work items

- [ ] `VerifySignature::handle() -> Result<Signature<'_>>` — wraps
      `rnp_op_verify_signature_get_handle`. Returns a typed
      [`Signature`] for full getter access (subpackets, key flags, etc.).
- [ ] `VerifySignature::key() -> Result<Option<Key<'_>>>` — wraps
      `rnp_op_verify_signature_get_key`. Returns `None` when the signer's
      key isn't in the keyring.
- [ ] Replace the `VerifySignature::keyid()` placeholder with a real
      implementation via the `Signature` handle.
- [ ] Tests: encrypt + sign to a known key, decrypt, walk verify result's
      signatures, confirm `key().unwrap().keyid()` matches the signing
      key's id.

## Architecture notes

`VerifySignature` borrows the parent `VerifyResult`. The returned
`Signature` and `Key` borrow the `VerifySignature` — three-level lifetime
chain. The existing `Signature<'parent>` already uses generic
`PhantomData<&'parent ()>`, so this just works.

For `Key<'_>` returned from `VerifySignature::key`: the lifetime must be
tied to the verify op (because the underlying handle is owned by the
result). Use `Key<'_>` with elided lifetime; callers get a key that lives
as long as the result.

## Acceptance criteria

- `key()` returns `Some` for a signing key already in the keyring.
- `key()` returns `None` for a signature from an unknown signer.
- `handle()` returns a Signature whose `alg()` matches the signing key's
  algorithm.

## Completion log

**DONE** in this session.

- `VerifySignature::handle()` returns a typed `Signature<'_>` borrowing
  the verify signature.
- `VerifySignature::key()` returns `Option<Key<'_>>` — `None` when the
  signer isn't in the keyring.
- `VerifySignature::keyid()` now delegates through `handle()` rather than
  returning an empty placeholder.
- Tests: signed message's verify result exposes the signing key with a
  keyid matching the signing key.
