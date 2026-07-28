//! Raw FFI bindings to `librnp`, generated at build time by `bindgen` from the
//! installed `rnp/rnp.h` header.
//!
//! The contents of this module are entirely machine-generated and mirror the
//! upstream C API verbatim, including its naming conventions. The lint
//! suppressions below acknowledge that: C identifiers are not Rust-idiomatic
//! and we do not want to fight the compiler over them.

#![allow(non_camel_case_types, non_snake_case, dead_code, non_upper_case_globals)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
