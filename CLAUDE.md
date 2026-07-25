# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`rnp` (the crate) / `rnp-rs` (the repo) is an idiomatic, safe Rust binding over
the public C FFI of [RNP](https://github.com/rnpgp/rnp), the C++ OpenPGP
(RFC 9580) implementation used by Mozilla Thunderbird. RNP's C API is declared
in [`include/rnp/rnp.h`](https://github.com/rnpgp/rnp/blob/main/include/rnp/rnp.h)
(~4100 lines, ~309 public `rnp_*` functions).

The crate is in **early development**. The first milestone was **sign + verify**
(to support [Confium](https://github.com/confium/confium) plugin artifact
verification). The road to full parity with librnp is tracked as the Roadmap
section below — every TODO in the codebase maps to one of those milestones.

The upstream C++ source lives at `../rnp/` (a sibling checkout). Treat it as the
authoritative spec for behavior; read it whenever a wrapper's semantics are
unclear.

## Build, run, test

The crate links against a **system-installed** `librnp` (`-lrnp`).

```sh
cargo build                 # generates FFI bindings via bindgen, links -lrnp
cargo test                  # runs the integration tests in tests/
cargo test --test sign_verify                # one test binary by name
cargo test inline_sign_verify_roundtrip      # one test by name
cargo build --features vendored              # currently a no-op stub (see Roadmap)
```

### Dependencies

- **Rust:** edition 2024 (Rust ≥ 1.85). Even `unsafe fn` bodies need their own
  `unsafe { ... }` block — see `password_thunk` and `verify_op_status` for the
  pattern used in this crate.
- **librnp:** install via `brew install rnp` (macOS), `dnf install librnp-devel`
  (Fedora), or build from a `../rnp/` checkout.
- **libclang:** required by `bindgen`. On macOS comes with the Xcode
  command-line tools.

### Pointing at a non-system librnp

```sh
RNP_INCLUDE_DIR=/path/to/rnp/install/include \
RNP_LIB_DIR=/path/to/rnp/install/lib \
cargo build
```

`build.rs` (`build.rs:21-43`) searches `RNP_INCLUDE_DIR`, then Homebrew's
`/opt/homebrew/include` and `/usr/local/include`, then `/usr/include`. The lib
dir is taken from `RNP_LIB_DIR` or assumed to be a sibling of the include dir.

### Runtime linking on macOS

`.cargo/config.toml` adds `-Wl,-rpath,/opt/homebrew/lib,-rpath,/usr/local/lib`
to test/run binaries so they find `librnp.0.dylib` at runtime. Downstream
consumers of the published crate must arrange their own runtime linking.

### Doctests are disabled

`Cargo.toml` sets `doctest = false`. The bindgen output contains the upstream C
doc comments verbatim, including fenced C/JSON code blocks — the doctest harness
would try to compile those as Rust.

## Architecture

Three layers, top to bottom:

1. **`src/ffi.rs`** — `include!`s the bindgen-generated `$OUT_DIR/bindings.rs`.
   The module is `#![allow(non_camel_case_types, non_snake_case, dead_code)]` —
   the FFI module mirrors the C API verbatim and we don't fight the linter over
   it. Never `use` from `ffi::` in the public API; keep it crate-private.

2. **`src/error.rs`** — `Error` (snafu), `Result<T>`, and `check(rnp_result_t)`.
   `check` maps non-zero codes to `Error::Rnp { code, message }` via
   `rnp_result_to_string`. bindgen does **not** pick up the constants in
   `include/rnp/rnp_err.h` (they live in an anonymous C enum), so the few that
   matter are duplicated as plain `u32` consts in `error::codes`. Add new ones
   there with a citation to the upstream header.

3. **Safe wrappers** — one module per OpenPGP concern:
   - `src/context.rs` — `Context` (`rnp_ffi_t`), `PasswordProvider` trait,
     `password_thunk` C callback, `KeyringFormat`.
   - `src/key.rs` — `Key<'ctx>`, `KeyIdentifier`, `find_key`, `load_keys`,
     key getter surface (`alg`, `bits`, `curve`, `keyid`, `fingerprint`,
     `grip`, `is_*`, etc.), UID/Subkey enumeration.
   - `src/uid.rs` — `Uid<'key>` (borrows parent key).
   - `src/subkey.rs` — `Subkey<'key>` as a newtype over `Key` (DRY: a
     subkey *is* a key).
   - `src/keygen.rs` — `KeyBuilder`, `SubkeyBuilder`, `Algorithm`/`Curve`/
     `Hash`/`Cipher`/`Compression`/`KeyUsage` enums, `generate_key_json`.
   - `src/signature.rs` — `sign`, `sign_detached`, `verify`,
     `verify_detached`.
   - `src/encrypt.rs` — `Encryptor` builder, `decrypt`, `decrypt_to`.
   - `src/armor.rs` — `enarmor`, `dearmor`, `guess_contents`.
   - `src/dump.rs` — packet dumps (text + JSON), per-object JSON flags.

### Patterns to follow

- **Lifetime discipline.** Every derived handle borrows the `Context` (or its
  parent handle) for at most its lifetime, enforced with `PhantomData`. See
  `Key<'ctx>` at `src/key.rs:15-18`. When adding new handle types (`Uid`,
  `Signature`, `Subkey`, `Recipient`, `Symenc`, …), follow the same shape:
  `pub(crate) fn from_handle(...) -> Self` + private `handle` field + `Drop`
  that calls the matching `rnp_*_handle_destroy`.
- **Drop chain.** `Context::drop` (`src/context.rs:93-106`) takes the password
  provider first, then calls `rnp_ffi_destroy`. Each derived `Drop` destroys its
  own handle. When you wrap an op-builder (`rnp_op_sign_*`, `rnp_op_encrypt_*`,
  `rnp_op_generate_*`, `rnp_op_verify_*`), `Drop` must call both the op destroy
  and any input/output destroys.
- **Error-path cleanup.** If a `*_create` call leaves you owning an
  `rnp_input_t` / `rnp_output_t` and a subsequent step fails, destroy them
  before returning. See `cleanup_sign` at `src/signature.rs:224-236`.
- **Memory-output drain.** `rnp_output_to_memory(&mut out, 0)` → run the op →
  `rnp_output_memory_get_buf(out, &mut buf, &mut len, do_copy=true)` →
  `rnp_buffer_destroy(buf)` → `rnp_output_destroy(out)`. The logic is currently
  duplicated between `src/key.rs:172-194` and `src/signature.rs:203-222`; factor
  it into a shared helper when adding encryption.
- **C strings.** Userids, keyids, fingerprints can contain NUL bytes. Convert
  via `CString::new(...).map_err(|_| Error::NulByte)?` — see
  `src/key.rs:55-56`.
- **No `Send` / `Sync`.** librnp handles are not safe to move between threads.
  The crate relies on the auto negative impl that raw pointers carry. Do **not**
  add `unsafe impl Send` for any handle-bearing type.
- **Comment style.** Module-level `//!` docs describe what the module covers.
  Inline comments explain the *why* when non-obvious (lifetime tricks, error
  code mapping, cleanup ordering). Do not narrate the *what*.

### Test layout

Integration tests live in `tests/`, one file per concern:
`sign_verify`, `keygen`, `key_inspect`, `encrypt`, `armor_dump`, `io`,
`error_kind`. Unit tests are not yet used; prefer integration tests until
the wrapper surface grows. Test keys are generated in-process via
`KeyBuilder` (or the deprecated `generate_test_key` shim) — no key material
on disk.

## Roadmap to feature parity

The C library exposes ~309 public functions; this binding wraps ~250 of them
across all 10 phases. Each phase has its own file under `TODO.roadmap/`
with priority, status, work items, architecture notes, and completion log.

Status snapshot (full detail in the per-phase TODO files):

- **Phase 01 — Foundations.** Done. `Input`/`Output` RAII, `ErrorKind`
  categorized errors, shared `cstr_to_string` helper.
- **Phase 02 — Key inspection.** Done. Scalar getters + `Uid` + `Subkey`
  + `Signature` + `Subpacket` + `IdentifierIterator`.
- **Phase 03 — Key mutation.** Done. `protect`/`unprotect`/`lock`/`unlock`/
  `add_uid`/`revoke`/`set_expiration`/`remove`/`remove_signatures`/
  `export_{revocation,autocrypt}` + 3 signature-creation builders sharing
  the `SignatureSetterOps` trait.
- **Phase 04 — Key generation.** Done. `KeyBuilder` + `SubkeyBuilder` +
  algorithm enums + JSON API. v6 keys gated behind `crypto-refresh` feature.
- **Phase 05 — Keyring management.** Done. `save_keys`/`unload_keys`/
  `import_keys`/`import_signatures`/homedir/counts/`IdentifierIterator`.
- **Phase 06 — Encryption/decryption.** Done. `Encryptor` + `decrypt`
  + rich `VerifyOp`/`VerifyResult`/`VerifySignature`/`Recipient`/`Symenc`.
- **Phase 07 — Armor & packet tools.** Done. `enarmor`/`dearmor`/packet
  dumps + per-object JSON.
- **Phase 08 — Security profile & misc.** Done. Security rules, feature
  queries, version helpers, `calculate_iterations`, `request_password`.
- **Phase 09 — PQC.** Done (feature-gated). `pqc` + `crypto-refresh`
  Cargo features gate the PQC algorithm enum, `Encryptor::prefer_pqc_enc_
  subkey`, and v6 PKESK/SKESK. Runtime probe via `librnp_supports_pqc()`.
- **Phase 10 — Vendored build.** Scaffolded. `vendored` Cargo feature
  exists; cmake invocation is a stub that falls through to system link.
  Real submodule + cmake wiring is a focused follow-up PR.

Each phase should land as its own focused PR (or set of PRs) with
round-trip integration tests modeled on the existing files in `tests/`.

### Phase 1 — Foundations (do first)
**DONE** — see `TODO.roadmap/01-foundations.md`.

### Phase 2 — Key inspection & mutation
- Full `rnp_key_get_*` surface: `alg`, `bits`, `curve`, `creation`,
  `expiration`, `fprint`, `grip`, `keyid`, `primary_fprint`, `primary_grip`,
  `version`, `dsa_qbits`, `protection_{mode,type,cipher,hash,iterations}`,
  `have_public`, `have_secret`, `is_{locked,protected,primary,sub,valid,
  revoked,compromised,expired,superseded,retired}`, `valid_till{,64}`.
- UID handle: `get_uid_count`, `get_uid_at`, `get_uid_handle_at`,
  `uid_get_{data,type}`, `uid_is_{primary,valid,revoked}`, `uid_remove`.
- Subkey handle: `get_subkey_count`, `get_subkey_at`.
- Signature handle + subpackets: `get_signature_count`, `get_signature_at`,
  `signature_get_*`, `signature_subpacket_{count,at,find,info,data,destroy}`,
  `signature_is_valid`, `signature_remove`, `signature_export`,
  `signature_error_{count,at}`.
- Mutation: `key_add_uid`, `key_revoke`, `key_remove`, `key_set_expiration`,
  `key_protect`, `key_unprotect`, `key_lock`, `key_unlock`,
  `key_remove_signatures`, `key_export_revocation`, `key_export_autocrypt`,
  `key_25519_bits_tweak{,ed}`.
- Builders for certification / direct signature / revocation signature
  (`rnp_key_certification_create`, `rnp_key_direct_signature_create`,
  `rnp_key_revocation_signature_create`) with the full
  `rnp_key_signature_*` setter surface.

### Phase 3 — Key generation
- Replace `generate_test_key` with a `KeyBuilder` over `rnp_op_generate_*`,
  covering RSA, DSA/ElGamal, ECDSA/ECDH, EdDSA/X25519, SM2, plus the generic
  `rnp_generate_key_ex`. Subkey builder via
  `rnp_op_generate_subkey_create`. v6 keys via `set_v6_key`.
- All `rnp_op_generate_set_protection_*`, `add_usage`, `clear_usage`,
  `add_pref_{cipher,compression,hash}`, `clear_pref_*`, `set_pref_keyserver`,
  `set_userid`, `set_expiration`, `set_{bits,hash,dsa_qbits,curve}`.
- JSON API: `rnp_generate_key_json` returning the result-fingerprint JSON.

### Phase 4 — Keyring management
- `save_keys`, `unload_keys`, `get_public_key_count`, `get_secret_key_count`.
- `import_keys` (returns status JSON), `import_signatures`.
- `get_default_homedir`, `detect_homedir_info`, `detect_key_format`.
- `IdentifierIterator` over `rnp_identifier_iterator_create/next/destroy`.

### Phase 5 — Encryption / decryption
- `Encryptor` builder wrapping `rnp_op_encrypt_*`: `add_recipient`,
  `add_password`, `set_{armor,cipher,hash,compression,aead,aead_bits,file_name,
  file_mtime,creation_time,expiration_time}`, `enable_pkesk_v6`,
  `enable_skesk_v6`, `set_flags` (NOWRAP), `prefer_pqc_enc_subkey` (gated).
- `decrypt` (simple path via `rnp_decrypt`), plus rich result inspection via the
  shared verify-op surface: `get_recipient_{count,at}`, `get_used_recipient`,
  `get_symenc_{count,at}`, `get_used_symenc`, `get_protection_info`,
  `get_file_info`, `get_format`, `Recipient` and `Symenc` handle wrappers.

### Phase 6 — Armor & packet tools
- `enarmor`/`dearmor`, `output_to_armor`, `armor_set_line_length`.
- `dump_packets_to_{output,json}`, `key_packets_to_json`,
  `signature_packet_to_json`, `key_to_json` (with the `RNP_JSON_*` flag
  bitmask). `guess_contents`.

### Phase 7 — Security profile & misc
- `add_security_rule`, `get_security_rule`, `remove_security_rule` over the
  `RNP_SECURITY_*` level/flag constants and the `RNP_FEATURE_*` strings.
- `supported_features`, `supports_feature`.
- `calculate_iterations`, `set_timestamp`, `request_password`.
- Wire `rnp_ffi_set_key_provider` (only pass-provider is wired today).
- Version helpers (`rnp_version_{major,minor,patch,string,string_full,
  commit_timestamp,for}`, `rnp_backend_{string,version}`).

### Phase 8 — PQC (experimental)
Gated on the C library being built with `ENABLE_PQC=ON`, which defines
`RNP_EXPERIMENTAL_PQC`. The header only declares PQC constants and
`rnp_op_encrypt_prefer_pqc_enc_subkey` under that macro.

- Add a `pqc` Cargo feature that:
  - passes `-DRNP_EXPERIMENTAL_PQC` to bindgen in `build.rs`,
  - panics at build time if the installed `librnp` was not built with PQC
    (probe via `rnp_supports_feature(... RNP_FEATURE_PK_ALG ...)`.
- Expose the composite algorithm names (`ML-KEM-*`, `ML-DSA-*`, `SLH-DSA-*`)
  through `KeyBuilder::algorithm` / `Encryptor` once supported.
- Round-trip tests for ML-KEM encryption and ML-DSA / SLH-DSA signatures.

### Phase 9 — Vendored build
- `vendored` Cargo feature: pull in `rnp` C++ as a pinned git submodule under
  `rnp-rs/vendor/rnp/` (or similar). Build it from `build.rs` via the `cmake`
  crate, statically link `librnp.a`. Default to the Botan backend; document how
  to opt into OpenSSL. Initialize `src/libsexpp` submodule on the user's behalf.
- This is what unblocks downstream consumers who don't have `librnp` packaged —
  currently the binding assumes a system install.

### Phase 10 — Polish
- Replace the duplicated `drain_memory_output` with a shared helper.
- Consider `Error::from` impls for the stdlib `io::Error` boundary so callers
  using `?` in IO paths don't have to map manually.
- Add fuzz harness mirroring `src/fuzzing/` upstream once the surface is wide
  enough to be worth fuzzing.

## Working with the upstream C source

When in doubt about a wrapper's behavior, read the corresponding implementation
under `../rnp/src/lib/rnp.cpp` (FFI layer) or `../rnp/src/librepgp/`
(packet layer). The Doxygen comments in `../rnp/include/rnp/rnp.h` are the
authoritative signatures. Examples in `../rnp/src/examples/` (`generate.c`,
`encrypt.c`, `decrypt.c`, `sign.c`, `verify.c`, `dump.c`) are the canonical
"how to use this API" references — mirror their structure when designing a new
wrapper.

## Conventions inherited from upstream

- The crate is BSD-2-Clause, matching RNP.
- Public Rust identifiers are idiomatic (`Key::primary_uid`, not
  `key_get_primary_uid`). The raw C names live only in `ffi::`.
- Flag types (`ExportFlags`, `LoadSaveFlags`, …) are thin `pub struct Foo(pub
  u32)` wrappers around the `RNP_*` bit constants with `const` associated
  values and a `BitOr` impl. Follow that pattern for new flag sets
  (`JsonDumpFlags`, `SecurityFlags`, `KeyRemoveFlags`, `VerifyFlags`, …).
