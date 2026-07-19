//! Hindley-Milner inference orchestration.
//!
//! [`InferEngine`] combines pooled types, unification, expression-type storage,
//! nested environments, bidirectional checking, and contextual diagnostics.

mod accessors;
mod body_finalize;
mod context;
mod engine;
mod engine_api;
mod env;
mod expr;
mod scope;
mod state;
mod type_builders;

pub use engine::{ExprIndex, InferEngine};
pub use env::TypeEnv;
pub use expr::{
    check_expr, compose_burden_for_idx, register_resolved_collection_burdens, resolve_parsed_type,
};
pub(crate) use expr::{
    infer_expr, match_self_type, register_concrete_applied_resolutions, tag_to_type_tag,
    type_satisfies_named_trait, validate_fixed_list_capacities, NestedPathStep, RefutableReason,
    OP_TRAIT_MAP,
};
pub(crate) use scope::{LoopContext, LoopForm};

#[cfg(test)]
mod tests;
