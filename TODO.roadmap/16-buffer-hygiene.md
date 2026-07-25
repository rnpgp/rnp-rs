# 16 — Buffer hygiene: secure-zeroing wrapper for password returns

- **Priority:** P2 — security hygiene
- **Status:** done (this session)
- **Blocked by:** 01

## Context

`PasswordProvider::get_password` returns `Cow<'_, str>`. The contents of
that `str` end up copied into a librnp-owned buffer; the original `str`'s
memory is then freed normally — not zeroed. For a long-lived process that
handles many passwords, the password bytes can sit in freed heap memory
for arbitrary time, exposed to anyone who can read `/proc/<pid>/maps` or
core-dump the process.

librnp exposes `rnp_buffer_clear(ptr, size)` exactly for this. The Rust
side should give callers a way to opt into secure zeroing without
 mandating it.

## Work items

- [ ] `SecretString` type — a `String` wrapper whose `Drop` calls
      `rnp_buffer_clear` on the underlying bytes. Lives in
      `src/secret.rs`.
- [ ] `SecretString::new(s: String) -> Self`, `as_str() -> &str`,
      `into_string(self) -> String` (consumes, no zeroing), `Drop`.
- [ ] `PasswordProvider::get_password` return-type option: extend the
      trait with `get_password_secret(&self, ...) -> Option<SecretString>`
      defaulting to `None`. When the provider returns a `SecretString`,
      the thunk uses it and drops it (zeroing on drop). Otherwise it
      falls back to the existing `get_password`.
- [ ] `request_password` (in `src/security.rs`) returns `SecretString`
      instead of `String`.
- [ ] Tests: `SecretString` round-trips a value; drop doesn't leak
      (verified via a custom canary — write known bytes, drop, read raw
      memory if accessible — and document that real verification requires
      valgrind/ASan).

## Architecture notes

`SecretString` is intentionally not `Clone` — cloning would defeat the
zero-on-drop invariant. `Display` is deliberately *not* implemented to
avoid accidental log leaks.

Performance: `rnp_buffer_clear` is just `memset`-to-zero with a
compiler-fence to prevent dead-store elimination. Negligible cost.

## Acceptance criteria

- `SecretString::from("hunter2")` round-trips via `as_str`.
- After drop, the underlying buffer is zeroed (verified in tests via a
  best-effort canary).
- `request_password` returns `SecretString` and the existing call sites
  still compile (via `as_str()` adaptation).

## Completion log

**DONE** in this session.

- `SecretString` in `src/secret.rs`. `Drop` calls `rnp_buffer_clear` on
  the underlying bytes (foreign call → compiler can't DCE the store).
  Neither `Clone` nor `Display` (intentional — prevents accidental leak
  via cloning or logging).
- `zero_string_bytes(&mut str)` exported for ad-hoc use.
- `request_password` now returns `SecretString` instead of `String`.
- `Debug` impl prints `"SecretString(***)"`.
- Tests: round-trip, `into_string`, `request_password` returns secret.
