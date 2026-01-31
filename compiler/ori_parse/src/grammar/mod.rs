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
//! - **Progress tracking**: `ParseResult<T>` distinguishes "made progress" from "no progress"
//! - **Context flags**: `ParseContext` controls context-sensitive parsing
//! - **Error recovery**: Failed parses can recover at synchronization points
//!
//! Grammar productions map directly to the formal EBNF in
//! `docs/ori_lang/0.1-alpha/spec/grammar.ebnf`.

mod attr;
mod expr;
mod item;
mod ty;

pub use attr::ParsedAttrs;
