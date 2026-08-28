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
//! ## Streaming
//!
//! Beyond the in-memory / path / stdin-stdout destinations, any
//! [`std::io::Read`] plugs in via [`Input::from_reader`] and any
//! [`std::io::Write`] via [`Output::to_writer`], so large or non-seekable
//! data (network streams, pipes) can be processed without buffering.
//! Builders that consume a message accept a caller-built [`Input`] through
//! their `*_with_input` constructors (`Signer::new_with_input`,
//! `Encryptor::new_with_input`, `VerifyOp::inline_with_input` /
//! `detached_with_input`, [`decrypt_from_input`](crate::decrypt_from_input)).
//! See the `stream` sub-module docs for the callback, error, and panic
//! semantics.
//!
//! ## Module layout
//!
//! | Sub-module    | Concern                                                |
//! |---------------|--------------------------------------------------------|
//! | `input`       | `Input` RAII                                           |
//! | `output`      | `Output` RAII + `OutputFileFlags` + `WriterOutcome`    |
//! | `armor_type`  | `ArmorType` enum                                       |
//! | `stream`      | Read/Write ↔ librnp callback plumbing                  |

// C-string / buffer return helpers — defined in [`crate::ffi_safe`].
// Re-exported here for compatibility with existing callers that import from
// `crate::ops::*`. New code should import directly from `crate::ffi_safe`.
pub use crate::ffi_safe::{
    call_for_optional_string, call_for_string, cstr_to_optional_string, cstr_to_string,
};

mod armor_type;
mod input;
mod output;
mod stream;

pub use armor_type::ArmorType;
pub use input::Input;
pub use output::{Output, OutputFileFlags, WriterOutcome};

/// Where an operation gets its message bytes from: either a caller-owned
/// slice, or a caller-built [`Input`] (e.g. from
/// [`Input::from_reader`](Input::from_reader)).
///
/// Used by the builders that offer both byte-slice and `Input`-taking
/// constructors, so the streaming path shares the byte path's execution
/// code exactly.
pub(crate) enum ByteSource<'a> {
    Bytes(&'a [u8]),
    Owned(Input),
}

impl std::fmt::Debug for ByteSource<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ByteSource::Bytes(bytes) => f.debug_tuple("Bytes").field(bytes).finish(),
            ByteSource::Owned(_) => f.debug_tuple("Owned").field(&"<input>").finish(),
        }
    }
}

impl ByteSource<'_> {
    pub(crate) fn into_input(self) -> crate::error::Result<Input> {
        match self {
            ByteSource::Bytes(bytes) => Input::from_memory(bytes),
            ByteSource::Owned(input) => Ok(input),
        }
    }

    /// Take the [`Input`] out, leaving an empty byte source behind. Lets a
    /// consuming builder extract the input and still borrow itself for the
    /// rest of the operation.
    pub(crate) fn take(&mut self) -> crate::error::Result<Input> {
        std::mem::replace(self, ByteSource::Bytes(b"")).into_input()
    }
}

/// When an operation fails on a stream-backed input, prefer the recorded
/// [`std::io::Error`] over librnp's generic `RNP_ERROR_READ`/`WRITE` code —
/// the io error says *why* the stream failed. Used by the ops that consume
/// the input on the Rust side (and therefore can still reach its error
/// slot at failure time).
pub(crate) fn or_stream_error<T>(
    result: crate::error::Result<T>,
    input: &mut Input,
) -> crate::error::Result<T> {
    if result.is_err()
        && let Some(io) = input.take_io_error()
    {
        return Err(io.into());
    }
    result
}
