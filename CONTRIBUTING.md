# Contributing to rnp-rs

Thanks for your interest in contributing! This document covers the
basics. The full architectural guide lives in
[`CLAUDE.md`](./CLAUDE.md); the per-phase TODO files in
[`TODO.roadmap/`](./TODO.roadmap) are the audit trail for each feature
area.

## Project layout

```
src/                       Rust source
├── lib.rs                 crate root + re-exports
├── context.rs             Context (rnp_ffi_t wrapper), PasswordProvider
├── key.rs                 Key + read getters + mutation methods + flags
├── key_options.rs         ProtectOptions, AddUidOptions, RevocationReason
├── keygen.rs              KeyBuilder, SubkeyBuilder, Algorithm/Curve/Hash/Cipher enums
├── keyring.rs             save/load/import/homedir/counts/IdentifierIterator
├── key_signature_builder.rs  3 signature-creation builders + shared trait
├── signature.rs           sign, sign_detached, sign_cleartext, verify, verify_detached
├── signature_handle.rs    Signature + Subpacket + SubpacketType + SignatureType
├── encrypt.rs             Encryptor, decrypt, AddPasswordOptions
├── verify.rs              VerifyOp, VerifyResult, VerifySignature, Recipient, Symenc
├── armor.rs               enarmor, dearmor, guess_contents
├── dump.rs                packet dumps + JSON helpers
├── security.rs            SecurityRule, feature queries, calculate_iterations
├── version.rs             version helpers
├── callbacks.rs           KeyProvider + RequestedKeyType + KeyRequestOutcome
├── secret.rs              SecretString (zero-on-drop)
├── strconv.rs             FromStr / Display impls for model enums
├── ops/
│   ├── mod.rs
│   └── io.rs              Input, Output, call_for_string helpers
├── context.rs             Context, KeyringFormat, PasswordProvider
├── error.rs               Error, ErrorKind, from_rnp_code, unknown_variant
├── ffi.rs                 bindgen output (do not edit)
tests/                     Integration tests (one file per concern)
examples/                  Runnable example programs
TODO.roadmap/              Per-phase TODO files (40 phases)
vendor/rnp/                librnp C++ source (git submodule, --features vendored)
build.rs                   Link-mode resolution + bindgen invocation
wrapper.h                  bindgen input
```

## Coding style

- `cargo fmt` is canonical. Run before every commit.
- `cargo clippy --all-targets -- -D warnings` must pass.
- Comments: explain *why*, not *what*. Default to no comments.
- No AI attribution in commit messages, PR descriptions, or code
  comments (no `Co-authored-by:`, no `Generated with`).

## Test matrix

Three configurations must stay green:

1. **Default** — system-installed librnp.
   ```sh
   cargo test
   ```
2. **Vendored** — cmake-built librnp from `vendor/rnp/`.
   ```sh
   git submodule update --init --recursive
   cargo test --features vendored
   ```
3. **PQC + crypto-refresh** — requires a Botan 3.6 built with PQC
   modules (Xcode clang, not Homebrew LLVM — see
   `TODO.roadmap/09-pqc.md`) and a librnp HEAD built with
   `ENABLE_PQC=ON -DENABLE_CRYPTO_REFRESH=ON`. See
   `TODO.roadmap/09-pqc.md` for the exact recipe.

## Adding a new C function wrapper

Pick a function from `include/rnp/rnp.h` (or the bindgen output at
`$OUT_DIR/bindings.rs`). End-to-end recipe:

1. **Find the right module**. Use the layout above. (e.g. key getters
   go in `src/key.rs`.)
2. **Wrap the C call**. Use the helpers in `src/ops/io.rs`:
   ```rust
   pub fn my_getter(&self) -> Result<String> {
       crate::ops::call_for_string(|raw| unsafe {
           ffi::rnp_key_get_my_field(self.handle, raw)
       })
   }
   ```
   For optional getters, use `call_for_optional_string` which maps
   `RNP_ERROR_NOT_FOUND` → `None`.
3. **Document**. A `///` doc comment with what the field means, not
   what the FFI call does.
4. **Test**. Add a round-trip test in the relevant `tests/*.rs` file.
5. **Re-export**. Add to `lib.rs` if it's a new public type.
6. **Update CLAUDE.md / TODO** if it's part of a tracked phase.

## Adding a new feature flag

1. Declare in `Cargo.toml` `[features]`.
2. In `build.rs`, gate any bindgen `clang_arg`s on `cfg!(feature = ...)`.
3. Use `#[cfg(feature = "...")]` on the affected types / methods.
4. Add to the CI matrix if the feature has runtime requirements.
5. Document in `README.adoc` Cargo features table.

## Bumping the vendored rnp submodule

```sh
cd vendor/rnp
git fetch --tags
git checkout v0.X.Y                 # pick a release tag
git submodule update --init --recursive
cd ../..
git add vendor/rnp
git commit -m "vendor/rnp: bump to v0.X.Y"
cargo test --features vendored      # confirm green
```

## Filing issues

- Search existing issues first.
- Include: librnp version (`rnp::version_string()`), Rust version
  (`rustc --version`), Cargo features enabled, OS, and a minimal
  reproduction.

## Pull requests

- One concern per PR. Split refactors from feature additions.
- Branch from `main`. Target `main`.
- All conversations resolved before merge.
- Squash-merge with a 50/72 commit message.

## License

By contributing you agree that your contributions are licensed under the
BSD-2-Clause license (see `LICENSE.md`).
