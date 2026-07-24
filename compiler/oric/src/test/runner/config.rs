//! Test-runner configuration types and config-derived helpers.

use std::path::PathBuf;

/// Backend for test execution.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Backend {
    /// Tree-walking interpreter (default).
    #[default]
    Interpreter,
    /// LLVM JIT compiler.
    LLVM,
}

/// Output format for the test-result summary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable text summary (default).
    #[default]
    Text,
    /// Machine-readable JSON object carrying the full summary: aggregate
    /// counts plus a `files` array of per-file/per-test results and durations.
    Json,
}

/// Configuration for the test runner.
#[derive(Clone, Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Config struct: each bool controls an independent flag"
)]
pub struct TestRunnerConfig {
    /// Filter tests by name pattern (substring match).
    pub filter: Option<String>,
    /// Enable verbose output.
    pub verbose: bool,
    /// Run tests in parallel.
    pub parallel: bool,
    /// Generate coverage report.
    pub coverage: bool,
    /// Backend to use for execution.
    pub backend: Backend,
    /// Enable incremental test execution (skip tests whose targets are unchanged).
    pub incremental: bool,
    /// Worker mode: run in-process and stream the per-test line protocol
    /// (`test::protocol`) to stdout. Set by the hidden `--__worker` flag the
    /// parent runner passes when isolating LLVM test files in subprocesses.
    pub worker_protocol: bool,
    /// Per-spawn protocol nonce stamped onto every emitted protocol line
    /// (forgery guard — see `test::protocol` module docs). Populated in
    /// worker mode from `ORI_TEST_PROTOCOL_TOKEN`; `None` otherwise.
    pub protocol_token: Option<String>,
    /// Worker mode: names of tests whose targets the PARENT's incremental
    /// cache proved unchanged. Set by the hidden `--__skip-unchanged=` flag
    /// the parent passes when spawning a worker; the worker reports each as
    /// `SkippedUnchanged` without running it. The parent owns the incremental
    /// cache — a worker never consults a cache of its own.
    pub skip_unchanged: Vec<String>,
    /// Summary output format. `Text` (default) prints the human-readable
    /// summary; `Json` emits the full machine-readable summary object.
    pub format: OutputFormat,
}

impl Default for TestRunnerConfig {
    fn default() -> Self {
        TestRunnerConfig {
            filter: None,
            verbose: false,
            parallel: true,
            coverage: false,
            backend: Backend::Interpreter,
            incremental: false,
            worker_protocol: false,
            protocol_token: None,
            skip_unchanged: Vec::new(),
            format: OutputFormat::Text,
        }
    }
}

/// On-disk incremental-cache path for this config, if persistence applies.
///
/// `None` unless incremental mode is on, the run is NOT a worker (the parent
/// owns the cache file exclusively), and `ORI_TEST_INCREMENTAL_CACHE` names a
/// non-empty path.
pub(super) fn incremental_cache_path(config: &TestRunnerConfig) -> Option<PathBuf> {
    if !config.incremental || config.worker_protocol {
        return None;
    }
    std::env::var(crate::debug_flags::ORI_TEST_INCREMENTAL_CACHE)
        .ok()
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}
