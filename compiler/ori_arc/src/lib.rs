//! ARC analysis for the Ori compiler.
//!
//! This crate provides:
//!
//! - **Type classification** ([`ArcClass`]) — every type is classified as
//!   [`Scalar`](ArcClass::Scalar) (no RC needed),
//!   [`DefiniteRef`](ArcClass::DefiniteRef) (always needs RC), or
//!   [`PossibleRef`](ArcClass::PossibleRef) (conservative fallback).
//!
//! - **ARC IR** ([`ArcFunction`], [`ArcBlock`], [`ArcInstr`], [`ArcTerminator`]) —
//!   a basic-block intermediate representation that all ARC analysis passes
//!   (borrow inference, RC insertion, RC elimination, constructor reuse)
//!   operate on.
//!
//! - **Ownership annotations** ([`Ownership`], [`DerivedOwnership`],
//!   [`AnnotatedParam`], [`AnnotatedSig`]) —
//!   borrow inference output that drives RC insertion decisions.
//!   [`DerivedOwnership`] extends ownership tracking to all local variables,
//!   not just function parameters.
//!
//! # Design
//!
//! Inspired by Lean 4's three-way classification (`isScalar`/`isPossibleRef`/
//! `isDefiniteRef` on `IRType`) and LCNF basic-block IR. Classification is
//! **monomorphized** — it operates on concrete types after type parameter
//! substitution. This means:
//!
//! - `option[int]` → **Scalar** (tag + int, no heap pointer)
//! - `option[str]` → **`DefiniteRef`** (contains heap-allocated string)
//! - `option[T]` where `T` is unresolved → **`PossibleRef`** (conservative)
//!
//! # Crate Dependencies
//!
//! `ori_arc` depends on `ori_types` (for `Pool`/`Idx`/`Tag`) and `ori_ir`
//! (for `Name`, `BinaryOp`, `UnaryOp`, etc.). No LLVM dependency — ARC
//! analysis is backend-independent.

pub mod borrow;
mod classify;
pub mod decision_tree;
pub mod drop;
pub mod expand_reuse;
pub mod fbip;
mod graph;
pub mod ir;
pub mod liveness;
pub mod lower;
pub mod ownership;
pub mod rc_elim;
pub mod rc_insert;
pub mod reset_reuse;

#[cfg(test)]
pub(crate) mod test_helpers;

use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::FxHashMap;

pub use borrow::{apply_borrows, infer_borrows, infer_derived_ownership};
pub use classify::ArcClassifier;
pub use decision_tree::{
    DecisionTree, FlatPattern, PathInstruction, PatternMatrix, PatternRow, ScrutineePath, TestKind,
    TestValue,
};
pub use drop::{
    collect_drop_infos, compute_closure_env_drop, compute_drop_info, DropInfo, DropKind,
};
pub use expand_reuse::expand_reset_reuse;
pub use graph::DominatorTree;
pub use ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    CtorKind, LitValue, PrimOp,
};
pub use liveness::{
    compute_liveness, compute_refined_liveness, BlockLiveness, LiveSet, RefinedLiveness,
};
pub use lower::{lower_function_can, ArcProblem};
pub use ownership::{AnnotatedParam, AnnotatedSig, DerivedOwnership, Ownership};
pub use rc_elim::eliminate_rc_ops_dataflow;
pub use rc_insert::insert_rc_ops_with_ownership;
pub use reset_reuse::detect_reset_reuse_cfg;

/// Run the full ARC optimization pipeline on a single function.
///
/// Pipeline order: ownership inference → dominator tree → refined liveness
/// (includes standard liveness) → RC insertion → reset/reuse detection →
/// expansion → RC elimination.
///
/// This is the canonical pass ordering. All consumers should call this function
/// instead of manually sequencing passes, which avoids duplicating ordering
/// knowledge across crate boundaries.
#[expect(clippy::implicit_hasher, reason = "callee functions require FxHashMap")]
pub fn run_arc_pipeline(
    func: &mut ArcFunction,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, AnnotatedSig>,
) {
    let ownership = borrow::infer_derived_ownership(func, sigs);
    let dom_tree = graph::DominatorTree::build(func);
    let (refined, liveness) = liveness::compute_refined_liveness(func, classifier);
    rc_insert::insert_rc_ops_with_ownership(func, classifier, &liveness, &ownership, sigs);
    reset_reuse::detect_reset_reuse_cfg(func, classifier, &dom_tree, &refined);
    expand_reuse::expand_reset_reuse(func, classifier);
    rc_elim::eliminate_rc_ops_dataflow(func, &ownership);
}

/// Run the full ARC pipeline on all functions, including borrow application.
///
/// This is the batch entry point for the entire ARC optimization pass:
/// 1. Apply borrow inference results to function parameters
/// 2. Run the per-function pipeline on each function
///
/// Consumers should call this instead of manually calling [`apply_borrows`]
/// followed by a per-function loop over [`run_arc_pipeline`].
#[expect(clippy::implicit_hasher, reason = "callee functions require FxHashMap")]
pub fn run_arc_pipeline_all(
    functions: &mut [ArcFunction],
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, AnnotatedSig>,
) {
    borrow::apply_borrows(functions, sigs);
    for func in functions {
        run_arc_pipeline(func, classifier, sigs);
    }
}

/// ARC classification for a type.
///
/// Determines whether values of this type need reference counting.
/// This classification is the foundation for all ARC optimization passes.
///
/// Inspired by Lean 4's three-way classification methods
/// (`isScalar`, `isPossibleRef`, `isDefiniteRef` on `IRType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArcClass {
    /// No reference counting needed. The value is purely stack/register.
    ///
    /// Examples: `int`, `float`, `bool`, `char`, `byte`, `unit`, `never`,
    /// `duration`, `size`, `ordering`, `option[int]`, `(int, float)`.
    Scalar,

    /// Definitely contains a reference-counted heap pointer.
    /// Every value of this type needs retain/release.
    ///
    /// Examples: `str`, `[T]`, `{K: V}`, `set[T]`, `chan<T>`,
    /// `(P) -> R`, `option[str]`, `(int, str)`.
    DefiniteRef,

    /// Might contain a reference-counted pointer depending on unresolved
    /// type variables. Conservatively treated as needing RC.
    ///
    /// Only appears for unresolved type variables before monomorphization.
    /// After monomorphization, every type classifies as either `Scalar` or
    /// `DefiniteRef` — encountering `PossibleRef` post-mono is a compiler bug.
    PossibleRef,
}

/// Classification trait for ARC analysis.
///
/// Provides the core `arc_class` query plus convenience predicates.
/// Implemented by [`ArcClassifier`], which wraps a `Pool` reference
/// with caching and cycle detection.
pub trait ArcClassification {
    /// Classify a type by its pool index.
    fn arc_class(&self, idx: Idx) -> ArcClass;

    /// Returns `true` if this type is scalar (no RC operations needed).
    fn is_scalar(&self, idx: Idx) -> bool {
        self.arc_class(idx) == ArcClass::Scalar
    }

    /// Returns `true` if this type might need reference counting.
    ///
    /// This is `true` for both `DefiniteRef` and `PossibleRef`.
    fn needs_rc(&self, idx: Idx) -> bool {
        self.arc_class(idx) != ArcClass::Scalar
    }
}

#[cfg(test)]
mod tests;
