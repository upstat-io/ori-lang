//! Canonical IR lowering for the Ori compiler.
//!
//! This crate transforms the type-checked AST (`ExprArena` + `TypeCheckResult`)
//! into canonical form (`CanArena` + `CanonResult`). The canonical IR is
//! sugar-free, type-annotated, and has pre-compiled decision trees.
//!
//! # Pipeline Position
//!
//! ```text
//! Source → Lex → Parse → Type Check → **Canonicalize** → ori_eval / ori_arc
//! ```
//!
//! # What Happens During Lowering
//!
//! 1. **Desugaring** (`desugar`): 7 sugar variants eliminated
//!    - `CallNamed` → positional `Call`
//!    - `MethodCallNamed` → positional `MethodCall`
//!    - `TemplateLiteral` → string concatenation chain
//!    - `ListWithSpread` / `MapWithSpread` / `StructWithSpread` → method calls
//!
//! 2. **Pattern Compilation** (`patterns`): Match patterns → decision trees
//!    via Maranget (2008) algorithm
//!
//! 3. **Constant Folding** (`const_fold`): Compile-time-known expressions
//!    pre-evaluated and stored in `ConstantPool`
//!
//! 4. **Type Attachment**: Every `CanNode` carries its resolved type
//!
//! # Prior Art
//!
//! - **Roc**: `canonicalize_expr()` in `crates/compiler/can/src/expr.rs`
//! - **Elm**: `canonicalize` in `compiler/src/Canonicalize/Expression.hs`

mod const_fold;
mod desugar;
mod lower;
mod patterns;
mod validate;

pub use lower::lower;
pub use validate::validate;

// Re-export canonical IR types for convenience.
pub use ori_ir::canon::{
    CanArena, CanExpr, CanField, CanFieldRange, CanId, CanMapEntry, CanMapEntryRange, CanNode,
    CanRange, CanonResult, ConstValue, ConstantId, ConstantPool, DecisionTreeId, DecisionTreePool,
};
