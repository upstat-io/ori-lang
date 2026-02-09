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
pub use error_matching::{
    format_actual, format_expected, match_errors, matches_expected, MatchResult,
};
pub use result::{CoverageReport, FunctionCoverage, TestOutcome, TestResult, TestSummary};
pub use runner::{run_test_file, run_tests, Backend, TestRunner, TestRunnerConfig};
