//! Low-level tokenizer for the Ori programming language.
//!
//! This crate provides a standalone, pure tokenizer with **zero `ori_*` dependencies**.
//! It is designed to be reusable by external tools (LSP, formatter, syntax highlighter)
//! without pulling in the entire Ori compiler.
//!
//! # Architecture
//!
//! `ori_lexer_core` is the "raw" half of Ori's two-layer lexer architecture
//! (modeled after Rust's `rustc_lexer` / `rustc_parse::lexer` separation):
//!
//! - **`ori_lexer_core`** (this crate): Produces `(RawTag, len)` pairs from raw bytes.
//!   No spans, no interning, no diagnostics.
//! - **`ori_lexer`**: "Cooks" raw tokens into compiler-ready form with spans,
//!   interning, keyword resolution, and diagnostics.
//!
//! # Usage
//!
//! ```
//! use ori_lexer_core::{SourceBuffer, RawTag};
//!
//! let buf = SourceBuffer::new("let x = 42");
//! let cursor = buf.cursor();
//!
//! // The sentinel byte is accessible but not part of the source
//! assert_eq!(buf.len(), 10);
//! assert!(buf.encoding_issues().is_empty());
//! ```
//!
//! # Stability
//!
//! - `RawTag` enum: Variants may be added (`#[non_exhaustive]`)
//! - `RawToken` struct: Fields are stable
//! - `SourceBuffer` / `Cursor`: API is stable
//! - Error tags: May be refined (new error kinds)

mod cursor;
mod raw_scanner;
mod source_buffer;
mod tag;

pub use cursor::Cursor;
pub use raw_scanner::{tokenize, RawScanner};
pub use source_buffer::{EncodingIssue, EncodingIssueKind, SourceBuffer};
pub use tag::{RawTag, RawToken};
