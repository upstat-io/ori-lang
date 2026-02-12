//! Ori IR - Re-exports from `ori_ir`
//!
//! This module re-exports types from the `ori_ir` crate that are used
//! within `oric`. The `ori_ir` crate is the single source of truth for
//! IR types; this module provides the subset needed by the compiler driver.

// AST node types
pub use ori_ir::{
    ArmRange, BinaryOp, BindingPattern, CallArg, CallArgRange, DurationUnit, ExpectedError, Expr,
    ExprKind, FieldInit, FieldInitRange, Function, FunctionExp, FunctionExpKind, FunctionSeq,
    ImportPath, MapEntry, MapEntryRange, MatchArm, MatchPattern, Module, NamedExpr, NamedExprRange,
    Param, ParamRange, SeqBinding, SeqBindingRange, SizeUnit, Stmt, StmtKind, TemplatePart,
    TemplatePartRange, TestDef, Token, TokenKind, TokenList, UnaryOp, UseDef, Visibility,
};

// Arena and ID types
pub use ori_ir::{ExprArena, ExprId, ExprRange, SharedArena, StmtId, StmtRange, TypeId};

// Name interning
pub use ori_ir::{Name, SharedInterner, StringInterner};

// Span and traits
pub use ori_ir::{Named, Span, Spanned, Typed};
