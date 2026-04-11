//! Shared test harness for AIMS snapshot tests and `FileCheck` IR assertions.
//!
//! Provides directive parsing, artifact naming, bless mode, revision expansion,
//! diff generation, and a canonical test runner loop. Consumed by `ori_arc`
//! (AIMS snapshots) and `ori_llvm` (`FileCheck` IR tests) as a dev-dependency.
//!
//! **Design principle**: this crate knows nothing about the Ori compiler.
//! It parses directives from text, names artifacts, diffs strings, and
//! orchestrates a test loop via the `TestStrategy` trait. Compiler-specific
//! behavior (compilation, IR capture, flag translation) lives in consumer
//! crates' `TestStrategy` implementations.

pub mod artifact;
pub mod bless;
pub mod check;
pub mod diff;
pub mod directive;
pub mod revision;
pub mod runner;
