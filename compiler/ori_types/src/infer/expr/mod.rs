//! Expression type inference.
//!
//! This module provides expression-level type inference using the
//! `InferEngine` infrastructure. It dispatches on `ExprKind` to
//! specialized inference functions.
//!
//! Expression inference follows Hindley-Milner with bidirectional checking:
//! - **Synthesis (infer)**: Bottom-up type derivation from expression structure
//! - **Checking (check)**: Top-down verification against expected type

mod bindings;
mod blocks;
mod calls;
mod checking;
mod collections;
mod concurrency;
mod constructors;
mod control_flow;
mod dispatch;
mod fixed_list_capacity;
mod format;
mod identifiers;
mod lambdas;
mod methods;
mod operators;
mod refutability;
mod registry_bridge;
mod sequences;
mod structs;
mod type_resolution;

#[cfg(test)]
use ori_ir::{BinaryOp, ParsedType, UnaryOp};

use super::InferEngine;
use calls::{infer_method_call, infer_method_call_named, MethodCallSite};

pub use calls::{compose_burden_for_idx, register_resolved_collection_burdens};
pub use checking::check_expr;
pub use type_resolution::resolve_parsed_type;

pub(crate) use calls::match_self_type;
pub(crate) use calls::register_concrete_applied_resolutions;
pub(crate) use calls::type_satisfies_named_trait;
pub(crate) use dispatch::infer_expr;
pub(crate) use fixed_list_capacity::validate_fixed_list_capacities;
pub(crate) use refutability::{pattern_is_irrefutable, NestedPathStep, RefutableReason};
pub(crate) use registry_bridge::{tag_to_type_tag, OP_TRAIT_MAP};

pub(super) use bindings::bind_pattern;
pub(super) use blocks::{infer_block, infer_let, infer_stmt};
pub(super) use calls::{compose_for_idx, infer_call, infer_call_named};
pub(super) use collections::{
    check_collect_method_call, infer_list, infer_list_spread, infer_map_literal, infer_map_spread,
    infer_range, infer_tuple,
};
pub(super) use concurrency::infer_function_exp;
pub(super) use constructors::{
    check_err, check_ok, check_some, infer_await, infer_err, infer_none, infer_ok, infer_some,
    infer_try, infer_with_capability,
};
pub(super) use control_flow::{
    check_match_pattern, for_loop_elem_ty, infer_break, infer_continue, infer_for, infer_if,
    infer_loop, infer_match, infer_while, substitute_type_params_with_map,
};
pub(super) use dispatch::infer_optional_or_unit;
pub(super) use format::infer_template_literal;
pub(super) use identifiers::{
    find_similar_type_names, infer_const, infer_function_ref, infer_ident, infer_self_ref,
};
pub(super) use lambdas::infer_lambda;
pub(super) use methods::resolve_builtin_method;
pub(super) use operators::{
    infer_assign, infer_assign_target, infer_binary, infer_cast, infer_unary,
};
pub(super) use sequences::infer_function_seq;
pub(super) use structs::{
    infer_field, infer_index, infer_struct, infer_struct_spread, lookup_struct_field_types,
};
pub(super) use type_resolution::resolve_and_check_parsed_type;

#[cfg(test)]
pub(super) use lambdas::should_generalize;

pub(in crate::infer::expr) use methods::range_method_requires_iteration;

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "Test assertions and helper setups are expected to panic on unexpected failure"
)]
#[expect(
    clippy::expect_used,
    reason = "Test assertions and helper setups are expected to panic on unexpected failure"
)]
mod tests;
