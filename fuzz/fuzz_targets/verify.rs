//! Fuzz target: `rnp::verify` and `rnp::verify_detached` against
//! arbitrary bytes.

#![no_main]

use libfuzzer_sys::fuzz_target;
use rnp::Context;

fuzz_target!(|data: &[u8]| {
    let ctx = match Context::new() {
        Ok(c) => c,
        Err(_) => return,
    };
    let _ = rnp::verify(&ctx, data);
    // Detached verify: treat the bytes as the signature and a fixed
    // (empty) message. The fuzz question is "does verify_detached
    // handle arbitrary sig bytes without panic".
    let _ = rnp::verify_detached(&ctx, b"", data);
});
