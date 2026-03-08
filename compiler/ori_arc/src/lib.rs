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
//! # Pipeline (canonical pass ordering)
//!
//! ```text
//! CanExpr → lower → ArcFunction
//!   → borrow inference (Owned/Borrowed per param)       [interprocedural]
//!   → uniqueness analysis (Unique/MaybeShared/Shared)    [interprocedural]
//!   → derived ownership (all locals)                     [per-function]
//!   → liveness + refined liveness
//!   → COW annotation (CowMode per COW operation)
//!   → RC insertion (RcInc/RcDec)
//!   → reset/reuse detection → expansion
//!   → RC identity propagation (Project roots)
//!   → RC elimination (intra-block + cross-block, dataflow-based)
//!   → tail call detection + loop lowering (self-recursive → loops)
//!   → block merge (post-lowering CFG simplification)
//!   → drop hints (unique-collection drop optimization)
//!   → FBIP enforcement (#fbip functions)
//! ```
//!
//! Entry: [`run_arc_pipeline()`] (single function),
//! [`run_arc_pipeline_all()`] (batch with borrow application).
//! This is the **sole codegen path** — Tier 1 (`ExprLowerer`) was removed.
//!
//! # Crate Dependencies
//!
//! `ori_arc` depends on `ori_types` (for `Pool`/`Idx`/`Tag`) and `ori_ir`
//! (for `Name`, `BinaryOp`, `UnaryOp`, etc.). No LLVM dependency — ARC
//! analysis is backend-independent.

mod block_merge;
pub mod borrow;
pub(crate) mod classify;
pub mod decision_tree;
pub mod drop;
pub mod expand_reuse;
pub mod fbip;
mod graph;
pub mod ir;
pub mod liveness;
pub mod lower;
pub mod ownership;
mod pipeline;
pub mod rc_elim;
pub mod rc_identity;
pub mod rc_insert;
pub mod reset_reuse;
pub mod tail_call;
pub mod uniqueness;
pub(crate) mod verify;

#[cfg(test)]
pub(crate) mod test_helpers;

use ori_types::Idx;

pub use pipeline::{run_arc_pipeline, run_arc_pipeline_all, run_uniqueness_analysis};

pub use borrow::{
    all_cow_method_names, apply_borrows, borrowing_builtin_names, consuming_receiver_builtin_names,
    consuming_receiver_only_builtin_names, extract_callees, infer_borrow_fixed_point,
    infer_borrow_single, infer_borrows_scc, infer_derived_ownership, BuiltinOwnershipSets,
};
pub use classify::ArcClassifier;
pub use decision_tree::{
    DecisionTree, FlatPattern, PathInstruction, PatternMatrix, PatternRow, ScrutineePath, TestKind,
    TestValue,
};
pub use drop::{
    collect_drop_infos, compute_closure_env_drop, compute_drop_info, DropInfo, DropKind,
};
pub use expand_reuse::expand_reset_reuse;
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
pub use rc_elim::eliminate_rc_ops_dataflow;
pub use rc_identity::{propagate_rc_identity, RcIdentityMap};
pub use rc_insert::{
    annotate_arg_ownership, insert_external_invoke_cleanup, insert_rc_ops_with_ownership,
};
pub use uniqueness::inter::{analyze_program, build_cow_summaries};
pub use uniqueness::intra::{analyze_intraprocedural, analyze_with_summaries, UniquenessResult};
pub use uniqueness::{
    compute_cow_annotations, compute_drop_hints, CowAnnotations, CowMode, DropHints, Uniqueness,
    UniquenessMap, UniquenessSummary,
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
