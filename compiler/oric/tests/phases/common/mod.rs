//! Shared test utilities for phase tests.
//!
//! This module provides helper functions for setting up and running tests
//! at each compiler phase. Each submodule provides utilities for a specific phase.

mod parse;
mod typecheck;

// Diagnostic system tests
mod diagnostics;

// AST visitor tests
mod visitor;

// Error matching tests (compile_fail infrastructure)
mod error_matching;

// Feature-gated modules
// mod eval;     // TODO: Enable when eval tests are added
// mod codegen;  // TODO: Enable when codegen tests are added (requires llvm feature)

pub use parse::*;
#[allow(
    unused_imports,
    reason = "re-exported for future test modules (see README.md)"
)]
pub use typecheck::*;
// pub use eval::*;
// pub use codegen::*;
