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
//! Three-way classification (`isScalar` / `isPossibleRef` / `isDefiniteRef`
//! on `IRType`) over an LCNF basic-block IR — historical influence: Lean 4's
//! three-way classification SHAPE; Ori's formulation per Spec: Annex E §AIMS.
//! Classification is **monomorphized** — it operates on concrete types after
//! type parameter substitution.
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
pub mod graph;
pub mod ir;
pub mod liveness;
pub mod lower;
pub mod ownership;
mod pipeline;
pub mod rc_insert;
pub mod tail_call;
pub mod uniqueness;
pub mod verify;

#[cfg(test)]
pub(crate) mod test_helpers;

pub use aims::contract::{ContractMapExt, MemoryContract};
pub use aims::interprocedural::{
    augment_contracts_with_impl_callees, compute_impl_method_contracts,
};
pub use aims::realize::push_receiver_lineage_returned;
pub use aims::realize::rc_remark::write_rc_remarks_header;
pub use aims::realize::rl31_disjoint::{prove_param_noalias, NoaliasProof};
pub use pipeline::{
    compute_aims_contracts, run_arc_pipeline, run_arc_pipeline_all, run_arc_pipeline_with_observer,
    run_uniqueness_analysis, CheckpointObserver,
};

pub use borrow::{
    borrowing_builtin_names, extract_callees, infer_borrow_fixed_point, infer_borrow_single,
    infer_borrows_scc, BuiltinOwnershipSets,
};
pub use classify::{ArcClassification, ArcClassifier};
pub use decision_tree::{
    DecisionTree, FlatPattern, PathInstruction, PatternMatrix, PatternRow, ScrutineePath, TestKind,
    TestValue,
};
pub use drop::{
    collect_drop_infos, compute_closure_env_drop, compute_consumer_attribution, compute_drop_info,
    drop_glue_symbol, type_drop_may_unwind, DropInfo, DropKind, DROP_GLUE_PREFIX,
};
pub use fbip::check_fbip_enforcement;
pub use graph::call_graph::CallGraph;
pub use graph::scc::{compute_sccs, topological_order, Scc};
pub use graph::{DominatorTree, PostDominatorTree};
pub use ir::validate::{
    assert_no_unresolved_bound_vars_in_params, assert_no_unresolved_idx,
    assert_no_unresolved_type_vars, UnresolvedBoundVar, UnresolvedTypeVar,
};
pub use ir::{
    compute_var_reprs, ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator,
    ArcValue, ArcVarId, ArgOwnership, CtorKind, LitValue, PrimOp, RcAtomicity, RcStrategy,
    ValueRepr,
};
pub use liveness::{
    compute_liveness, compute_refined_liveness, BlockLiveness, LiveSet, RefinedLiveness,
};
pub use lower::{lower_function_can, ArcProblem};
pub use ownership::{AnnotatedParam, AnnotatedSig, DerivedOwnership, Ownership};
pub use rc_insert::annotate_arg_ownership;
// Legacy RC insertion (insert_rc_ops_with_ownership, insert_external_invoke_cleanup)
// has been deleted — AIMS realize_rc_reuse handles RC emission.
pub use uniqueness::{
    CowAnnotations, CowMode, DropHints, Uniqueness, UniquenessMap, UniquenessSummary,
};

pub use ir::collect_all_arc_functions;

/// ARC classification for a type.
///
/// Determines whether values of this type need reference counting.
/// This classification is the foundation for all ARC optimization passes.
///
/// Three-way classification (`isScalar`, `isPossibleRef`, `isDefiniteRef`)
/// over the IR type representation.
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

#[cfg(test)]
mod tests;
