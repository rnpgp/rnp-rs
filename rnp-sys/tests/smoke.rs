//! Smoke tests for the rnp-sys FFI surface.
//!
//! These don't exercise cryptographic operations — they verify that the
//! raw FFI is wired correctly: every call returns a sensible result, every
//! handle is destroyable, and every allocated buffer is freeable with
//! `rnp_buffer_destroy` as the upstream C API documents.
//!
//! Constants like `RNP_SUCCESS` live in `rnp_err.h`, which `wrapper.h`
//! does not include — they're not generated into the bindings. We don't
//! assert specific numeric return codes.
//!
//! The two handle-lifecycle tests are gated on `--features vendored`
//! because `rnp_ffi_create` and `rnp_get_default_homedir` succeed only
//! against a librnp new enough to support the bindings' expectations;
//! host-installed system librnp versions vary. The version-string test
//! is unconditional — it's been present in every librnp release and is
//! the cheapest possible proof that the FFI is wired.

use std::ffi::CStr;
use std::ptr;

use std::ffi::CString;

use rnp_sys::{
    rnp_buffer_destroy, rnp_ffi_create, rnp_ffi_destroy, rnp_get_default_homedir,
    rnp_version_string,
};

/// `rnp_version_string()` returns a non-null C string identifying the linked
/// librnp. The pointer lives for the process lifetime.
#[test]
fn version_string_is_available() {
    unsafe {
        let ptr = rnp_version_string();
        assert!(!ptr.is_null(), "rnp_version_string returned NULL");
        let version = CStr::from_ptr(ptr).to_str().unwrap();
        assert!(!version.is_empty());
    }
}

/// `rnp_ffi_create` followed by `rnp_ffi_destroy` proves the FFI handle
/// lifecycle is callable end-to-end. Requires `--features vendored` so the
/// linked librnp is the pinned 0.18.1 — old system librnp versions return
/// errors that leave the handle unset.
#[cfg(feature = "vendored")]
#[test]
fn can_create_and_destroy_ffi_handle() {
    unsafe {
        let mut ffi = ptr::null_mut();
        // rnp_ffi_create requires non-NULL keystore format strings ("GPG",
        // "KBX", ...); NULL triggers RNP_ERROR_NULL_POINTER.
        let pub_format = CString::new("GPG").unwrap();
        let sec_format = CString::new("GPG").unwrap();
        let rc = rnp_ffi_create(&mut ffi, pub_format.as_ptr(), sec_format.as_ptr());
        assert_eq!(rc, 0, "rnp_ffi_create failed (rc={rc})");
        assert!(!ffi.is_null(), "rnp_ffi_create returned a NULL handle");
        rnp_ffi_destroy(ffi);
    }
}

/// `rnp_get_default_homedir` allocates a buffer the caller must free via
/// `rnp_buffer_destroy`. Same vendored-only rationale as the create/destroy
/// test.
#[cfg(feature = "vendored")]
#[test]
fn can_call_allocating_api_and_free_with_rnp_buffer_destroy() {
    unsafe {
        let mut homedir = ptr::null_mut();
        let rc = rnp_get_default_homedir(&mut homedir);
        assert_eq!(rc, 0, "rnp_get_default_homedir failed (rc={rc})");
        assert!(!homedir.is_null());
        let value = CStr::from_ptr(homedir).to_str().unwrap();
        assert!(!value.is_empty());
        rnp_buffer_destroy(homedir.cast());
    }
}
