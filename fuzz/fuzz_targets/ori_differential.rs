#![no_main]

//! Differential oracle fuzz target — eval (in-process) vs AOT (subprocess).
//!
//! Stub at §10.1. The actual differential implementation — typed-program
//! generator (§10.2), in-process eval execution, subprocess AOT compilation
//! and execution, structural exit-category comparison, canonicalized stdout
//! comparison, leak-count comparison — lands in §10.4.
//!
//! At §10.1 the body just consumes the input so the binary builds and the
//! fuzz infrastructure (cargo-fuzz, libfuzzer-sys, workspace exclude, ASan
//! defaults) is exercised end-to-end.

use libfuzzer_sys::fuzz_target;

// TODO: §10.4 — full differential implementation.
fuzz_target!(|data: &[u8]| {
    let _ = data;
});
