//! `aims-proof-checker` binary entry point.
//!
//! Thin wrapper around `aims_proof_checker::cli::run` — delegates all
//! arg parsing + dispatch + exit-code routing to the library so the
//! integration tests at `tests/smoke.rs` can drive the same code path
//! without spawning a subprocess.

fn main() {
    let exit_code = aims_proof_checker::cli::run(std::env::args().collect());
    std::process::exit(exit_code);
}
