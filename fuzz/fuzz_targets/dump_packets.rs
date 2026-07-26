//! Fuzz target: `rnp::dump_packets_to_json` (librnp's packet parser
//! exposed via Rust).

#![no_main]

use libfuzzer_sys::fuzz_target;
use rnp::dump_packets_bytes_to_json;
use rnp::JsonDumpFlags;

fuzz_target!(|data: &[u8]| {
    let _ = dump_packets_bytes_to_json(data, JsonDumpFlags::default());
});
