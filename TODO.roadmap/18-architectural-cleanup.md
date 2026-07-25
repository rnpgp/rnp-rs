# 18 — Architectural cleanup: DRY the c_char out-param pattern

- **Priority:** P2 — code cleanliness
- **Status:** partial (this session): helper added + used in new code;
      existing call sites remain on the legacy pattern pending a focused
      refactor pass.
- **Blocked by:** 01

## Context

The pattern

```rust
let mut raw: *mut c_char = ptr::null_mut();
unsafe {
    check(ffi::rnp_X_get_Y(handle, &mut raw))?;
    cstr_to_string(raw)
}
```

appears ~25 times across `src/key.rs`, `src/signature_handle.rs`,
`src/verify.rs`, `src/keyring.rs`, `src/security.rs`. Each instance is
5-7 lines of boilerplate. The pattern is identical modulo the FFI call.

## Work items

- [ ] Add `ops::io::call_for_string<F: FnMut(*mut *mut c_char) -> u32>
      (F) -> Result<String>` — runs the closure, checks the result, frees
      the buffer via `rnp_buffer_destroy`.
- [ ] Add `ops::io::call_for_optional_string<F>(F) -> Result<Option
      <String>>` — same but treats `RNP_ERROR_NOT_FOUND` as `None`.
- [ ] Audit and rewrite every repetition. Confirm no behavior change
      (each call still destroys exactly one buffer).
- [ ] Confirm via `grep -rn 'rnp_buffer_destroy' src/` that all
      destroy calls remain inside `ops/io.rs` — the DRY refactor must not
      leak the buffer-freeing responsibility to other modules.
- [ ] Tests: existing suite stays green; no new tests needed (this is
      pure refactor).

## Architecture notes

The helper takes a closure `FnMut(*mut *mut c_char) -> u32` because each
FFI function has a different name but the same out-param shape. The
closure captures the handle and any indexes.

`call_for_optional_string` exists because many C getters return
`RNP_ERROR_NOT_FOUND` for "no value" rather than returning null with
success — wrapping the same logic in two helpers keeps each call site
to one line.

## Acceptance criteria

- All 25+ sites converted. `cargo grep '\*mut c_char' src/` should only
  return matches inside `ops/io.rs`.
- `cargo test` green across all three configurations (default, vendored,
  pqc+crypto-refresh).
- `cargo clippy --all-targets -- -D warnings` clean.

## Completion log

**PARTIAL** in this session.

- `ops::io::call_for_string<F>(F) -> Result<String>` and
  `call_for_optional_string<F>(F) -> Result<Option<String>>` added. Take
  a closure that writes into `*mut *mut c_char` and returns `u32`.
- Re-exported from `lib.rs` so all modules can use them.
- Used in new code added in phases 11-17 (e.g. `Signature::packet_to_json`,
  `VerifySignature::hash`, etc.).

**Remaining work** (deferred to a focused refactor PR):

The ~25 legacy call sites in `key.rs`, `signature_handle.rs`, `verify.rs`,
`keyring.rs`, `security.rs` still use the explicit
`let mut raw: *mut c_char = ptr::null_mut(); check(ffi::X(handle, &mut raw))?;
cstr_to_string(raw)` triplet. Each is 5-7 lines that becomes 1-2 with the
helper. The refactor is mechanical (replace each triplet with a closure
call) but touches many sites — better done as its own atomic commit so a
regression is easy to bisect.
