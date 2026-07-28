//! Logging — `Context::set_log_fd` + `set_log_file` (feature-gated).

#![cfg(feature = "logging")]

use crate::context::Context;
use crate::error::{self, Result, check};
use crate::ffi;
use std::ffi::CString;

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
        let fd = unsafe {
            extern "C" {
                fn open(
                    path: *const std::os::raw::c_char,
                    flags: std::os::raw::c_int,
                    ...
                ) -> std::os::raw::c_int;
            }
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
