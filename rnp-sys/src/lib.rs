//! Raw FFI bindings to the `librnp` C API (`rnp/rnp.h`), generated at build
//! time by bindgen — or taken from the pregenerated `bindings/` file when
//! the headers are known to match, so cross builds don't need a working
//! host-side libclang.
//!
//! The contents of this crate are entirely machine-generated and mirror the
//! upstream C API verbatim, including its naming conventions. The lint
//! suppressions below acknowledge that: C identifiers are not Rust-idiomatic
//! and we do not want to fight the compiler over them.
//!
//! Safe, idiomatic wrappers live in the `rnp` crate (`rnp-rs`); most users
//! want that instead.

#![allow(
    non_camel_case_types,
    non_snake_case,
    dead_code,
    non_upper_case_globals
)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
