//! Canonical IR — sugar-free, type-annotated intermediate representation.
//!
//! The canonical IR (`CanExpr`) sits between the type checker and both backends.
//! It is a **distinct type** from `ExprKind` — sugar variants cannot be represented,
//! enforced at the type level. Both `ori_eval` (interpreter) and `ori_arc` (ARC/LLVM
//! codegen) consume `CanExpr` exclusively after migration.
//!
//! # Architecture
//!
//! ```text
//! Source → Lex → Parse → Type Check → Canonicalize ─┬─→ ori_eval  (interprets CanExpr)
//!                                       (ori_canon)   └─→ ori_arc   (lowers CanExpr → ARC IR)
//! ```
//!
//! # Prior Art
//!
//! - **Roc**: `ast::Expr` → `can::Expr` → `mono::Expr` — both dev and LLVM backends
//!   consume the same mono IR. Zero parse-AST dispatch in codegen.
//! - **Elm**: `Source` → `Canonical` → `Optimized` → JS — decision trees baked into
//!   the `Optimized` form, backends never see raw patterns.
//!
//! # What's Different from `ExprKind`
//!
//! - No `CallNamed` / `MethodCallNamed` — desugared to positional `Call` / `MethodCall`
//! - No `TemplateLiteral` / `TemplateFull` — desugared to string concatenation chains
//! - No `ListWithSpread` / `MapWithSpread` / `StructWithSpread` — desugared to method calls
//! - Added `Constant(ConstantId)` — compile-time-folded values
//! - Added `DecisionTreeId` on `Match` — patterns pre-compiled to decision trees
//! - Uses `CanId` / `CanRange` (not `ExprId` / `ExprRange`) — distinct index space

mod arena;
mod expr;
pub mod hash;
mod ids;
mod patterns;
mod pools;
pub mod tree;

pub use arena::*;
pub use expr::*;
pub use ids::*;
pub use patterns::*;
pub use pools::*;
pub use tree::{
    DecisionTree, FlatPattern, PathInstruction, PatternMatrix, PatternRow, ScrutineePath, TestKind,
    TestValue,
};

#[cfg(test)]
mod tests;
