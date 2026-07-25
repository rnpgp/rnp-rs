# 17 — Misc getters: default subkey and signature signer

- **Priority:** P3 — small surface, easy
- **Status:** done (this session)
- **Blocked by:** 02

## Context

Two small C functions haven't been wrapped:

- `rnp_key_get_default_key(primary, usage, flags, *subkey)` — find the
  default subkey for a given usage on a primary.
- `rnp_signature_get_signer(sig, *key)` — return the signing key handle
  (when present in the keyring) for a signature.

## Work items

- [ ] `Key::default_key_for(usage: KeyUsage) -> Result<Option<Key<'_>>>`
      — wraps `rnp_key_get_default_key`.
- [ ] `Signature::signer_key() -> Result<Option<Key<'_>>>` — wraps
      `rnp_signature_get_signer`. Returns `None` if the signer isn't in
      the keyring.
- [ ] Tests:
  - Generate primary + encrypt subkey. `default_key_for(EncryptComms)`
    returns the subkey.
  - Generate key, find self-certification on UID, `signer_key()` returns
    Some(primary).

## Architecture notes

Both return `Key<'_>` borrowing the parent handle. Lifetimes thread
naturally.

## Acceptance criteria

- `default_key_for(KeyUsage::EncryptComms)` returns the encryption subkey
  after `SubkeyBuilder` is used.
- `signer_key()` on a self-certification returns the primary.

## Completion log

**DONE** in this session.

- `Key::default_key_for(KeyUsage) -> Result<Option<Key<'_>>>` wraps
  `rnp_key_get_default_key`.
- `Signature::signer_key() -> Result<Option<Key<'_>>>` wraps
  `rnp_signature_get_signer`.
- Tests: `default_key_for(EncryptComms)` returns the encryption subkey
  after `SubkeyBuilder`. `signer_key()` on a UID self-certification
  returns the primary.
