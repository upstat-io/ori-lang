//! ARC IR — backend-neutral basic-block carrier for AIMS analysis.
//!
//! The ARC IR carries logical ownership, lifetime, cleanup, and reuse facts
//! for physical realization.

pub mod format;
pub mod validate;

mod function;
mod function_cache;
mod function_data;
mod instr;
mod primitive;
mod repr;
mod terminator;
mod types;

pub use function_cache::collect_all_arc_functions;
pub use function_data::{
    AllocationSiteId, ArcFunction, DirectCallFact, MethodCallFact, MethodCallForm,
    OperatorCallFact, VariableMetadataState, YieldAllocationFact, YieldAllocationLocality,
    YieldExtent,
};
pub use instr::ArcInstr;
pub use primitive::{PrimitiveFact, PrimitiveFacts};
pub use repr::{
    compute_var_rc_strategies, compute_var_reprs, is_transitive_drop_strategy, RcAtomicity,
    RcStrategy, ValueRepr,
};
pub use types::{
    ArcBlock, ArcBlockId, ArcParam, ArcTerminator, ArcValue, ArcVarId, ArgOwnership, CtorKind,
    LitValue, ParamEdgeArg, PrimOp,
};

pub(crate) use repr::derive_var_rc_strategies;

#[cfg(test)]
mod tests;
