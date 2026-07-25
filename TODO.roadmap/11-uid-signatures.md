# 11 — UID signature enumeration

- **Priority:** P1 — completes the inspection surface
- **Status:** done (this session)
- **Blocked by:** 02

## Context

`Key::signatures()` returns signatures on the *key*, but in OpenPGP the
self-certifications and third-party certifications live on the *UID*, not the
primary. Phase 02 wrapped `rnp_uid_get_signature_count` but not
`rnp_uid_get_signature_at`. The result is that callers can see a count but
can't actually inspect the UID's binding signatures — which is where most
of the interesting metadata (key flags, preferred algorithms, expiration)
actually lives.

## Work items

- [ ] Add `Signature<'uid>` borrow variant — currently `Signature<'parent>`
      is generic over the parent. Either keep it generic or split into
      `KeySignature` / `UidSignature` types. Pick the simpler one.
- [ ] `Uid::signature_at(idx) -> Result<Option<Signature>>`
- [ ] `Uid::signatures() -> Result<Vec<Signature>>`
- [ ] `Uid::revocation_signature() -> Result<Option<Signature>>` — wraps
      `rnp_uid_get_revocation_signature`.
- [ ] Tests: generate key with multiple UIDs, walk signatures on each,
      verify at least one is a self-certification with key flags.

## Architecture notes

`Signature<'parent>` already uses `PhantomData<&'parent ()>` — it doesn't
care whether the parent is a Key or a Uid. The single generic type works.

## Acceptance criteria

- A freshly-generated RSA key's first UID has at least one signature.
- That signature's `key_flags()` is non-zero (self-certification with
  usage flags).
- UID revocation signatures return None on a non-revoked UID.

## Completion log

**DONE** in this session.

- `Uid::signature_at(idx)`, `Uid::signatures()`, `Uid::revocation_signature()`
  added to `src/uid.rs`. Return `Signature<'_>` borrowing the UID (the
  generic `Signature<'parent>` type already supports it via `PhantomData`).
- Tests in `tests/parity_gaps.rs`: self-certification has non-zero
  `key_flags`; non-revoked UID returns `None` from
  `revocation_signature()`.
