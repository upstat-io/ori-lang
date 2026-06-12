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
/// Always returns exit code 0 on completion: pass/fail accounting flows
/// through the protocol, and a crash never reaches the return path — the
/// parent classifies it from the worker's exit status.
pub fn run_worker(paths: &[PathBuf], config: &TestRunnerConfig) -> i32 {
    let runner = TestRunner::with_config(config.clone());
    let summary = runner.run_paths(paths);

    for file in &summary.files {
        for error in &file.errors {
            protocol::emit_file_error(error);
        }
        if file.llvm_compile_error {
            protocol::emit_llvm_compile_error();
        }
    }
    protocol::emit_done();
    0
}
