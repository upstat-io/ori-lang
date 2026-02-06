//! Parser phase tests.
//!
//! Tests for the `ori_lexer` and `ori_parse` crates, validating:
//! - Tokenization edge cases
//! - Parser grammar coverage
//! - Error recovery
//! - AST structure
//! - Span correctness
//!
//! # Test Organization
//!
//! - `lexer` - Token recognition, escape sequences, comment handling

mod function;
mod lexer;
