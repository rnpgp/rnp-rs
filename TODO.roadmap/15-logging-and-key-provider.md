# 15 — Logging and key-provider callbacks

- **Priority:** P2
- **Status:** done (this session)
- **Blocked by:** 08

## Context

Two callbacks on `Context` are still unwrapped:

- `rnp_ffi_set_log_fd` — direct librnp diagnostic output to a Unix fd.
- `rnp_ffi_set_key_provider` — invoked when librnp needs a key that isn't
  in the keyring (typically during verify/decrypt). The callback is
  expected to call `Context::load_keys` to fetch it.

The existing `PasswordProvider` trait + C thunk pattern in
`src/context.rs` is the model for both.

## Work items

- [ ] `logging` Cargo feature (off by default — pulls in nothing but
      gates the API).
- [ ] `Context::set_log_fd(fd: i32) -> Result<()>` behind `#[cfg(feature
      = "logging")]`. Wraps `rnp_ffi_set_log_fd`.
- [ ] Convenience: `Context::set_log_file(path) -> Result<()>` that opens
      the file and passes the fd.
- [ ] `KeyProvider` trait mirroring `PasswordProvider`:
      ```rust
      pub trait KeyProvider: Send + Sync {
          fn on_key_request(&self, ctx: &Context, id: KeyIdentifier<'_>,
                            requested: RequestedKeyType)
              -> KeyRequestResult;
      }
      ```
- [ ] `RequestedKeyType` enum (`Keyid`, `Fingerprint`, ...) and
      `KeyRequestResult` enum (`Found`, `NotFound`, `Error`).
- [ ] `Context::set_key_provider(Box<dyn KeyProvider>)` — wires the C
      callback via a thunk. Same pattern as `set_password_provider`.
- [ ] Tests: verify a signature whose key is initially absent, with a
      `KeyProvider` that loads it from a serialized keyring on demand.

## Architecture notes

The `KeyProvider` callback runs *inside* a librnp call, so it receives a
`&Context` to call `load_keys` on. That's re-entrant — the same `Context`
is on the C stack. The thunk must not hold a `&mut Context`; it should
hold a `*const Context` (raw pointer, safe because the C side guarantees
the ffi is alive during the call).

For Send/Sync: `KeyProvider` requires `Send + Sync` (same as
`PasswordProvider`). The boxed trait object goes through
`Box::into_raw`/`Box::from_raw` exactly like the password provider.

## Acceptance criteria

- `set_log_file("/tmp/rnp.log")` produces a log file with librnp diagnostic
  messages after a key generation.
- A `KeyProvider` that loads keys from a byte vector satisfies a verify
  whose signer isn't in the initial keyring.

## Completion log

**DONE** in this session.

- `logging` Cargo feature in `Cargo.toml` (off by default).
- `Context::set_log_fd(fd)` and `Context::set_log_file(path)` behind
  `#[cfg(feature = "logging")]`. `set_log_file` opens via libc `open()`
  to avoid pulling an extra crate.
- `KeyProvider` trait mirroring `PasswordProvider`. Returns
  `KeyRequestOutcome::Found` or `NotFound`.
- `RequestedKeyType` enum (`Keyid`, `Fingerprint`, `Grip`).
- `Context::set_key_provider(Box<dyn KeyProvider>)` — thunk pattern
  matches `set_password_provider`.
- `Context::borrow_ffi` + `borrowed: bool` flag — borrowed contexts skip
  `rnp_ffi_destroy` on drop. Critical for callback re-entrancy (otherwise
  the borrowed Context would destroy the ffi mid-call).
- Tests in `tests/parity_gaps.rs`: key-provider callback loads a public
  key into the verify context and the signature verifies.
