//! Structured Problem Types
//!
//! This module separates problem definitions from rendering. Problems describe
//! what went wrong in a structured way, while rendering converts them to
//! user-facing diagnostics.
//!
//! The 1:1 coupling with [`super::reporting`] is intentional: each problem type
//! has a corresponding renderer. `problem` owns the *data*, `reporting` owns
//! the *presentation*. This separation keeps error descriptions independent of
//! output format while guaranteeing every problem has a rendering.
//!
//! # Design
//!
//! Problems are categorized by compilation phase:
//! - `LexProblem`: Lex-time warnings (detached doc comments)
//! - `SemanticProblem`: Semantic analysis errors (name resolution, patterns, etc.)
//!
//! Lex errors use `render_lex_error()` directly from `&LexError` references.
//! Parse errors are rendered directly by `ori_parse::ParseError::to_queued_diagnostic()`
//! and do not flow through this module.
//! Type errors use `TypeErrorRenderer` for Pool-aware rendering.
//!
//! Each problem type carries all the data needed to render a helpful error
//! message, including spans, types, and context.

pub mod eval;
pub mod lex;
pub mod semantic;

#[cfg(feature = "llvm")]
pub mod codegen;
#[cfg(feature = "llvm")]
pub use codegen::{emit_codegen_error, report_codegen_error, CodegenProblem};

pub use eval::eval_error_to_diagnostic;
pub use lex::LexProblem;
pub use semantic::SemanticProblem;
