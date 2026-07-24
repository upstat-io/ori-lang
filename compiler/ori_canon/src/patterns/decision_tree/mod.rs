//! Pattern match decision trees.
//!
//! Compiles match expressions to efficient decision trees during AST-to-Can-IR
//! canonicalization.

pub(super) mod compile;
pub(super) mod flatten;
