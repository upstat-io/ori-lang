// Test code uses unwrap/expect for clarity - panics provide good test failure messages
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Phase-based compiler tests.
//!
//! This test module organizes compiler tests by compilation phase rather than
//! by language feature. This complements the spec tests in `tests/spec/` which
//! are organized by feature.
//!
//! # Organization
//!
//! - `parse/` - Lexer and parser tests (`ori_lexer`, `ori_parse`)
//! - `eval/` - Interpreter tests (`ori_eval`, `ori_patterns`)
//! - `codegen/` - LLVM backend tests (`ori_llvm`) [requires `llvm` feature]
//! - `common/` - Shared test utilities
//!
//! # When to Add Tests Here
//!
//! Add tests to `tests/phases/` when:
//! - Testing internal compiler behavior (not user-facing features)
//! - Inline test module would exceed 200 lines
//! - Test needs access to multiple compiler internals
//!
//! Add tests to `tests/spec/` when:
//! - Testing user-facing language behavior
//! - Test should run on both interpreter and LLVM backends
//!
//! # Running Phase Tests
//!
//! ```bash
//! # Run all phase tests
//! cargo test -p oric --test phases
//!
//! # Run specific phase
//! cargo test -p oric --test phases parse
//!
//! # Run with LLVM codegen tests (requires llvm feature)
//! cargo test -p oric --test phases --features llvm
//! ```

// Phase test modules
#[path = "phases/common/mod.rs"]
mod common;

#[path = "phases/parse/mod.rs"]
mod parse;

#[path = "phases/eval/mod.rs"]
mod eval;

// Codegen tests (most require LLVM feature, runtime_lib does not)
#[path = "phases/codegen/mod.rs"]
mod codegen;
