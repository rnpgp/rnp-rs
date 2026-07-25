# 01 — Foundations: shared I/O, categorized errors, op-builder base

- **Priority:** P0 — every other phase depends on this
- **Status:** done (this session)
- **Blocks:** 02, 03, 04, 05, 06, 07, 08, 09

## Context

The current code has three structural problems that compound as the surface
grows:

1. **Duplicated memory-output drain.** The `unsafe fn drain_memory_output`
   appears verbatim in `src/key.rs:172-194` and `src/signature.rs:203-222`.
   Adding encryption would add a third copy.
2. **Ad-hoc input/output lifetime management.** Each call site builds an
   `rnp_input_t`/`rnp_output_t`, runs the op, then manually destroys them —
   including the error path, which uses bespoke cleanup functions like
   `cleanup_sign` (`src/signature.rs:224-236`).
3. **Lossy error type.** Every non-success `rnp_result_t` collapses into
   `Error::Rnp { code, message }`. Callers cannot match on the category
   (signature-invalid vs. bad-password vs. key-not-found) without re-parsing
   the numeric code.

## Work items

- [x] Categorized `Error`: add `ErrorKind` enum mirroring the categories in
      `include/rnp/rnp_err.h` (Common / Storage / Crypto / Parsing / Sig-validation).
      Map by high nibble so a future unknown code still classifies as `Other`.
- [x] `Input` newtype wrapping `rnp_input_t` with `Drop`. Constructors:
      `from_memory`, `from_path`, `from_stdin`, `from_callback`.
- [x] `Output` newtype wrapping `rnp_output_t` with `Drop`. Constructors:
      `to_memory`, `to_null`, `to_path`, `to_file`, `to_stdout`, `to_armor`,
      `to_callback`. Methods: `write`, `finish`, `pipe`, `into_bytes` (drain).
- [x] Refactor `key.rs` and `signature.rs` to use `Input`/`Output`. Delete the
      duplicated `drain_memory_output` and `cleanup_sign`.
- [x] Shared `ops::raw` helpers: `check()` stays in `error`, but the
      memory-drain + buffer-destroy sequence moves to `Output::into_bytes`.
- [x] Tests: round-trip via the new abstractions (the existing `sign_verify.rs`
      exercises them end-to-end); add a focused unit test for `Error::kind()`
      mapping.

## Architecture notes

**MECE:** `Input` and `Output` are the only owners of `rnp_input_t` /
`rnp_output_t`. Every other module borrows them by `&` or owns them by value.
No code outside `ops::io` calls `rnp_input_*` / `rnp_output_*` destroyers.

**OCP:** Adding a new input source (e.g. `from_url` once we have an HTTP
fetcher) means adding a constructor on `Input`, not editing existing call sites.
Adding a new error category means adding an `ErrorKind` variant.

**DRY:** The drain sequence appears exactly once (`Output::into_bytes`).
The cleanup-on-error pattern is `Drop`, not a per-call-site function.

**Performance:** `Input::from_memory` uses the C side's `copy=true` flag so the
caller's `&[u8]` doesn't need to outlive the call — same as before, no
regression. `Output::into_bytes` consumes `self` so the destroy happens in
`Drop` without an extra indirection.

**Rust analogue of the global "no `require_relative`, use autoload" rule:** all
modules are declared in `src/lib.rs` (the parent). Public types are
re-exported from `lib.rs`. No `#[path]` overrides. A reader can reconstruct the
module tree from `lib.rs` alone.

## Acceptance criteria

- `cargo test` green.
- `grep -rn 'rnp_input_destroy\|rnp_output_destroy\|rnp_buffer_destroy' src/`
  returns matches only inside `src/ops/io.rs`.
- `Error::kind()` returns a non-`Other` value for every code documented in
  `rnp_err.h`.
- No `cleanup_*` helper functions remain.

## Completion log

**DONE** in this session. Implemented:

- `src/error.rs` — `Error` gains a `kind: ErrorKind` field on `Error::Rnp`;
  `ErrorKind` enum with 27 variants grouped by the upstream category prefixes
  (`0x10…` Common, `0x11…` Storage, `0x12…` Crypto, `0x13…` Parsing,
  `0x14…` Sig-validation). Unknown codes classify as `Other`.
- `src/ops/mod.rs`, `src/ops/io.rs` — `Input` and `Output` RAII handles.
  `Input::from_{memory,path,stdin,callback}`; `Output::to_{memory,null,path,
  file,stdout,armor,callback}`; `Output::{write,finish,pipe,into_bytes}`.
- `src/key.rs`, `src/signature.rs`, `src/keygen.rs` — refactored to use the new
  types. `cleanup_sign` and both `drain_memory_output` copies deleted.
- `tests/error_kind.rs` — exhaustive `Error::kind()` mapping for the
  documented codes.
