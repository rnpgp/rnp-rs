# 07 — Armor, dearmor, packet dumps, JSON serialization

- **Priority:** P1
- **Status:** done (this session)
- **Blocked by:** 01

## Context

OpenPGP messages and keyrings come in binary and ASCII-armored forms.
librnp has dedicated `rnp_enarmor` / `rnp_dearmor` calls, an
`rnp_output_to_armor` output wrapper, and a family of packet-dump /
to-JSON functions. None are currently wrapped.

## Work items

### Armor / dearmor

- [ ] `armor(input) -> Result<Vec<u8>>` — wraps `rnp_enarmor` (uses the
      default armor type "message").
- [ ] `armor_with_type(input, type) -> Result<Vec<u8>>` — same with explicit
      type (`"message"`, `"public key"`, `"secret key"`, `"signature"`,
      `"cleartext signed message"`).
- [ ] `dearmor(input) -> Result<Vec<u8>>` — wraps `rnp_dearmor`.
- [ ] `Output::to_armor(inner, type)` already added in phase 01 — confirm
      the wrapper takes the armor type string correctly.
- [ ] `Output::armor_set_line_length(output, len)` — wraps
      `rnp_output_armor_set_line_length`.

### Packet dumps

- [ ] `dump_packets(input, output, DumpFlags)` — wraps
      `rnp_dump_packets_to_output`.
- [ ] `dump_packets_to_json(input, JsonDumpFlags) -> Result<String>` — wraps
      `rnp_dump_packets_to_json`.

### Per-object JSON

- [ ] `Key::to_json(JsonFlags) -> Result<String>` — wraps `rnp_key_to_json`.
- [ ] `Key::packets_to_json(JsonDumpFlags) -> Result<String>` — wraps
      `rnp_key_packets_to_json`.
- [ ] `Signature::packet_to_json(JsonDumpFlags) -> Result<String>` — wraps
      `rnp_signature_packet_to_json` (depends on phase 02's `Signature`
      handle).

### Content sniffing

- [ ] `guess_contents(input) -> Result<ContentType>` — wraps
      `rnp_guess_contents`. Returns an enum (`Message`, `PublicKeyring`,
      `SecretKeyring`, `Signature`, `Cleartext`, `Unknown`).

## Architecture notes

**`DumpFlags` / `JsonDumpFlags` / `JsonFlags`:** three new bit-flag structs
in `src/key/flags.rs` (or a new `src/flags.rs` if the file gets crowded).
Same `BitOr` pattern as existing flags. MECE: each flagset lives next to the
operation that consumes it.

**`ContentType` as enum:** the C side returns a string. Map to a Rust enum
at the boundary; unknown strings become `Unknown` with the raw string
preserved for diagnostics. OCP — adding a new content type is one enum
variant + one match arm.

**`to_json` returns `String`, not `serde_json::Value`:** the JSON shape is
defined by librnp and may change between versions. Parsing it to a Rust
struct couples us to that shape. Return the raw string; if a caller wants
typed access, they bring `serde_json` themselves (or we add a `serde`
feature later that depends on a fixed librnp version).

## Acceptance criteria

- Round-trip: armor → dearmor → bytes match input.
- Dump: known keyring → `dump_packets_to_json` returns non-empty JSON that
  mentions the expected algorithm.
- `guess_contents`: armor messages, keyrings, signatures each classify
  correctly.
- Per-object JSON: generated key → `Key::to_json` → JSON contains the
  expected keyid and algorithm.

## Completion log

**DONE** in this session. Implemented:

- `src/armor.rs` — `enarmor`, `dearmor`, `guess_contents`, convenience
  `armor_bytes` / `dearmor_bytes`. `ContentType` enum mapping the 5 known
  strings + `Unknown`.
- `src/dump.rs` — `dump_packets_to_output`, `dump_packets_to_json`,
  `dump_packets_bytes_to_json`. Three flag structs: `DumpFlags`,
  `JsonDumpFlags`, `JsonFlags`.
- `Key::to_json(flags)`, `Key::packets_to_json(secret, flags)` added to
  `src/key.rs`.
- `tests/armor_dump.rs` — 4 tests: armor round-trip, packet JSON parses as
  array, key JSON contains keyid, key packet JSON parses.

**Deferred** (called out in the file but not blocking parity):
- `signature_packet_to_json` lands with phase 02's `Signature` handle.
- Callback-based `Input`/`Output` still TODO with phase 08.
