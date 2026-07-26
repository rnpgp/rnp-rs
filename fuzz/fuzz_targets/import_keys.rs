//! Fuzz target: `Context::import_keys` against arbitrary byte streams.
//!
//! Each iteration constructs a fresh `Context`, passes the fuzzer-generated
//! bytes as a key blob to import, and asserts that the call either
//! succeeds with valid status JSON or fails cleanly via `Err` — never
//! panics.

#![no_main]

use libfuzzer_sys::fuzz_target;
use rnp::Context;

fuzz_target!(|data: &[u8]| {
    let ctx = match Context::new() {
        Ok(c) => c,
        Err(_) => return,
    };
    let _ = ctx.import_keys(
        data,
        rnp::LoadSaveFlags::PUBLIC | rnp::LoadSaveFlags::SECRET,
    );
});
