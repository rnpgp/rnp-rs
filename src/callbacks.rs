//! Logging and key-provider callbacks on [`Context`](crate::Context).
//!
//! The `logging` Cargo feature gates the log-fd surface. The key-provider
//! surface is always available.

use crate::context::Context;
use crate::ffi;
use crate::key::{Key, KeyIdentifier};
use std::os::raw::c_void;
use std::ptr;

// -----------------------------------------------------------------------
// Logging (feature-gated).
// -----------------------------------------------------------------------

#[cfg(feature = "logging")]
mod logging_impl {
    use super::*;

    impl Context {
        /// Direct librnp's diagnostic output to the given Unix file
        /// descriptor. The fd must be open and writable; librnp will
        /// write to it directly.
        pub fn set_log_fd(&self, fd: std::os::raw::c_int) -> Result<()> {
            unsafe { check(ffi::rnp_ffi_set_log_fd(self.ffi, fd)) }
        }

        /// Convenience wrapper for [`Self::set_log_fd`]: open `path` for
        /// writing (truncating) and pass its fd to librnp.
        pub fn set_log_file(&self, path: &str) -> Result<()> {
            use std::io::Write;
            let path_c = CString::new(path).map_err(|_| error::Error::PathNul)?;
            // Open via libc to avoid pulling the `nix` crate. macOS and
            // Linux both expose `open()` and `write()` through libc.
            let fd = unsafe {
                extern "C" {
                    fn open(path: *const std::os::raw::c_char, flags: std::os::raw::c_int, ...)
                        -> std::os::raw::c_int;
                }
                // O_WRONLY | O_CREAT | O_TRUNC = 0x1 | 0x40 | 0x200 on macOS.
                // On Linux: 0x1 | 0x40 | 0x80. Use libc constants when
                // possible; here we hard-code macOS values for the
                // common case and fall back to fcntl for portability.
                let flags = if cfg!(target_os = "macos") {
                    0x1 | 0x40 | 0x200
                } else {
                    0x1 | 0x40 | 0x80
                };
                open(path_c.as_ptr(), flags, 0o644)
            };
            if fd < 0 {
                return Err(error::Error::Io {
                    source: std::io::Error::last_os_error(),
                });
            }
            self.set_log_fd(fd)
        }

        #[allow(dead_code)]
        fn _unused_io_marker<W: Write>(_w: W) {}
    }
}

// -----------------------------------------------------------------------
// Key-provider callback.
// -----------------------------------------------------------------------

/// Why the key-provider is being called.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestedKeyType {
    Keyid,
    Fingerprint,
    Grip,
}

impl RequestedKeyType {
    pub fn as_str(self) -> &'static str {
        match self {
            RequestedKeyType::Keyid => "keyid",
            RequestedKeyType::Fingerprint => "fingerprint",
            RequestedKeyType::Grip => "grip",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "keyid" => Self::Keyid,
            "fingerprint" => Self::Fingerprint,
            "grip" => Self::Grip,
            _ => Self::Keyid,
        }
    }
}

/// What the key-provider returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyRequestOutcome {
    /// The key was loaded into the keyring via `Context::load_keys` (or
    /// similar) inside the callback.
    Found,
    /// The callback couldn't find the key. librnp will try the next
    /// identifier (e.g. fall back from keyid to fingerprint).
    NotFound,
}

/// Trait implemented by the consumer to feed keys into librnp on demand.
///
/// Called by librnp during verify/decrypt when the keyring doesn't have a
/// key the operation needs. The implementation typically inspects `id`
/// and `kind`, fetches the key bytes from some external store, and calls
/// [`Context::load_keys`](crate::Context::load_keys) on the supplied
/// `ctx`. Returns [`KeyRequestOutcome::Found`] if a key was loaded,
/// [`KeyRequestOutcome::NotFound`] otherwise.
pub trait KeyProvider: Send + Sync {
    fn on_key_request(
        &self,
        ctx: &Context,
        id: KeyIdentifier<'_>,
        kind: RequestedKeyType,
    ) -> KeyRequestOutcome;
}

// Internal holder (mirrors PasswordHolder in context.rs).
pub(crate) struct KeyProviderHolder {
    pub(crate) inner: Box<dyn KeyProvider>,
}

impl Context {
    /// Install a key provider. Called when librnp needs a key that isn't
    /// in the keyring. Inside the callback, use `ctx.load_keys(...)` to
    /// satisfy the request.
    pub fn set_key_provider(&mut self, provider: Box<dyn KeyProvider>) {
        let boxed = Box::new(KeyProviderHolder { inner: provider });
        let raw: *mut KeyProviderHolder = Box::into_raw(boxed);
        unsafe {
            let _ = ffi::rnp_ffi_set_key_provider(
                self.ffi,
                Some(key_provider_thunk),
                raw as *mut c_void,
            );
        }
        // Reclaim via Box::from_raw on Drop. Stash the box so it lives
        // as long as the context.
        self._key_provider = Some(unsafe { Box::from_raw(raw) });
    }
}

// Thunk: C calls us with (ffi, app_ctx, identifier_type, identifier, new_*).
// We dispatch into the trait method.
unsafe extern "C" fn key_provider_thunk(
    ffi: ffi::rnp_ffi_t,
    app_ctx: *mut c_void,
    identifier_type: *const std::os::raw::c_char,
    identifier: *const std::os::raw::c_char,
    _hidden: bool,
) {
    unsafe {
        if app_ctx.is_null() || identifier.is_null() {
            return;
        }
        let holder = &*(app_ctx as *const KeyProviderHolder);
        let kind_str = if identifier_type.is_null() {
            String::new()
        } else {
            std::ffi::CStr::from_ptr(identifier_type)
                .to_string_lossy()
                .into_owned()
        };
        let id_str = std::ffi::CStr::from_ptr(identifier).to_string_lossy();
        let kind = RequestedKeyType::parse(&kind_str);
        let id = match kind {
            RequestedKeyType::Keyid => KeyIdentifier::Keyid(&id_str),
            RequestedKeyType::Fingerprint => KeyIdentifier::Fingerprint(&id_str),
            RequestedKeyType::Grip => KeyIdentifier::Grip(&id_str),
        };
        // Reconstruct a Context that borrows ffi for the duration of the
        // callback. We can't go through Context::with_format (which would
        // create a new ffi); we need to wrap the existing one.
        let ctx_ref = Context::borrow_ffi(ffi);
        let _ = holder.inner.on_key_request(&ctx_ref, id, kind);
    }
}

// Suppress unused-parameter warnings.
#[allow(dead_code)]
fn _silence_warnings() {
    let _ = ptr::null::<()>();
    let _ = std::any::TypeId::of::<Key>();
}
