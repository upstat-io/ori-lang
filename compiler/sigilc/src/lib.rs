//! Sigil V3 Compiler - Salsa-First Incremental Compilation
//!
//! V3 is built with Salsa as the foundation, not an afterthought.
//! Every type that flows through queries has Clone, Eq, Hash, Debug.
//!
//! # Architecture
//!
//! ```text
//! SourceFile (input)
//!     │
//!     ▼
//! tokens() ──► TokenList
//!     │
//!     ▼
//! parsed_module() ──► ParsedModule
//!     │
//!     ▼
//! typed_module() ──► TypedModule
//! ```
//!
//! Each arrow is a Salsa query with automatic caching and invalidation.

/// Compile-time assertion that a type has a specific size.
///
/// Used to prevent accidental size regressions in frequently-allocated types.
/// If the size changes, compilation will fail with a clear error message.
///
/// # Example
///
/// ```ignore
/// static_assert_size!(Span, 8);
/// static_assert_size!(Token, 16);
/// ```
///
/// # Note
///
/// Size assertions are platform-specific. Use `#[cfg(target_pointer_width = "64")]`
/// to limit assertions to 64-bit platforms where sizes may differ from 32-bit.
#[macro_export]
macro_rules! static_assert_size {
    ($ty:ty, $size:expr) => {
        const _: [(); $size] = [(); ::std::mem::size_of::<$ty>()];
    };
}

pub mod db;
pub mod input;
pub mod query;
pub mod ir;
pub mod parser;

// Re-export lex function from the sigil_lexer crate (single source of truth)
pub use sigil_lexer::lex;
pub mod diagnostic;
pub mod edit;
pub mod problem;
pub mod reporting;
pub mod suggest;
pub mod types;
pub mod typeck;
pub mod eval;
pub mod test;
pub mod patterns;
pub mod context;
pub mod testing;
pub mod debug;
pub mod stack;

// Re-exports for convenience
pub use db::{Db, CompilerDb};
pub use input::SourceFile;
pub use ir::{
    Name, Span, Token, TokenKind, TokenList, DurationUnit, SizeUnit,
    StringInterner, SharedInterner,
    TypeId, ExprId, ExprRange, StmtId, StmtRange,
    Spanned, Named, Typed,
    Expr, ExprKind, Stmt, StmtKind, Param, ParamRange,
    BinaryOp, UnaryOp, Function, Module, TestDef,
    ExprArena,
    BindingPattern, MatchPattern, MatchArm,
    MapEntry, FieldInit,
    ArmRange, MapEntryRange, FieldInitRange,
    // function_seq types
    SeqBinding, SeqBindingRange, FunctionSeq,
    // function_exp types
    NamedExpr, NamedExprRange, FunctionExpKind, FunctionExp,
    // CallNamed types
    CallArg, CallArgRange,
};
pub use diagnostic::{Diagnostic, ErrorCode, Severity, Label};
pub use types::{Type, TypeVar, TypeScheme, TypeEnv, InferenceContext, TypeError};
pub use typeck::{TypedModule, TypeChecker, TypeCheckError, type_check, type_check_with_context};
pub use eval::{Value, FunctionValue, RangeValue, Environment, Evaluator, EvalResult, EvalError, EvalOutput, ModuleEvalResult};
pub use query::evaluated;
pub use test::{TestRunner, TestRunnerConfig, TestSummary, TestResult, TestOutcome, run_tests, run_test_file};
pub use patterns::{PatternRegistry, PatternDefinition, TypeCheckContext, EvalContext};
pub use context::{CompilerContext, SharedContext, shared_context};
