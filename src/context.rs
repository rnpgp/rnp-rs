//! Top-level librnp FFI handle.
//!
//! [`Context`] is the entry point to the library: it wraps an `rnp_ffi_t` and
//! owns its lifetime. Key rings, signing operations and verification operations
//! are all performed through a `Context`.

use crate::error::{self, check, Result};
use crate::ffi;
use std::ffi::CString;
use std::os::raw::c_void;
use std::ptr;

/// Wraps `rnp_ffi_t`. Destroyed on drop via `rnp_ffi_destroy`.
///
/// All higher-level handles (keys, signing ops, etc.) borrow the `Context`
/// they were created from for at most its lifetime.
pub struct Context {
    pub(crate) ffi: ffi::rnp_ffi_t,
    // Boxed password provider, kept alive so the C callback can deref it.
    // `Send` is intentionally not implemented: librnp handles are not safe to
    // move between threads.
    _password_provider: Option<Box<PasswordHolder>>,
}

/// What the library calls when it needs a passphrase to unlock a secret key.
///
/// Implementations return the password as `Cow<str>`; it is copied into the
/// librnp-provided buffer before the callback returns.
pub trait PasswordProvider: Send + Sync {
    fn get_password(
        &self,
        key: Option<&crate::key::Key>,
        context: &str,
    ) -> Option<std::borrow::Cow<'_, str>>;
}

// Internal holder that we can hand a raw pointer to.
struct PasswordHolder {
    inner: Box<dyn PasswordProvider>,
}

impl Context {
    /// Create a new context with the GPG keyring format (the most common one).
    pub fn new() -> Result<Self> {
        Self::with_format(KeyringFormat::Gpg, KeyringFormat::Gpg)
    }

    /// Create a new context with explicit keyring formats.
    pub fn with_format(
        pub_format: KeyringFormat,
        sec_format: KeyringFormat,
    ) -> Result<Self> {
        let pub_c = CString::new(pub_format.as_str()).unwrap();
        let sec_c = CString::new(sec_format.as_str()).unwrap();
        let mut ffi_h: ffi::rnp_ffi_t = ptr::null_mut();
        // SAFETY: passing valid C strings and a valid out-pointer.
        unsafe {
            check(ffi::rnp_ffi_create(
                &mut ffi_h,
                pub_c.as_ptr(),
                sec_c.as_ptr(),
            ))?;
        }
        if ffi_h.is_null() {
            return Err(error::Error::NullPointer);
        }
        Ok(Context {
            ffi: ffi_h,
            _password_provider: None,
        })
    }

    /// Install a password provider. Must be called before any operation that
    /// may need to unlock a secret key.
    pub fn set_password_provider(&mut self, provider: Box<dyn PasswordProvider>) {
        let boxed = Box::new(PasswordHolder { inner: provider });
        let raw: *mut PasswordHolder = Box::into_raw(boxed);
        // SAFETY: rnp_ffi_set_passProvider takes a C callback + opaque ctx.
        // The thunk (below) derefs `raw` only while `self` is alive; we re-box
        // it on drop.
        unsafe {
            let _ = ffi::rnp_ffi_set_pass_provider(
                self.ffi,
                Some(password_thunk),
                raw as *mut c_void,
            );
        }
        // Reclaim via Box::from_raw in Drop. Stash the box.
        self._password_provider = Some(unsafe { Box::from_raw(raw) });
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        // Clear the provider first so the C side stops calling into freed
        // memory if rnp_ffi_destroy triggers any final callbacks.
        self._password_provider.take();
        if !self.ffi.is_null() {
            // SAFETY: ffi was created by rnp_ffi_create and not yet destroyed.
            unsafe {
                let _ = ffi::rnp_ffi_destroy(self.ffi);
            }
            self.ffi = ptr::null_mut();
        }
    }
}

// librnp handles contain raw pointers and so are not Send/Sync by default;
// we rely on that auto-trait negative impl rather than spelling it out, since
// the explicit `impl !Send` form requires the unstable `negative_impls` feature.

/// Keyring container format passed to `rnp_ffi_create`.
#[derive(Clone, Copy, Debug)]
pub enum KeyringFormat {
    Gpg,
    Kbx,
    G10,
    Json,
}

impl KeyringFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            KeyringFormat::Gpg => "GPG",
            KeyringFormat::Kbx => "KBX",
            KeyringFormat::G10 => "G10",
            KeyringFormat::Json => "JSON",
        }
    }
}

// --- password callback thunk ---------------------------------------------
//
// Bridges librnp's C callback to our `PasswordProvider` trait. We do not
// currently surface the `key` argument as a fully fledged `Key` because doing
// so safely within the callback's lifetime is fiddly; it is left as a TODO.

unsafe extern "C" fn password_thunk(
    _ffi: ffi::rnp_ffi_t,
    app_ctx: *mut c_void,
    key: ffi::rnp_key_handle_t,
    pgp_context: *const std::os::raw::c_char,
    buf: *mut std::os::raw::c_char,
    buf_len: usize,
) -> bool {
    if app_ctx.is_null() {
        return false;
    }
    // The whole body is unsafe because edition 2024 no longer treats `unsafe
    // fn` bodies as implicitly unsafe. Group everything that touches raw
    // pointers under one block.
    unsafe {
        // SAFETY: app_ctx was placed there by Box::into_raw in set_password_provider.
        let provider = &*(app_ctx as *const PasswordHolder);

        let context_str = if pgp_context.is_null() {
            "".to_string()
        } else {
            std::ffi::CStr::from_ptr(pgp_context)
                .to_string_lossy()
                .into_owned()
        };

        // TODO: surface `key` as a borrowed crate::key::Key. For now pass None.
        let _ = key;
        let password = provider.inner.get_password(None, &context_str);
        let Some(password) = password else { return false };

        let bytes = password.as_bytes();
        if bytes.len() + 1 > buf_len {
            return false;
        }
        // SAFETY: buf is buf_len bytes, we just verified there's room.
        std::ptr::copy_nonoverlapping(bytes.as_ptr() as *const u8, buf as *mut u8, bytes.len());
        *buf.add(bytes.len()) = 0;
        true
    }
}
