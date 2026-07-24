//! Common canonical-IR types consumed by lowering and execution backends.

pub use super::arena::*;
pub use super::expr::*;
pub use super::ids::*;
pub use super::patterns::*;
pub use super::pools::*;
pub use super::result::*;
pub use super::support::*;
pub use super::tree::{
    DecisionTree, FlatPattern, LeafDiscardPaths, PathInstruction, PatternMatrix, PatternRow,
    ScrutineePath, TestKind, TestValue,
};
