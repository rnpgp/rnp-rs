# rnp-rs

Idiomatic Rust binding to the [RNP](https://github.com/rnpgp/rnp) OpenPGP
(RFC 9580) C library.

RNP is the C++ OpenPGP implementation that powers Mozilla Thunderbird.
This crate provides a thin, safe Rust wrapper over its public C FFI
(declared in `include/rnp/rnp.h`).

## Status

The crate wraps ~250 of librnp's ~309 public functions across all major
OpenPGP concerns: signing, verification, encryption, decryption, key
generation, keyring management, ASCII armor, packet dumps, security
profile, and feature queries. PQC and crypto-refresh (RFC 9580 v6)
support is feature-gated.

## Features

- **Sign/verify** — inline, detached, and cleartext signatures.
- **Encrypt/decrypt** — public-key + password-based, AEAD, ASCII armor,
  rich verify-result inspection (recipients, symenc, protection info).
- **Key management** — full getter surface, UID enumeration, subkey
  handles, signature handles with subpacket inspection, key generation
  via builder, keyring save/load/import, homedir discovery.
- **Key mutation** — protect/unprotect/lock/unlock, add UID, revoke,
  set expiration, export revocation cert, three signature-creation
  builders sharing a setter trait.
- **ASCII armor** — armor/dearmor with explicit types, packet dumps
  (text + JSON), per-object JSON serialization.
- **Security profile** — add/get/remove security rules with typed
  levels and flags, feature queries (`supports_feature`,
  `supported_features`).
- **PQC** — `ML-KEM`, `ML-DSA`, `SLH-DSA` composite algorithms,
  `prefer_pqc_enc_subkey`, v6 PKESK/SKESK.
- **Vendored build** — `--features vendored` builds librnp from a git
  submodule via CMake, statically links the result.

## Cargo features

| Feature           | Default | Description                                                                                                                                                                                           |
|-------------------|---------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `vendored`        | off     | Build librnp from `vendor/rnp/` via CMake and statically link the resulting `librnp.a`. See [vendor/README.md](vendor/README.md).                                                                     |
| `pqc`             | off     | Pass `-DRNP_EXPERIMENTAL_PQC` to bindgen so PQC algorithm constants and `rnp_op_encrypt_prefer_pqc_enc_subkey` are exposed. Requires the linked librnp to have been built with `ENABLE_PQC=ON`.       |
| `crypto-refresh`  | off     | Pass `-DRNP_EXPERIMENTAL_CRYPTO_REFRESH` to bindgen so v6 keys, crypto-refresh algorithm names, and `enable_pkesk_v6` / `enable_skesk_v6` are exposed. Requires the linked librnp built with `ENABLE_CRYPTO_REFRESH=ON`. |
| `logging`         | off     | Gate `Context::set_log_fd` / `set_log_file` for directing librnp's diagnostic output to a file.                                                                                                       |

## Quick start

```rust
use rnp::{Algorithm, Context, Hash, KeyBuilder, KeyUsage};

let ctx = Context::new()?;
let key = KeyBuilder::new(Algorithm::Rsa)
    .bits(2048)
    .userid("alice <alice@example.com>")
    .hash(Hash::Sha256)
    .add_usage(KeyUsage::Sign)
    .add_usage(KeyUsage::Certify)
    .build(&ctx)?;

let message = b"hello, world";
let signed = rnp::sign(&ctx, message, &key)?;
assert!(rnp::verify(&ctx, &signed)?);
# Ok::<(), rnp::Error>(())
```

See `examples/` for end-to-end demos of sign/verify, encrypt/decrypt,
and key generation.

## Linking

`rnp-rs` links against a **system-installed** `librnp` (`-lrnp`) by
default.

### macOS (Homebrew)

```sh
brew install rnp
```

### Linux

```sh
# Fedora
sudo dnf install librnp-devel

# Debian/Ubuntu (if packaged)
sudo apt install librnp-dev
```

### Pointing at a source checkout

```sh
RNP_INCLUDE_DIR=/path/to/rnp/install/include \
RNP_LIB_DIR=/path/to/rnp/install/lib \
cargo build
```

### Vendored

```sh
git submodule update --init --recursive   # initialize vendor/rnp
cargo build --features vendored
```

Requires Botan, JSON-C, and zlib to be installed system-wide.

## License

BSD-2-Clause, matching the upstream RNP project. See `LICENSE.md`.
