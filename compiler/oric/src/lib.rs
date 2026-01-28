//! Ori Compiler - Salsa-First Incremental Compilation
//!
//! Built with Salsa as the foundation for efficient incremental compilation.
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
/// ```text
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
pub mod ir;
pub mod parser;
pub mod query;

// Re-export lex function from the ori_lexer crate (single source of truth)
pub use ori_lexer::lex;
pub mod context;
pub mod debug;
pub mod diagnostic;
pub mod edit;
pub mod eval;
pub mod problem;
pub mod reporting;
pub mod stack;
pub mod suggest;
pub mod test;
pub mod testing;
pub mod typeck;
pub mod types;

// Re-exports for convenience
pub use context::{shared_context, CompilerContext, SharedContext};
pub use db::{CompilerDb, Db};
pub use diagnostic::{Diagnostic, ErrorCode, Label, Severity};
pub use eval::{
    Environment, EvalError, EvalOutput, EvalResult, Evaluator, FunctionValue, ModuleEvalResult,
    RangeValue, Value,
};
pub use input::SourceFile;
pub use ir::{
    ArmRange,
    BinaryOp,
    BindingPattern,
    // CallNamed types
    CallArg,
    CallArgRange,
    DurationUnit,
    Expr,
    ExprArena,
    ExprId,
    ExprKind,
    ExprRange,
    FieldInit,
    FieldInitRange,
    Function,
    FunctionExp,
    FunctionExpKind,
    FunctionSeq,
    MapEntry,
    MapEntryRange,
    MatchArm,
    MatchPattern,
    Module,
    Name,
    Named,
    // function_exp types
    NamedExpr,
    NamedExprRange,
    Param,
    ParamRange,
    // function_seq types
    SeqBinding,
    SeqBindingRange,
    SharedInterner,
    SizeUnit,
    Span,
    Spanned,
    Stmt,
    StmtId,
    StmtKind,
    StmtRange,
    StringInterner,
    TestDef,
    Token,
    TokenKind,
    TokenList,
    TypeId,
    Typed,
    UnaryOp,
};
pub use ori_patterns::{EvalContext, PatternDefinition, PatternRegistry, TypeCheckContext};
pub use query::evaluated;
pub use test::{
    run_test_file, run_tests, TestOutcome, TestResult, TestRunner, TestRunnerConfig, TestSummary,
};
pub use typeck::{type_check, type_check_with_context, TypeCheckError, TypeChecker, TypedModule};
pub use types::{InferenceContext, Type, TypeEnv, TypeError, TypeScheme, TypeVar};
