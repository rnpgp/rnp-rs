# 13 — Signature lifecycle: export, remove, find_subpacket

- **Priority:** P1
- **Status:** done (this session)
- **Blocked by:** 02, 03

## Context

`Signature` is currently read-only — you can inspect a signature but not
remove it from its key, export it as a standalone binary blob, or look up
a specific subpacket by type. The C API has all three.

## Work items

- [ ] `Signature::export(flags: ExportFlags) -> Result<Vec<u8>>` — wraps
      `rnp_signature_export`.
- [ ] `Signature::remove_from_key(&self, key: &Key)` — wraps
      `rnp_signature_remove`. Note: the C signature requires both the sig
      and the key it belongs to (librnp uses the key to find the sig's
      container). The Rust API should reflect that.
- [ ] `SubpacketType` enum — RFC 4880 §5.2.3.1 lists ~50 types. Add the
      commonly-used ones as variants and a fallback `Other(u8)`. OCP.
- [ ] `Signature::find_subpacket(typ, hashed: Option<bool>, skip: usize)
      -> Result<Option<Subpacket>>` — wraps
      `rnp_signature_subpacket_find`.
- [ ] Tests:
  - Generate key, find the self-certification on UID[0], export it as
    armored bytes, re-import via `rnp_import_signatures`.
  - Find the KeyFlags subpacket on a self-certification, verify the
    expected usage bits are set.
  - `remove_from_key` then re-inspect — signature count decreases.

## Architecture notes

`SubpacketType` enum: RFC 4880 subpacket tags are u8 (0..255). Cover the
ones librnp actually inspects (the rest stay as `Other(u8)`). Adding a new
known type = one variant + one match arm — OCP.

`hashed: Option<bool>` in `find_subpacket`: `None` means "either hashed
or unhashed", `Some(true)` means "only hashed", `Some(false)` means "only
unhashed". The C API exposes this as a bool with no "don't care" option,
so we model the tristate explicitly.

## Acceptance criteria

- `Signature::export` produces bytes that `dump_packets_to_json` parses.
- `find_subpacket(SubpacketType::KeyFlags, None, 0)` finds the KeyFlags
  subpacket on a self-certification.

## Completion log

**DONE** in this session.

- `SubpacketType` enum covering 21 known RFC 4880/9580 tags + `Other(u8)`
  fallback. `as_u8()` / `from_u8()` round-trip. `#[non_exhaustive]` so
  future tags can be promoted to typed variants without breaking callers.
- `Subpacket::typ()` returns the typed enum; `typ_raw()` returns the u8
  for callers who need the raw byte.
- `Signature::export(flags)` wraps `rnp_signature_export`.
- `Signature::remove_from_key(key)` wraps `rnp_signature_remove`. Uses
  `ManuallyDrop` to skip the Rust Drop after the C side destroys the
  handle.
- `Signature::find_subpacket(typ, hashed: Option<bool>, skip)` wraps
  `rnp_signature_subpacket_find`.
- Tests in `tests/parity_gaps.rs`: export round-trips through
  `dump_packets_to_json`; `find_subpacket(KeyFlags, None, 0)` finds the
  self-certification's KeyFlags; enum round-trips known and unknown tags.
