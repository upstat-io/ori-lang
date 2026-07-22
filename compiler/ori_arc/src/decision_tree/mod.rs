//! Pattern match decision trees — ARC IR emission.
//!
//! Emits a pre-compiled `DecisionTree` (built by `ori_canon`) as ARC IR
//! `Switch` terminators that map trivially to LLVM `switch` instructions.
//!
//! # Algorithm
//!
//! Follows Maranget (2008) "Compiling Pattern Matching to Good Decision Trees",
//! as implemented in Roc and Elm. Operates on a **pattern matrix** where rows
//! are match arms and columns are sub-patterns at each scrutinee position.
//!
//! # Architecture
//!
//! `ori_ir::canon::tree` defines the shared tree types,
//! `ori_canon::patterns::decision_tree` compiles the pattern matrix, and this
//! module emits the resulting ARC IR.
//!
//! # References
//!
//! - Maranget (2008): foundational algorithm
//! - Decision-tree pattern compilation lowering each constructor test to a
//!   tag/discriminant switch

pub(crate) mod emit;
mod emit_switches;

// Re-export decision tree types from ori_ir (the shared types crate).
// These types were relocated to ori_ir::canon::tree so that both ori_canon
// (builds them during canonicalization) and ori_arc (emits them as ARC IR)
// can depend on the same definitions without circular dependencies.
pub use ori_ir::canon::tree::{
    DecisionTree, FlatPattern, PathInstruction, PatternMatrix, PatternRow, ScrutineePath, TestKind,
    TestValue,
};
