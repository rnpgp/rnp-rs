//! Fuzz target: `rnp::dearmor` against arbitrary bytes.

#![no_main]

use libfuzzer_sys::fuzz_target;
use rnp::{dearmor_bytes, Output};

fuzz_target!(|data: &[u8]| {
    // Convenience function path.
    let _ = dearmor_bytes(data);

    // Streaming path: input + output, makes sure the Output RAII handles
    // survive an early-exit on parse failure.
    let input = match rnp::Input::from_memory(data) {
        Ok(i) => i,
        Err(_) => return,
    };
    let mut output = match Output::to_memory() {
        Ok(o) => o,
        Err(_) => return,
    };
    let _ = rnp::dearmor(&input, &mut output);
});
