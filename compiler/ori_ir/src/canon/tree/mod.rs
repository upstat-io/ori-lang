//! Maranget-style decision-tree model.

mod model;

pub use model::{
    DecisionTree, FlatPattern, LeafDiscardPaths, PathInstruction, PatternMatrix, PatternRow,
    ScrutineePath, TestKind, TestValue,
};

#[cfg(test)]
mod tests;
