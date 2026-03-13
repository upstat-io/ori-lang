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
//!   a basic-block intermediate representation that ARC analysis passes
//!   operate on.
//!
//! - **Ownership annotations** ([`Ownership`], [`DerivedOwnership`],
//!   [`AnnotatedParam`], [`AnnotatedSig`]) —
//!   borrow inference output that drives ABI decisions.
//!
//! # Design
//!
//! Inspired by Lean 4's three-way classification (`isScalar`/`isPossibleRef`/
//! `isDefiniteRef` on `IRType`) and LCNF basic-block IR. Classification is
//! **monomorphized** — it operates on concrete types after type parameter
//! substitution.
//!
//! # Pipeline (AIMS unified lattice)
//!
//! ```text
//! CanExpr → lower → ArcFunction
//!   Interprocedural (once):
//!     1. analyze_program()         — MemoryContract per function (SCC fixpoint)
//!     2. apply_ownership()         — Populate ArcParam.ownership
//!   Per-function (steps 3–12):
//!     3. compute_var_reprs()       — ValueRepr per variable
//!     4. analyze_function()        — Backward dataflow → AimsStateMap
//!     5. realize_rc_reuse()        — Phase 1: RC + reuse (pre-merge)
//!     6. verify()                  — ARC IR sanity check
//!     7. detect/rewrite tail calls — CFG optimization
//!     8. merge_blocks()            — CFG cleanup
//!     9. realize_annotations()     — Phase 2: COW + drop hints (post-merge)
//!    10. verify()                  — Final sanity check
//!    11. FBIP enforcement          — Read-only diagnostic
//! ```
//!
//! Entry: [`run_arc_pipeline()`] (single function),
//! [`run_arc_pipeline_all()`] (batch with ownership application).

pub mod aims;
mod block_merge;
pub mod borrow;
pub(crate) mod classify;
pub mod decision_tree;
pub mod drop;
pub mod fbip;
mod graph;
pub mod ir;
pub mod liveness;
pub mod lower;
pub mod ownership;
mod pipeline;
pub mod rc_insert;
pub mod tail_call;
pub mod uniqueness;
pub(crate) mod verify;

#[cfg(test)]
pub(crate) mod test_helpers;

use ori_types::Idx;

pub use aims::contract::MemoryContract;
pub use pipeline::{
    compute_aims_contracts, run_arc_pipeline, run_arc_pipeline_all, run_uniqueness_analysis,
};

pub use borrow::{
    borrowing_builtin_names, extract_callees, infer_borrow_fixed_point, infer_borrow_single,
    infer_borrows_scc, BuiltinOwnershipSets,
};
pub use classify::ArcClassifier;
pub use decision_tree::{
    DecisionTree, FlatPattern, PathInstruction, PatternMatrix, PatternRow, ScrutineePath, TestKind,
    TestValue,
};
pub use drop::{
    collect_drop_infos, compute_closure_env_drop, compute_drop_info, DropInfo, DropKind,
};
pub use fbip::check_fbip_enforcement;
pub use graph::call_graph::CallGraph;
pub use graph::scc::{compute_sccs, topological_order, Scc};
pub use graph::{DominatorTree, PostDominatorTree};
pub use ir::{
    compute_var_reprs, ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator,
    ArcValue, ArcVarId, ArgOwnership, CtorKind, LitValue, PrimOp, RcStrategy, ValueRepr,
};
pub use liveness::{
    compute_liveness, compute_refined_liveness, BlockLiveness, LiveSet, RefinedLiveness,
};
pub use lower::{lower_function_can, ArcProblem};
pub use ownership::{AnnotatedParam, AnnotatedSig, DerivedOwnership, Ownership};
pub use rc_insert::annotate_arg_ownership;
pub use uniqueness::{
    CowAnnotations, CowMode, DropHints, Uniqueness, UniquenessMap, UniquenessSummary,
};

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
    Scalar,

    /// Definitely contains a reference-counted heap pointer.
    /// Every value of this type needs retain/release.
    DefiniteRef,

    /// Might contain a reference-counted pointer depending on unresolved
    /// type variables. Conservatively treated as needing RC.
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
