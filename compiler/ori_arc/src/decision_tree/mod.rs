//! Pattern match decision trees.
//!
//! Compiles match expressions to efficient decision trees during AST-to-ARC-IR
//! lowering, producing `Switch` terminators that map trivially to LLVM `switch`
//! instructions.
//!
//! # Algorithm
//!
//! Follows Maranget (2008) "Compiling Pattern Matching to Good Decision Trees",
//! as implemented in Roc and Elm. Operates on a **pattern matrix** where rows
//! are match arms and columns are sub-patterns at each scrutinee position.
//!
//! # Architecture
//!
//! Type definitions live in `ori_ir::canon::tree` (shared across crates).
//! The compilation algorithm and ARC IR emission logic live here in `ori_arc`.
//!
//! NOTE: `decision_tree::compile` and `flatten` are pattern-matching primitives
//! temporarily housed in `ori_arc`. They are consumed by `ori_canon` (which
//! imports `ori_arc`), creating an upward dependency from canonicalization into
//! ARC optimization. These will move to `ori_canon::patterns` in `eval_v2`
//! Section 03 — see `ori_ir::canon::tree` for the migration plan.
//!
//! # References
//!
//! - Maranget (2008): foundational algorithm
//! - Roc `crates/compiler/mono/src/ir/decision_tree.rs`
//! - Elm `compiler/src/Nitpick/PatternMatches.hs`
//! - Swift: pattern compilation to SIL `switch_enum`

pub mod compile;
pub(crate) mod emit;
mod emit_switches;
pub mod flatten;

// Re-export decision tree types from ori_ir (the shared types crate).
// These types were relocated to ori_ir::canon::tree so that both ori_canon
// (builds them during canonicalization) and ori_arc (emits them as ARC IR)
// can depend on the same definitions without circular dependencies.
pub use ori_ir::canon::tree::{
    DecisionTree, FlatPattern, PathInstruction, PatternMatrix, PatternRow, ScrutineePath, TestKind,
    TestValue,
};
