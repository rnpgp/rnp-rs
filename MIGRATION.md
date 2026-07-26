# Migration Guide

Coming to rnp-rs from another OpenPGP library? This guide maps the
common operations.

## From sequoia-openpgp

| Operation | sequoia-openpgp | rnp-rs |
|---|---|---|
| Create context | `Cert::generate(...)` | `Context::new()` + `KeyBuilder::new(Algorithm::Rsa).build(&ctx)` |
| Sign message | `Signer::new(key, msg).sign()` | `rnp::sign(&ctx, msg, &key)` |
| Verify message | `Verifier::from_bytes(...)` | `rnp::verify(&ctx, msg)` |
| Encrypt | `Encryptor::for_recipients(...)` | `rnp::Encryptor::new(&ctx, msg).add_recipient(&key).build(&mut output)` |
| Decrypt | `Decryptor::from_bytes(...)` | `rnp::decrypt(&ctx, ciphertext)` |
| Load key | `Cert::from_bytes(...)` | `Context::load_keys(format, bytes, flags)` |
| Export key | `cert.armored()` | `key.export(ExportFlags::ARMORED)` |

### Key differences

- **Identity**: rnp-rs keys borrow a `Context`; sequoia certs are owned
  values. All rnp-rs operations go through the `Context`.
- **Algorithm enum**: `rnp::Algorithm::Rsa` vs sequoia's `PublicKeyAlgorithm::RSA`.
- **Error type**: `rnp::Error` with categorized `ErrorKind` vs sequoia's
  `anyhow::Error`.

## From the librnp C API

| C API | rnp-rs |
|---|---|
| `rnp_ffi_create(&ffi, "GPG", "GPG")` | `Context::new()` |
| `rnp_op_sign_create(...)` | `rnp::sign(ctx, msg, key)` |
| `rnp_op_verify_create(...)` | `rnp::verify(ctx, msg)` |
| `rnp_op_encrypt_create(...)` | `Encryptor::new(ctx, msg).build(&mut output)` |
| `rnp_decrypt(...)` | `rnp::decrypt(ctx, ciphertext)` |
| `rnp_key_export(...)` | `key.export(flags)` |
| `rnp_generate_key_rsa(...)` | `KeyBuilder::new(Rsa).bits(2048).build(ctx)` |

### Key differences

- **No manual cleanup**: every handle is RAII. No `rnp_*_destroy` calls
  needed.
- **Typed errors**: `rnp_result_t` → `Error` with `ErrorKind`. Use `?`
  instead of checking return codes.
- **Builder pattern**: op-config via chained method calls, not a
  flat struct of setter functions.

## From gpgme / rust-gpgme

| gpgme | rnp-rs |
|---|---|
| `Context::new()` | `Context::new()` |
| `ctx.sign(SignerMode::Detached, ...)` | `rnp::sign_detached(ctx, msg, key)` |
| `ctx.verify(...)` | `rnp::verify(ctx, msg)` |
| `ctx.encrypt(...)` | `Encryptor::new(ctx, msg).add_recipient(key).build(...)` |
| `ctx.decrypt(...)` | `rnp::decrypt(ctx, ciphertext)` |
| `Key::export(...)` | `key.export(flags)` |

### Key differences

- **No GPGME protocol negotiation**: rnp-rs always uses the RNP
  OpenPGP backend (librnp). No `Protocol::OpenPgp` enum.
- **No gpg-agent dependency**: password prompts go through the
  `PasswordProvider` trait, not gpg-agent's pinentry.
- **Thread safety**: rnp-rs handles are not `Send`/`Sync`. Use a
  `Mutex<Context>` for multi-threaded access.
