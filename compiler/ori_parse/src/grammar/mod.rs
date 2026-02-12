//! Grammar Modules
//!
//! Parsing implementations for Ori syntax constructs.
//!
//! # Organization
//!
//! Each module extends `Parser` with methods for specific grammar productions:
//!
//! - [`attr`]: Attribute parsing (`#derive`, `#test`, `#skip`, etc.)
//! - [`expr`]: Expression parsing (literals, operators, calls, control flow)
//! - [`item`]: Module-level item parsing (functions, types, traits, impls)
//! - [`ty`]: Type expression parsing (primitives, generics, function types)
//!
//! # Design
//!
//! The parser uses recursive descent with these patterns:
//!
//! - **Progress tracking**: `ParseOutcome<T>` distinguishes consumed/empty and success/failure
//! - **Context flags**: `ParseContext` controls context-sensitive parsing
//! - **Error recovery**: Failed parses can recover at synchronization points
//!
//! Grammar productions map directly to the formal EBNF in
//! `docs/ori_lang/0.1-alpha/spec/grammar.ebnf`.
//!
//! # Return Type Conventions
//!
//! Grammar functions use four distinct return type patterns. Each serves a
//! specific purpose — when adding new grammar functions, choose the pattern
//! that matches the function's role:
//!
//! - **`ParseOutcome<T>`** — Grammar entry points that may be tried as
//!   alternatives. Carries progress information (`ConsumedOk`/`EmptyErr`) so
//!   the caller knows whether to backtrack. Used by: expression parsing, item
//!   entry points, generics, bounds. Bridge to `Result` with `committed!`.
//!
//! - **`Result<T, ParseError>`** — Internal helpers called after the caller
//!   has committed (consumed at least one token). Backtracking is no longer
//!   possible, so progress tracking is unnecessary. Used by: parameter parsing,
//!   type-decl internals, trait items, impl methods, patterns, postfix chains.
//!   Bridge to `ParseOutcome` from Result with the `committed!` macro.
//!
//! - **`Option<ParsedType>`** — Lightweight "try" for type parsing. Returns
//!   `None` when no type is present; the caller decides whether absence is an
//!   error. Used by: `parse_type()` in `ty.rs`.
//!
//! - **`ParsedAttrs` + `&mut Vec<ParseError>`** — Attribute parsing accumulates
//!   errors into a caller-provided Vec while always producing a `ParsedAttrs`
//!   value (even when malformed). This is intentional: the parser needs to know
//!   what attributes were attempted regardless of validity. Used by:
//!   `parse_attributes()` in `attr.rs`.

mod attr;
mod expr;
mod item;
mod ty;

pub use attr::ParsedAttrs;
