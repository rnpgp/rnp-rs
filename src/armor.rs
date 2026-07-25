//! ASCII-armor and dearmor wrappers, plus content-type sniffing.
//!
//! These wrap [`rnp_enarmor`], [`rnp_dearmor`], and [`rnp_guess_contents`].
//! For streaming armor wrapping of an existing [`Output`], see
//! [`crate::Output::to_armor`].

use crate::error::{self, check, Result};
use crate::ffi;
use crate::ops::{cstr_to_optional_string, ArmorType, Input, Output};
use std::ffi::CString;
use std::ptr;

/// Add ASCII armor to `input`'s bytes, writing the result to `output`.
///
/// `ty` selects the armor header (`"-----BEGIN PGP MESSAGE-----"` etc.).
/// Pass `None` to let librnp guess.
pub fn enarmor(input: &Input, output: &mut Output, ty: Option<ArmorType>) -> Result<()> {
    let cstr = ty.map(|t| CString::new(t.as_str()).unwrap());
    let ptr = cstr.as_ref().map(|c| c.as_ptr()).unwrap_or(ptr::null());
    unsafe { check(ffi::rnp_enarmor(input.as_ptr(), output.as_ptr(), ptr)) }
}

/// Remove ASCII armor from `input`, writing the binary OpenPGP packets to
/// `output`.
pub fn dearmor(input: &Input, output: &mut Output) -> Result<()> {
    unsafe { check(ffi::rnp_dearmor(input.as_ptr(), output.as_ptr())) }
}

/// The kind of OpenPGP content a byte stream appears to be, as guessed by
/// `rnp_guess_contents`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    Message,
    PublicKey,
    SecretKey,
    Signature,
    Cleartext,
    /// librnp couldn't identify the stream.
    Unknown,
}

impl ContentType {
    fn from_cstr(s: &str) -> Self {
        match s {
            "message" => ContentType::Message,
            "public key" => ContentType::PublicKey,
            "secret key" => ContentType::SecretKey,
            "signature" => ContentType::Signature,
            "cleartext signed message" | "cleartext" => ContentType::Cleartext,
            _ => ContentType::Unknown,
        }
    }
}

/// Peek at `input` and report its likely content type. Does not consume the
/// stream — the input is still usable for further calls.
pub fn guess_contents(input: &Input) -> Result<ContentType> {
    let mut raw: *mut std::os::raw::c_char = ptr::null_mut();
    unsafe {
        check(ffi::rnp_guess_contents(input.as_ptr(), &mut raw))?;
        let s = cstr_to_optional_string(raw).unwrap_or_default();
        Ok(ContentType::from_cstr(&s))
    }
}

/// Convenience: armor `bytes` and return the armored string.
pub fn armor_bytes(bytes: &[u8], ty: ArmorType) -> Result<Vec<u8>> {
    let input = Input::from_memory(bytes)?;
    let mut output = Output::to_memory()?;
    enarmor(&input, &mut output, Some(ty))?;
    output.into_bytes()
}

/// Convenience: dearmor `bytes` and return the binary OpenPGP packets.
pub fn dearmor_bytes(bytes: &[u8]) -> Result<Vec<u8>> {
    let input = Input::from_memory(bytes)?;
    let mut output = Output::to_memory()?;
    dearmor(&input, &mut output)?;
    output.into_bytes()
}

// Re-export the error type for convenience at the module boundary.
#[allow(unused_imports)]
use error as _error_reexport;
