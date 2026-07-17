//! ARC analysis for the Ori compiler.
//!
//! This crate provides:
//!
//! - **Type classification** ([`ArcClass`]) — every type is classified by its
//!   managed logical ownership/drop obligation. This does not prescribe heap
//!   allocation, reference counting, or any other physical representation.
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
//!     5. realize_rc_reuse()        — Phase 1: ownership events + reuse (pre-merge)
//!     6. verify()                  — ARC IR sanity check
//!     7. detect/rewrite tail calls — CFG optimization
//!     8. merge_blocks()            — CFG cleanup
//!     9. realize_annotations()     — Phase 2: COW + drop hints (post-merge)
//!    10. verify()                  — Final sanity check
//!    11. FBIP enforcement          — Read-only diagnostic
//! ```
//!
//! Production entry: [`realize_closed_program()`]. The current `RcInc` and
//! `RcDec` instruction names are transitional spellings for backend-neutral
//! logical ownership events; physical backends choose their realization.

pub mod aims;
mod block_merge;
pub mod borrow;
pub(crate) mod classify;
mod closure_abi;
pub mod decision_tree;
mod derived_body;
mod derived_compare;
mod derived_format;
mod derived_hash;
pub mod drop;
pub mod fbip;
pub mod graph;
pub mod ir;
mod lambda_specialization;
pub mod liveness;
pub mod lower;
mod operator_calls;
pub mod ownership;
mod pipeline;
pub mod rc_insert;
pub mod tail_call;
pub mod uniqueness;
pub mod verify;

#[cfg(test)]
pub(crate) mod test_helpers;

pub use aims::contract::{
    CalleeOwnerDemand, CalleeOwnerDemandConflict, ContractMapExt, EffectSummary,
    FreshSelfAllocationFacts, FunctionEffectFacts, MemoryAccessClass, MemoryContract,
};
pub use aims::realize::push_receiver_lineage_returned;
pub use aims::realize::rc_remark::write_rc_remarks_header;
pub use aims::realize::rl31_disjoint::{prove_param_disjointness, ParamDisjointnessFacts};
pub use aims::{freeze_primitive_facts, validate_primitive_facts};
pub use pipeline::{
    realize_closed_program, realize_closed_program_with_observer, ArcPipelineBatchOutcome,
    ArcPipelineContext, CallableBoundaryError, CallableBoundaryFacts, CheckpointObserver,
};

pub use borrow::{
    borrowing_builtin_names, extract_callees, infer_borrow_fixed_point, infer_borrow_single,
    infer_borrows_scc, BuiltinOwnershipSets,
};
pub use classify::{ArcClassification, ArcClassifier};
pub use closure_abi::{
    freeze_closure_adapter_plans, freeze_function_callable_facts, ClosureAbiError,
    ClosureAdapterAction, ClosureAdapterPlan, ClosureAdapterSlot, ClosureAdapterSource,
    ClosureValueSignature, FrozenClosureAdapters, FunctionCallableFacts, RetainPlanEdge,
    RetainPlanId, RetainPlanKind, RetainPlanNode, RetainPlanTable,
};
pub use decision_tree::{
    DecisionTree, FlatPattern, PathInstruction, PatternMatrix, PatternRow, ScrutineePath, TestKind,
    TestValue,
};
pub use derived_body::{
    build_derived_clone_identity, build_derived_default, build_derived_eq, DerivedCloneBodyError,
    DerivedDefaultBodyError, DerivedEqBodyError,
};
pub use derived_compare::{build_derived_compare, DerivedCompareBodyError};
pub use derived_format::{build_derived_format, DerivedFormatBodyError};
pub use derived_hash::{build_derived_hash, DerivedHashBodyError};
pub use drop::{
    collect_drop_infos, compute_closure_env_drop, compute_consumer_attribution, compute_drop_info,
    type_drop_may_unwind, DropInfo, DropKind,
};
pub use fbip::check_fbip_enforcement;
pub use graph::call_graph::CallGraph;
pub use graph::scc::{compute_sccs, topological_order, Scc};
pub use graph::{DominatorTree, PostDominatorTree};
pub use ir::validate::{
    assert_no_unresolved_bound_vars, assert_no_unresolved_idx, assert_no_unresolved_type_vars,
    UnresolvedBoundVar, UnresolvedTypeVar,
};
pub use ir::{
    compute_var_reprs, ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator,
    ArcValue, ArcVarId, ArgOwnership, CtorKind, DirectCallFact, LitValue, MethodCallFact,
    MethodCallForm, PrimOp, RcAtomicity, RcStrategy, ValueRepr, VariableMetadataState,
};
pub use lambda_specialization::{
    first_unresolved_bound_var, specialize_polymorphic_lambdas, type_contains_bound_var,
    LambdaSpecializationError,
};
pub use liveness::{
    compute_liveness, compute_refined_liveness, BlockLiveness, LiveSet, RefinedLiveness,
};
pub use lower::{lower_function_can, ArcProblem};
pub use operator_calls::{rewrite_operator_trait_calls, OperatorCallResolutionError};
pub use ownership::{AnnotatedParam, AnnotatedSig, DerivedOwnership, Ownership};
pub use rc_insert::annotate_arg_ownership;
// Legacy ad hoc RC insertion (insert_rc_ops_with_ownership,
// insert_external_invoke_cleanup) has been deleted. AIMS freezes the logical
// event plan; each validated physical projection chooses its own mechanism.
pub use uniqueness::{CowAnnotations, CowMode, DropHints, Uniqueness};

pub use ir::collect_all_arc_functions;

/// ARC classification for a type.
///
/// Determines whether values of this type carry a managed logical
/// ownership/drop obligation. Physical storage and lifetime mechanisms remain
/// projection decisions.
///
/// Three-way classification (`isScalar`, `isPossibleRef`, `isDefiniteRef`)
/// over the IR type representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArcClass {
    /// No managed logical ownership/drop obligation. Physical storage remains
    /// a representation-plan decision.
    Scalar,

    /// Definitely participates in managed logical ownership/drop. This does
    /// not imply a reference-counted pointer or heap allocation.
    DefiniteRef,

    /// Might carry a managed logical ownership/drop obligation depending on
    /// unresolved type variables. Conservatively treated as managed until
    /// representation is known.
    PossibleRef,
}

#[cfg(test)]
mod tests;
