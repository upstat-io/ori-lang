//! Ori IR - Re-exports from `ori_ir`
//!
//! This module re-exports the subset of `ori_ir` types consumed within `oric`.
//! The `ori_ir` crate is the single source of truth for IR types; only types
//! actually imported via `crate::ir::` appear here. Types used in fewer than
//! two modules (or only in tests) may import `ori_ir` directly.
//!
//! Canon IR types (`ori_ir::canon::*`) are NOT re-exported here — they form
//! a separate sub-API consumed directly by modules that need canonicalization.

// AST node types consumed by oric modules
pub use ori_ir::{
    BinaryOp, ExpectedError, ExprKind, Function, ImportPath, Module, TestDef, TokenKind, TokenList,
    UseDef,
};

// Arena types
pub use ori_ir::{ExprArena, SharedArena};

// Name interning
pub use ori_ir::{Name, SharedInterner, StringInterner};

// Span
pub use ori_ir::Span;
