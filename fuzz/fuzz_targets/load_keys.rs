//! Fuzz target: `Context::load_keys` against arbitrary bytes.

#![no_main]

use libfuzzer_sys::fuzz_target;
use rnp::{Context, KeyringFormat, LoadSaveFlags};

fuzz_target!(|data: &[u8]| {
    let ctx = match Context::new() {
        Ok(c) => c,
        Err(_) => return,
    };
    let _ = ctx.load_keys(KeyringFormat::Gpg, data, LoadSaveFlags::PUBLIC);
});
