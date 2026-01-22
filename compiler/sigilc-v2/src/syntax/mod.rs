//! Syntax module containing lexer, parser, AST, and expression arena.

mod span;
mod ids;
mod token;
mod arena;
mod expr;
mod items;
mod operators;
mod lexer;
mod parser;
mod traits;

pub use span::Span;
pub use ids::{ExprId, ExprRange, StmtRange, ArmRange, ParamRange, MapEntryRange, FieldInitRange, PatternArgsId, TypeExprId};
pub use token::{Token, TokenKind, Trivia, TriviaKind, DurationUnit, SizeUnit};
pub use arena::ExprArena;
pub use expr::{Expr, ExprKind, PatternKind, BindingPattern, Stmt, StmtKind, PatternArg, PatternArgs, MatchPattern, MatchArm, TypeExpr, TypeExprKind};
pub use items::{Item, ItemId, ItemKind, Function, TypeDef, Config, Import, ImportPath, Test, Trait, Impl};
pub use operators::{BinaryOp, UnaryOp};
pub use lexer::{Lexer, TokenList};
pub use parser::{Parser, ParseResult};
pub use traits::{Spanned, Named, find_by_name, merge_spans};

// Re-export TypeId for convenience
pub use crate::intern::TypeId;
