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

mod extensions;
mod extern_def;
mod file_attr;
mod function;
mod imports;
mod lexer;
