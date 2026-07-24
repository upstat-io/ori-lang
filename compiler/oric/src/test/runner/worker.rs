//! Worker-mode entry for subprocess-isolated test execution.
//!
//! A worker runs the file(s) it was handed IN-PROCESS (the per-test
//! `test::protocol` records stream from the run loops) and finishes by
//! emitting file-level errors plus the `done` record. The parent owns all
//! human-readable output; a worker writes only protocol lines and the test
//! programs' own output.

use std::path::PathBuf;

use super::super::protocol;
use super::{TestRunner, TestRunnerConfig};

/// Run test paths in worker mode, streaming the wire protocol to stdout.
///
/// Requires the per-spawn protocol nonce in `ORI_TEST_PROTOCOL_TOKEN` (set
/// by the parent runner): without it every protocol line would be
/// un-stamped passthrough, so the worker refuses to run (exit code 1).
///
/// Returns exit code 0 on completion otherwise: pass/fail accounting flows
/// through the protocol, and a crash never reaches the return path — the
/// parent classifies it from the worker's exit status.
pub fn run_worker(paths: &[PathBuf], config: &TestRunnerConfig) -> i32 {
    let Some(token) = take_protocol_token() else {
        eprintln!(
            "error: --__worker requires {} (set per spawn by the parent test runner)",
            crate::debug_flags::ORI_TEST_PROTOCOL_TOKEN
        );
        return 1;
    };

    let mut config = config.clone();
    config.protocol_token = Some(token.clone());
    let runner = TestRunner::with_config(config);
    let summary = runner.run_paths(paths);

    for file in &summary.files {
        for error in &file.errors {
            protocol::emit_file_error(&token, error);
        }
        if file.llvm_compile_error {
            protocol::emit_llvm_compile_error(&token);
        }
    }
    protocol::emit_done(&token);
    0
}

/// Read the per-spawn protocol nonce and scrub it from the worker's own
/// environment BEFORE any test code runs.
///
/// INVARIANT: JIT'd test code (and any process it spawns) shares the
/// worker's environment and must never see the token — a visible token
/// would let test programs forge protocol records. The scrub runs
/// unconditionally, even for an empty/absent token.
///
/// No `.ori`-level pin exists: `std.env` is an unimplemented stub, so test
/// programs cannot read environment variables yet; the unit pins on this
/// function are the guard.
pub(super) fn take_protocol_token() -> Option<String> {
    let token = std::env::var(crate::debug_flags::ORI_TEST_PROTOCOL_TOKEN)
        .ok()
        .filter(|token| !token.is_empty());
    std::env::remove_var(crate::debug_flags::ORI_TEST_PROTOCOL_TOKEN);
    token
}

#[cfg(test)]
mod tests;
