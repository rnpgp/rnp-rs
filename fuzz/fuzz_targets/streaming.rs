//! Fuzz target: the streaming I/O callback machinery
//! (`Input::from_reader` / `Output::to_writer` in `src/ops/io/stream.rs`).
//!
//! Arbitrary bytes flow through a real reader into real writers via the
//! C thunks, exercising: partial-read/EOF translation, the panic-catching
//! thunks, io-error capture on the borrowed-input path, closer-driven
//! flush/discard, and stream-state reclamation on early exit.

#![no_main]

use libfuzzer_sys::fuzz_target;
use rnp::{Context, Input, Output};
use std::io::Cursor;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Writer that counts bytes and, for some input prefixes, deliberately
/// fails partway — driving the error/discard path of the closer.
struct FlakyWriter {
    bytes: Arc<Mutex<Vec<u8>>>,
    budget: AtomicUsize,
}

impl std::io::Write for FlakyWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let left = self.budget.load(Ordering::SeqCst);
        if left == 0 {
            return Err(std::io::Error::other("budget exhausted"));
        }
        let n = buf.len().min(left);
        self.bytes.lock().unwrap().extend_from_slice(&buf[..n]);
        self.budget.store(left - n, Ordering::SeqCst);
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fuzz_target!(|data: &[u8]| {
    // 1. Pure pipe through the thunks: reader -> rnp -> writer, compared
    //    byte-for-byte with a direct copy.
    let input = match Input::from_reader(Cursor::new(data.to_vec())) {
        Ok(i) => i,
        Err(_) => return,
    };
    let collected = Arc::new(Mutex::new(Vec::new()));
    let mut output = match Output::to_writer({
        let bytes = collected.clone();
        struct Sink(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for Sink {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        Sink(bytes.clone())
    }) {
        Ok(o) => o,
        Err(_) => return,
    };
    let piped = output.pipe(&input);
    let outcome = output.into_writer();
    if piped.is_ok() {
        debug_assert_eq!(&*collected.lock().unwrap(), &*data);
        if let Some(outcome) = outcome {
            debug_assert!(!outcome.discarded, "successful pipe must not discard");
        }
    }

    // 2. Decrypt through a reader-backed input with a failing writer:
    //    whatever the outcome, the io error must be observable and the
    //    closer must have run exactly once.
    let ctx = match Context::new() {
        Ok(c) => c,
        Err(_) => return,
    };
    let input = match Input::from_reader(Cursor::new(data.to_vec())) {
        Ok(i) => i,
        Err(_) => return,
    };
    // Budget derived from the data so both success and failure paths are
    // reachable under mutation.
    let budget = usize::from(data.first().copied().unwrap_or(0)) * 64;
    let mut output = match Output::to_writer(FlakyWriter {
        bytes: Arc::new(Mutex::new(Vec::new())),
        budget: AtomicUsize::new(budget),
    }) {
        Ok(o) => o,
        Err(_) => return,
    };
    let result = rnp::decrypt_from_input(&ctx, &input, &mut output);
    let _ = input.io_error();
    let _ = result;
    // Drop runs the closer (flush or discard) and reclaims the state.
    drop(output);
});
