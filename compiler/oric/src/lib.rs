//! Ori Compiler - Salsa-First Incremental Compilation
//!
//! Built with Salsa as the foundation for efficient incremental compilation.
//! Every type that flows through queries has Clone, Eq, Hash, Debug.
//!
//! # Architecture
//!
//! ```text
//! SourceFile (input)
//!     │
//!     ▼
//! tokens() ──► TokenList
//!     │
//!     ▼
//! parsed_module() ──► ParsedModule
//!     │
//!     ▼
//! typed_module() ──► TypedModule
//! ```
//!
//! Each arrow is a Salsa query with automatic caching and invalidation.

// EvalError is a fundamental error type - boxing would add complexity across the crate
#![allow(clippy::result_large_err)]
// Arc is needed for sharing captures across closures in the evaluator
#![allow(clippy::disallowed_types)]

// Allow modules to use `oric::` paths for consistency with external consumers
extern crate self as oric;

/// Compile-time assertion that a type has a specific size.
///
/// Used to prevent accidental size regressions in frequently-allocated types.
/// If the size changes, compilation will fail with a clear error message.
///
/// # Example
///
/// ```text
/// static_assert_size!(Span, 8);
/// static_assert_size!(Token, 16);
/// ```
///
/// # Note
///
/// Size assertions are platform-specific. Use `#[cfg(target_pointer_width = "64")]`
/// to limit assertions to 64-bit platforms where sizes may differ from 32-bit.
#[macro_export]
macro_rules! static_assert_size {
    ($ty:ty, $size:expr) => {
        const _: [(); $size] = [(); ::std::mem::size_of::<$ty>()];
    };
}

pub mod commands;
pub mod db;
pub mod imports;
pub mod input;
pub mod ir;
pub mod parser;
pub mod query;

// Re-export lex function from the ori_lexer crate (single source of truth)
pub use ori_lexer::lex;
pub mod diagnostic;
pub mod edit;
pub mod eval;
pub mod problem;
pub mod reporting;
pub mod test;
pub mod testing;
pub mod tracing_setup;
pub mod typeck;

// Re-exports: only types actually consumed by external crates (ori-lsp, benches)
// and by internal modules via `oric::` paths. IR types are accessed internally
// via `crate::ir::` — re-exporting them here would expose internal phase types
// through the crate's public boundary.
pub use db::{CompilerDb, Db};
pub use eval::{EvalOutput, ModuleEvalResult};
pub use input::SourceFile;
pub use query::evaluated;
pub use test::{
    run_test_file, run_tests, TestOutcome, TestResult, TestRunner, TestRunnerConfig, TestSummary,
};
