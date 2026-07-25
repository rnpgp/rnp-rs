# 12 — Key generation: v6 keys and protection at generation time

- **Priority:** P1
- **Status:** done (this session)
- **Blocked by:** 04

## Context

Phase 04 added `KeyBuilder` over `rnp_op_generate_*` but deferred two
surfaces: v6 keys (gated on `RNP_EXPERIMENTAL_CRYPTO_REFRESH`) and
protection configuration (cipher, hash, mode, iterations, password,
request_password). The protection surface overlaps with phase 03's
`ProtectOptions` — we want one canonical config type, not two.

## Work items

- [ ] `KeyBuilder::protection(impl Into<ProtectOptions>)` — apply the same
      `ProtectOptions` builder used by `Key::protect`. DRY.
- [ ] `KeyBuilder::request_password(bool)` — wraps
      `rnp_op_generate_set_request_password`. When true, librnp will ask
      the password provider at execution time instead of taking an explicit
      password.
- [ ] `KeyBuilder::v6()` under `#[cfg(feature = "crypto-refresh")]` — wraps
      `rnp_op_generate_set_v6_key`.
- [ ] Same on `SubkeyBuilder` minus `v6` (subkeys inherit the primary's
      version).
- [ ] Tests:
  - Generate key with `ProtectOptions` set, then immediately `is_protected`
    returns true.
  - Generate v6 key (feature-gated) — `key.version() == 6`.

## Architecture notes

`ProtectOptions` is the canonical protection config. It's used both for
post-hoc `Key::protect` and now for at-generation `KeyBuilder::protection`.
Adding new protection fields (e.g. AEAD) means extending `ProtectOptions`,
not adding parallel setters on two builders. OCP via composition.

## Acceptance criteria

- Generated+protected key opens with the configured password.
- v6 generation round-trips through `Key::version()`.

## Completion log

**DONE** in this session.

- `KeyBuilder::protection(&ProtectOptions)` and
  `SubkeyBuilder::protection(&ProtectOptions)` — apply the same canonical
  config as `Key::protect` at generation time. DRY via shared
  `ProtectConfig` (private).
- `KeyBuilder::request_password()` / `SubkeyBuilder::request_password()`.
- `KeyBuilder::v6()` under `#[cfg(feature = "crypto-refresh")]`.
- Tests: protected key is `is_protected()` immediately after generation
  and `is_locked()` until `unlock(password)`.
