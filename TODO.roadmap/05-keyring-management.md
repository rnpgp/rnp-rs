# 05 — Keyring management: save, unload, import, homedir

- **Priority:** P1
- **Status:** done (this session)
- **Blocked by:** 01, 02

## Context

Phase 01 has `Context::load_keys`. The other keyring-level operations — save,
unload, import (with status JSON), import signatures, homedir discovery — are
not yet wrapped.

## Work items

### Save / unload / counts

- [ ] `Context::save_keys(format, flags)` — wraps `rnp_save_keys`.
- [ ] `Context::unload_keys(UnloadFlags)` — wraps `rnp_unload_keys`.
- [ ] `Context::public_key_count()` / `secret_key_count()` — wraps
      `rnp_get_public_key_count` / `rnp_get_secret_key_count`.

### Import

- [ ] `Context::import_keys(bytes, flags) -> Result<ImportResult>` — wraps
      `rnp_import_keys`. Returns the JSON status (new keys, updated keys,
      unchanged keys). Consider parsing to a struct via `serde_json` (a new
      optional dependency, feature-gated) — or just return `String` and let
      callers parse.
- [ ] `Context::import_signatures(bytes) -> Result<String>` — wraps
      `rnp_import_signatures`. Same JSON-status decision.

### Homedir discovery

- [ ] `Context::default_homedir() -> Result<PathBuf>` — wraps
      `rnp_get_default_homedir`.
- [ ] `Context::detect_homedir_info(path)` — wraps
      `rnp_detect_homedir_info`, returns `(pub_format, sec_format)`.
- [ ] `Context::detect_key_format(bytes)` — wraps `rnp_detect_key_format`.

## Architecture notes

**Import return type:** `ImportResult` struct with `new`, `updated`,
`unchanged` fields parsed from the JSON. Lives in `src/keyring.rs`. If we add
`serde_json` as an optional dep, the parse lives behind a `json` feature
(P3 — low priority, defer until needed). For now, return `String`.

**`UnloadFlags`:** new bit-flag struct in `src/key/flags.rs` alongside the
existing `LoadSaveFlags` and `ExportFlags`. Re-export from `crate::*`. Same
`BitOr` pattern.

**Homedir as `PathBuf`, not `String`:** paths are paths. Use `PathBuf` and
let callers decide encoding.

## Acceptance criteria

- Round-trip: generate → save to temp file → unload → load from file →
  inspect → keys match.
- Import: import a known armored key → `public_key_count()` increments →
  re-import → status JSON reports "unchanged".
- Homedir: `default_homedir()` returns a path that exists on the test machine
  (or returns `NotFound` cleanly when `GNUPGHOME` is unset and no default
  exists).

## Completion log

**DONE** in this session.

- `Context::save_keys(format, flags, &mut Output)` and convenience
  `save_keys_to_memory(format, flags) -> Result<Vec<u8>>`.
- `Context::unload_keys(UnloadFlags)` with `UnloadFlags::{PUBLIC,SECRET}`.
- `Context::import_keys(bytes, flags) -> Result<String>` (returns the
  status JSON).
- `Context::import_signatures(bytes, flags) -> Result<String>`.
- `Context::public_key_count()` / `secret_key_count()`.
- `Context::default_homedir() -> Result<PathBuf>` (associated function —
  doesn't need a context).
- `Context::detect_homedir_info(path) -> Result<(pub_fmt, pub_path,
  sec_fmt, sec_path)>`.
- `Context::detect_key_format(bytes) -> Result<String>`.
- `Context::identifiers(IdentifierKind) -> Result<IdentifierIterator>` —
  the iterator yields `String`s lazily and destroys itself on drop.
- `tests/extended.rs::save_unload_reload_roundtrip` exercises
  save→unload→load and `identifier_iterator_yields_keys` covers iteration.

**Deferred:**

- `ImportResult` struct (typed view of the import status JSON) is not
  added — we return the raw JSON `String`. Adding `serde_json` as an
  optional dep remains a future `json` Cargo feature.
