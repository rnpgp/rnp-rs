# rnp-rs fuzz harness

cargo-fuzz + libFuzzer targets for the safe Rust API. Each target
exercises one entry point with mutation-guided random input.

## Running

```sh
# Install the cargo-fuzz subcommand (one-time):
cargo install cargo-fuzz

# Run a single target for 60 seconds:
cargo +nightly fuzz run decrypt -- -max_total_time=60

# Run all targets:
for t in decrypt verify load_keys dump_packets dearmor import_keys signature_parse streaming; do
    cargo +nightly fuzz run "$t" -- -max_total_time=60
done
```

Requires nightly Rust (libFuzzer integration is nightly-only).

`streaming` additionally drives the `Input::from_reader` /
`Output::to_writer` C-thunk machinery (partial reads, EOF, io-error
capture, closer flush/discard, state reclamation) against real pipe and
decrypt paths.

## What's checked

Each target exercises the safe Rust API (not raw FFI). The invariant
is **no panic, no UB** for any input. A panic indicates a Rust-side
bug; a memory error indicates an unsafe-block bug. Both should be
reported as issues.

Targets deliberately re-create a `Context` per iteration — that's
cheap relative to the FFI call, and avoids shared state.

## Corpus

Corpus files persist under `fuzz/corpus/<target>/`. The directory is
gitignored; treat it as ephemeral.

## Bugs

If a target finds a bug:

1. Reproduce with `cargo +nightly fuzz run <target> --
   fuzz/artifacts/<target>/crash-<hash>`.
2. File an issue with the reproducer and the panic trace.
3. Add a regression test to `tests/` once fixed.
