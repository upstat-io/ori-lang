//! Test runner infrastructure for Ori.
//!
//! This module provides:
//! - Test discovery: Finding test files in the codebase
//! - Test execution: Running tests with proper isolation
//! - Result tracking: Collecting pass/fail/skip counts
//! - Parallel execution: Running tests concurrently with rayon

mod discovery;
mod error_matching;
mod result;
mod runner;

pub use discovery::{discover_tests, TestFile};
pub use error_matching::{match_errors, matches_expected, format_expected, format_actual, MatchResult};
pub use result::{TestResult, TestSummary, TestOutcome, CoverageReport, FunctionCoverage};
pub use runner::{run_tests, run_test_file, TestRunner, TestRunnerConfig, Backend};
