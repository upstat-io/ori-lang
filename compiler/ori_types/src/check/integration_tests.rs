#![expect(
    clippy::expect_used,
    reason = "integration fixtures abort when parsing or required registry lookup fails"
)]
//! Integration tests for the module checker.
//!
//! These tests feed real Ori source code through the full pipeline:
//! lexer → parser → type checker, verifying the end-to-end behavior.
//!
//! # Test Categories
//!
//! - **Literals**: Basic literal expressions in function bodies
//! - **Parameters**: Typed function parameters
//! - **Multi-function**: Forward references, mutual recursion
//! - **Tests**: `@test` declarations
//! - **Type errors**: Mismatches, unknown identifiers
//! - **Let bindings**: Local variable bindings
//! - **Control flow**: If/then/else expressions
//! - **Collections**: List literals
//! - **Operators**: Arithmetic, comparison, boolean
//! - **Empty module**: No declarations

#![expect(
    clippy::unwrap_used,
    reason = "integration fixtures treat unexpected type-check errors as assertion failures"
)]

mod builtin_materialization;
mod deferred_mono;
mod drop_invariants;
mod expressions;
mod generic_calls;
mod hash_first_imports;
mod imports;
mod language_basics;
mod resolution_contracts;
mod support;
mod type_declarations;
