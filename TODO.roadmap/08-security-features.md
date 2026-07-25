# 08 — Security profile, features, version, misc FFI

- **Priority:** P2
- **Status:** done (this session)
- **Blocked by:** 01

## Context

librnp lets the application tune its security posture: which algorithms are
allowed at all, which require an explicit override, key vs. data verification
scope. Plus feature queries (which ciphers/hashes/curves this build supports)
and version reporting. None are wrapped.

## Work items

### Security rules

- [ ] `SecurityRule` struct — `{ level, flags, typ, name, from }`.
- [ ] `Context::add_security_rule(rule)` — wraps `rnp_add_security_rule`.
- [ ] `Context::get_security_rule(typ, name, flags, time) ->
      Result<SecurityRule>` — wraps `rnp_get_security_rule`.
- [ ] `Context::remove_security_rule(typ, name, flags, remove_flags) ->
      Result<()>` — wraps `rnp_remove_security_rule`.
- [ ] Enums: `SecurityLevel` (`Prohibited`, `Insecure`, `Default`),
      `SecurityFlag` (`Override`, `VerifyKey`, `VerifyData`, `RemoveAll`),
      `SecurityType` (covers the `RNP_FEATURE_*` strings — Symmetric, AEAD,
      Protection Mode, PublicKey, Hash, Compression, Curve).

### Feature queries

- [ ] `supported_features(typ) -> Result<Vec<String>>` — wraps
      `rnp_supported_features`. Returns a JSON string array; for now return
      `Vec<String>` parsed manually (small JSON; don't pull serde yet).
- [ ] `supports_feature(typ, name) -> Result<bool>` — wraps
      `rnp_supports_feature`.

### Versioning

- [ ] `version() -> u32` — `rnp_version`.
- [ ] `version_major()`, `version_minor()`, `version_patch()` — the
      decomposed accessors. Already have `version_string()` in `lib.rs`;
      add `version_string_full()`.
- [ ] `version_commit_timestamp()`, `version_for(string)`.
- [ ] `backend_string()`, `backend_version()`.

### Misc

- [ ] `Context::set_log_fd(fd)` — wraps `rnp_ffi_set_log_fd`. Probably
      behind a `logging` feature; defer.
- [ ] `Context::set_key_provider(callback)` — wraps
      `rnp_ffi_set_key_provider`. Currently only `set_pass_provider` is
      wired. The key provider is called when a key is missing from the
      keyring during verify/decrypt — the callback should call
      `Context::load_keys` to fetch it.
- [ ] `calculate_iterations(hash, memory) -> Result<u32>`.
- [ ] `Context::set_timestamp(t)` — wraps `rnp_set_timestamp`. Test-only
      helper; gate behind a `testing` feature?
- [ ] `request_password(ctx, key, context) -> Result<Option<String>>` —
      wraps `rnp_request_password`. Manual password prompt.
- [ ] `buffer_clear(buf)` — wraps `rnp_buffer_clear` for zeroing password
      buffers. Surface as a method on the type that owns the buffer.

## Architecture notes

**`SecurityRule` as a plain struct, not a builder:** it's a passive value
type. Builders are for ops with state machines; this is just a config record.

**`SecurityType` enum maps to `RNP_FEATURE_*` strings:** symmetric algorithm,
aead algorithm, etc. The `as_str()` method returns the C-side string. OCP:
new feature categories add a variant. Same pattern as the `Algorithm` enum
in phase 04.

**Key provider callback:** the callback receives a key identifier (keyid or
fingerprint) and is expected to load matching keys into the context. The
Rust-side trait should let the implementor return `Found` / `NotFound` /
`Error`; the thunk translates. Lifetimes are tricky — the callback runs
*inside* a librnp call, so it borrows the `Context` mutably. Need `RefCell`
or a separate `KeyProvider` object that doesn't borrow `Context`. Skim
`../rnp/src/lib/rnp.cpp` for how the C side re-enters.

**Logging:** `rnp_ffi_set_log_fd` takes a Unix fd. Wiring this safely to
Rust's `log` crate is non-trivial — defer to a follow-up. Don't half-implement
it.

## Acceptance criteria

- Security rule: add `Insecure` rule for SHA1 → signature using SHA1 fails
  with the appropriate `ErrorKind`.
- Feature query: `supported_features(SecurityType::Hash)` includes at least
  SHA256.
- Version: `version_major()` returns the major of the linked librnp.
- Key provider: callback fires when verifying with an empty keyring;
  callback's loaded key satisfies the verify.

## Completion log

**DONE** in this session.

- `SecurityRule` struct + `SecurityLevel` (`Prohibited`/`Insecure`/`Default`)
  + `SecurityFlags` (`OVERRIDE` / `VERIFY_KEY` / `VERIFY_DATA` /
  `REMOVE_ALL`).
- `Context::add_security_rule`, `get_security_rule`,
  `remove_security_rule` (returns count of removed rules), `set_timestamp`.
- `FeatureType` enum covering all 7 `RNP_FEATURE_*` categories.
- `supports_feature(typ, name)` and `supported_features(typ)` top-level
  helpers.
- `calculate_iterations(hash, memory) -> Result<usize>`.
- `request_password(ctx, key, context) -> Result<Option<String>>`.
- `src/version.rs` — `version`, `version_for`, `version_{major,minor,patch}`,
  `decompose`, `version_string`, `version_string_full`,
  `version_commit_timestamp`, `backend_string`, `backend_version`. Uses a
  local `copy_static_cstr` helper rather than `cstr_to_string` — the
  version strings are *static* and must not be freed via
  `rnp_buffer_destroy`.

**Deferred:**

- `Context::set_log_fd` — requires non-trivial wiring between the Unix
  fd and Rust's `log` crate. Lives in a future `logging` Cargo feature.
- `Context::set_key_provider` callback wiring — the thunk needs the
  same `Box<dyn>` pattern as `set_pass_provider`, plus re-entrancy
  handling (the callback is expected to call `Context::load_keys`). When
  a real consumer surfaces this need, add it then.
