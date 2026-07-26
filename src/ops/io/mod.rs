//! RAII wrappers around `rnp_input_t` and `rnp_output_t`.
//!
//! Every byte-stream that crosses the Rust/C boundary in this crate goes
//! through [`Input`] or [`Output`]. They own the underlying handle and free
//! it on `Drop`, even on error paths reached via `?`. This eliminates the
//! bespoke cleanup functions that previously appeared in `signature.rs` and
//! the duplicated `drain_memory_output` helpers in `key.rs` /
//! `signature.rs`.
//!
//! The C-string / buffer-destroy helpers live in [`crate::ffi_safe`]; this
//! module re-exports them for backward compatibility with callers that
//! import from `crate::ops::*`.
//!
//! ## Listener model
//!
//! Callback-based inputs and outputs are deferred to a later phase (they
//! require careful lifetime design around boxed trait objects). Memory, path,
//! stdin/stdout, file, null, and armor destinations are all supported here.
//!
//! ## Module layout
//!
//! | Sub-module    | Concern                                                |
//! |---------------|--------------------------------------------------------|
//! | `input`       | `Input` RAII                                           |
//! | `output`      | `Output` RAII + `OutputFileFlags`                      |
//! | `armor_type`  | `ArmorType` enum                                       |

// C-string / buffer return helpers — defined in [`crate::ffi_safe`].
// Re-exported here for compatibility with existing callers that import from
// `crate::ops::*`. New code should import directly from `crate::ffi_safe`.
pub use crate::ffi_safe::{
    call_for_optional_string, call_for_string, cstr_to_optional_string, cstr_to_string,
};

mod armor_type;
mod input;
mod output;

pub use armor_type::ArmorType;
pub use input::Input;
pub use output::{Output, OutputFileFlags};
