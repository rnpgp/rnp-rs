# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

**DO NOT** open a public issue for security vulnerabilities.

Instead, please report security issues to: <security@ribose.com>

### What to Include

1. Description of the vulnerability.
2. Steps to reproduce.
3. Potential impact.
4. Affected version(s) (run `rnp::version_string()`).
5. Your name/handle (optional, for acknowledgment).

### Response Timeline

- **Initial response**: within 48 hours.
- **Status update**: within 7 days.
- **Resolution target**: within 30 days (critical), 90 days (others).

## Security Considerations

### FFI boundary

This crate wraps librnp (a C++ OpenPGP implementation) via Rust unsafe
FFI bindings. The safe wrappers in `src/*.rs` are designed so callers
never need to write `unsafe` code. Bugs in the unsafe blocks are
treated as security issues.

If you find a way to trigger a panic, out-of-bounds read, or
use-after-free through the safe API, please report it privately.

### Password hygiene

[`SecretString`](src/secret.rs) zeroises its underlying bytes on drop
via `rnp_buffer_clear` (a foreign call the compiler can't optimise
away). Use it for any short-lived secret material.

The `request_password` helper returns `SecretString` by default.
Callers who need a plain `String` can use `SecretString::into_string`
(but should prefer keeping the secret scoped).

### PQC and crypto-refresh feature gates

`--features pqc` and `--features crypto-refresh` enable experimental
RFC 9580 features that require librnp to be built with the matching
CMake options. The runtime probe
[`librnp_supports_pqc()`](src/keygen.rs) confirms the linked librnp
actually supports PQC at runtime; calling PQC APIs without that
confirmation fails opaquely.

### Crypto backend

librnp uses Botan or OpenSSL as its crypto backend. Reported CVEs in
those backends affect rnp-rs indirectly — see the upstream
[`SECURITY.md`](https://github.com/rnpgp/rnp/security/policy) for the
full backend chain.

## Known incidents

- **CVE-2025-13470** (librnp, fixed in 0.18.1): PKESK session keys were
  generated without cryptographically random values. rnp-rs 0.1.0
  links against a fixed librnp by default; older librnp installs are
  vulnerable. Mitigation: upgrade librnp to >= 0.18.1.
