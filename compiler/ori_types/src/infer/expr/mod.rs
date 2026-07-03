//! Expression type inference.
//!
//! This module provides expression-level type inference using the
//! `InferEngine` infrastructure. It dispatches on `ExprKind` to
//! specialized inference functions.
//!
//! Expression inference follows Hindley-Milner with bidirectional checking:
//! - **Synthesis (infer)**: Bottom-up type derivation from expression structure
//! - **Checking (check)**: Top-down verification against expected type

mod blocks;
mod calls;
mod collections;
mod concurrency;
mod constructors;
mod control_flow;
mod format;
mod identifiers;
mod lambdas;
mod methods;
mod operators;
mod refutability;
mod registry_bridge;
pub(crate) use registry_bridge::{tag_to_type_tag, OP_TRAIT_MAP};
mod sequences;
mod structs;
mod type_resolution;

// Named re-exports for tests and sibling access. Kept alphabetical per
// submodule so drift is easy to spot at review time.

pub(super) use blocks::{infer_block, infer_let, infer_let_binding_core};
pub(crate) use calls::register_concrete_applied_resolutions;
pub(super) use calls::{
    compose_builtin_burdens_for_resolved_types, compose_for_idx, infer_call, infer_call_named,
    infer_method_call, infer_method_call_named,
};
pub use calls::{compose_burden_for_idx, register_resolved_collection_burdens};
pub(super) use collections::{
    check_collect_method_call, infer_list, infer_list_spread, infer_map_literal, infer_map_spread,
    infer_range, infer_tuple,
};
pub(super) use concurrency::{infer_cache, infer_catch, infer_recurse};
pub(super) use constructors::{
    check_err, check_ok, check_some, infer_await, infer_err, infer_none, infer_ok, infer_some,
    infer_try, infer_with_capability,
};
pub(super) use control_flow::{
    check_match_pattern, infer_break, infer_continue, infer_for, infer_if, infer_loop, infer_match,
    infer_while, substitute_type_params_with_map,
};
pub(super) use format::infer_template_literal;
pub(super) use identifiers::{
    find_similar_type_names, infer_const, infer_function_ref, infer_ident, infer_self_ref,
};
pub(super) use lambdas::infer_lambda;
#[cfg(test)]
pub(super) use lambdas::should_generalize;
pub(super) use methods::resolve_builtin_method;
// `range_method_requires_iteration` keeps its narrower `pub(in crate::infer::expr)`
// source visibility — re-exporting at the same level lets `calls/method_call.rs`
// reach it via `super::super::range_method_requires_iteration`.
pub(in crate::infer::expr) use methods::range_method_requires_iteration;
pub(super) use operators::{
    infer_assign, infer_assign_target, infer_binary, infer_cast, infer_unary,
};
#[cfg(test)]
pub(super) use sequences::infer_try_stmt;
pub(super) use sequences::{
    bind_pattern, infer_function_exp, infer_function_seq, unwrap_result_or_option,
};
pub(super) use structs::{
    infer_field, infer_index, infer_struct, infer_struct_spread, lookup_struct_field_types,
};
// Public re-exports for the crate's public API
// (re-exported through infer/mod.rs)
pub(crate) use refutability::{pattern_is_irrefutable, NestedPathStep, RefutableReason};
pub(super) use type_resolution::resolve_and_check_parsed_type;
pub use type_resolution::resolve_parsed_type;

use ori_ir::{ExprArena, ExprId, ExprKind, Span};
use ori_stack::ensure_sufficient_stack;

use super::InferEngine;
use crate::{Expected, Idx, Tag};

// Re-import types that tests.rs needs via `use super::*;`
// (these were in scope in the pre-split monolithic expr.rs)
#[cfg(test)]
use ori_ir::{BinaryOp, ParsedType, UnaryOp};

/// Infer the type of an expression.
///
/// This is the main entry point for expression type inference.
/// It dispatches to specialized handlers based on expression kind.
#[tracing::instrument(level = "trace", skip(engine, arena))]
pub fn infer_expr(engine: &mut InferEngine<'_>, arena: &ExprArena, expr_id: ExprId) -> Idx {
    ensure_sufficient_stack(|| infer_expr_inner(engine, arena, expr_id))
}

/// Inner implementation of expression inference, dispatching on `ExprKind`.
#[expect(
    clippy::too_many_lines,
    reason = "exhaustive ExprKind → inference handler router"
)]
fn
infer_expr_inner(engine: &mut InferEngine<'_>, arena: &ExprArena, expr_id: ExprId) -> Idx {
    let expr = arena.get_expr(expr_id);
    let span = expr.span;

    let ty = match &expr.kind {
        // Literals
        ExprKind::Int(_) | ExprKind::HashLength => Idx::INT,
        ExprKind::Float(_) => Idx::FLOAT,
        ExprKind::Bool(_) => Idx::BOOL,
        ExprKind::String(_) | ExprKind::TemplateFull(_) => Idx::STR,
        ExprKind::Char(_) => Idx::CHAR,
        ExprKind::Duration { .. } => Idx::DURATION,
        ExprKind::Size { .. } => Idx::SIZE,
        ExprKind::Unit => Idx::UNIT,

        // Identifiers
        ExprKind::Ident(name) => infer_ident(engine, *name, span),
        ExprKind::FunctionRef(name) => infer_function_ref(engine, *name, span),
        ExprKind::SelfRef => infer_self_ref(engine, span),
        ExprKind::Const(name) => infer_const(engine, *name, span),

        // Operators
        ExprKind::Binary { op, left, right } => {
            infer_binary(engine, arena, *op, *left, *right, span)
        }
        ExprKind::Unary { op, operand } => infer_unary(engine, arena, *op, *operand, span),

        // Calls
        ExprKind::Call { func, args } => infer_call(engine, arena, expr_id, *func, *args, span),
        ExprKind::CallNamed { func, args } => {
            infer_call_named(engine, arena, expr_id, *func, *args, span)
        }
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => infer_method_call(
            engine, arena, expr_id, *receiver, *method, *args, span, None,
        ),
        ExprKind::MethodCallNamed {
            receiver,
            method,
            args,
        } => infer_method_call_named(
            engine, arena, expr_id, *receiver, *method, *args, span, None,
        ),

        // Field/Index Access
        ExprKind::Field { receiver, field } => infer_field(engine, arena, *receiver, *field, span),
        ExprKind::Index { receiver, index } => infer_index(engine, arena, *receiver, *index, span),

        // Control Flow
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => infer_if(engine, arena, *cond, *then_branch, *else_branch, span),
        ExprKind::Match { scrutinee, arms } => infer_match(engine, arena, *scrutinee, *arms, span),
        ExprKind::For {
            label,
            pattern,
            iter,
            guard,
            body,
            is_yield,
        } => infer_for(
            engine, arena, *label, *pattern, *iter, *guard, *body, *is_yield, span,
        ),
        ExprKind::Loop { label, body } => infer_loop(engine, arena, *label, *body, span),
        ExprKind::While { label, cond, body } => {
            infer_while(engine, arena, *label, *cond, *body, span)
        }

        // Blocks and Bindings
        ExprKind::Block { stmts, result } => infer_block(engine, arena, *stmts, *result, span),
        ExprKind::Let {
            pattern,
            ty,
            init,
            mutable,
        } => infer_let(engine, arena, *pattern, *ty, *init, *mutable, span),

        // Lambdas
        ExprKind::Lambda {
            params,
            ret_ty,
            body,
        } => {
            let ret_ty_ref = if ret_ty.is_valid() {
                Some(arena.get_parsed_type(*ret_ty))
            } else {
                None
            };
            infer_lambda(engine, arena, *params, ret_ty_ref, *body, span)
        }

        // Collections
        ExprKind::List(elements) => infer_list(engine, arena, *elements, span),
        ExprKind::ListWithSpread(elements) => infer_list_spread(engine, arena, *elements, span),
        ExprKind::Tuple(elements) => infer_tuple(engine, arena, *elements, span),
        ExprKind::Map(entries) => infer_map_literal(engine, arena, *entries, span),
        ExprKind::MapWithSpread(elements) => infer_map_spread(engine, arena, *elements, span),
        ExprKind::Range {
            start,
            end,
            step,
            inclusive,
        } => infer_range(engine, arena, *start, *end, *step, *inclusive, span),

        // Structs
        ExprKind::Struct { name, fields } => infer_struct(engine, arena, *name, *fields, span),
        ExprKind::StructWithSpread { name, fields } => {
            infer_struct_spread(engine, arena, *name, *fields, span)
        }

        // Option/Result Constructors
        ExprKind::Ok(inner) => infer_ok(engine, arena, *inner, span),
        ExprKind::Err(inner) => infer_err(engine, arena, *inner, span),
        ExprKind::Some(inner) => infer_some(engine, arena, *inner, span),
        ExprKind::None => infer_none(engine),

        // Control Flow Expressions
        ExprKind::Break { label, value } => infer_break(engine, arena, *label, *value, span),
        ExprKind::Continue { label, value } => infer_continue(engine, arena, *label, *value, span),
        ExprKind::Unsafe(inner) => infer_expr(engine, arena, *inner),
        ExprKind::Try(inner) => infer_try(engine, arena, *inner, span),
        ExprKind::Await(inner) => infer_await(engine, arena, *inner, span),

        // Casts and Assignment
        ExprKind::Cast { expr, ty, fallible } => infer_cast(
            engine,
            arena,
            *expr,
            arena.get_parsed_type(*ty),
            *fallible,
            span,
        ),
        ExprKind::Assign { target, value } => infer_assign(engine, arena, *target, *value, span),
        ExprKind::AssignTarget { root, steps } => {
            // Why: `infer_assign_target` is run for side-effect desugar planning;
            // standalone assign-target chain discards the type and returns UNIT.
            let _ = infer_assign_target(engine, arena, expr_id, *root, *steps);
            Idx::UNIT
        }

        // Capabilities
        ExprKind::WithCapability {
            capability,
            provider,
            body,
        } => infer_with_capability(engine, arena, *capability, *provider, *body, span),

        // Pattern Expressions
        ExprKind::FunctionSeq(seq_id) => {
            let func_seq = arena.get_function_seq(*seq_id);
            infer_function_seq(engine, arena, func_seq, span)
        }
        ExprKind::FunctionExp(exp_id) => {
            let func_exp = arena.get_function_exp(*exp_id);
            infer_function_exp(engine, arena, func_exp)
        }

        // Template Literals
        ExprKind::TemplateLiteral { parts, .. } => {
            infer_template_literal(engine, arena, *parts, span)
        }

        // Error
        ExprKind::Error => Idx::ERROR,
    };

    // Store the inferred type
    engine.store_type(expr_id.raw() as usize, ty);
    ty
}

/// Coerces integer literals in range 0-255 to byte when expected type is byte.
fn check_int_literal_coercion(
    engine: &mut InferEngine<'_>,
    expr_id: ExprId,
    kind: &ExprKind,
    expected_tag: Tag,
) -> Option<Idx> {
    if let ExprKind::Int(value) = kind {
        if expected_tag == Tag::Byte {
            if *value >= 0 && *value <= 255 {
                engine.store_type(expr_id.raw() as usize, Idx::BYTE);
                return Some(Idx::BYTE);
            }
        }
    }
    None
}

fn check_sum_constructors(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    expr_id: ExprId,
    kind: &ExprKind,
    expected: &Expected,
    span: Span,
) -> Option<Idx> {
    match kind {
        ExprKind::Ok(inner) => {
            let ty = check_ok(engine, arena, *inner, span, expected);
            engine.store_type(expr_id.raw() as usize, ty);
            let _ = engine.check_type(ty, expected, span);
            Some(ty)
        }
        ExprKind::Err(inner) => {
            let ty = check_err(engine, arena, *inner, span, expected);
            engine.store_type(expr_id.raw() as usize, ty);
            let _ = engine.check_type(ty, expected, span);
            Some(ty)
        }
        ExprKind::Some(inner) => {
            let ty = check_some(engine, arena, *inner, span, expected);
            engine.store_type(expr_id.raw() as usize, ty);
            let _ = engine.check_type(ty, expected, span);
            Some(ty)
        }
        _ => None,
    }
}

fn check_method_calls(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    expr_id: ExprId,
    kind: &ExprKind,
    expected: &Expected,
    span: Span,
) -> Option<Idx> {
    match kind {
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => {
            let ty = infer_method_call(
                engine,
                arena,
                expr_id,
                *receiver,
                *method,
                *args,
                span,
                Some(expected),
            );
            engine.store_type(expr_id.raw() as usize, ty);
            let _ = engine.check_type(ty, expected, span);
            Some(ty)
        }
        ExprKind::MethodCallNamed {
            receiver,
            method,
            args,
        } => {
            let ty = infer_method_call_named(
                engine,
                arena,
                expr_id,
                *receiver,
                *method,
                *args,
                span,
                Some(expected),
            );
            engine.store_type(expr_id.raw() as usize, ty);
            let _ = engine.check_type(ty, expected, span);
            Some(ty)
        }
        _ => None,
    }
}

/// Check an expression against an expected type.
///
/// This implements the check direction of bidirectional type checking,
/// letting the type checker guide literal and method-call typing.
#[tracing::instrument(level = "trace", skip(engine, arena, expected))]
pub fn check_expr(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    expr_id: ExprId,
    expected: &Expected,
    span: Span,
) -> Idx {
    let expr = arena.get_expr(expr_id);

    // Resolve the expected type to see what we're checking against
    let expected_ty = engine.resolve(expected.ty);
    let expected_tag = engine.pool().tag(expected_ty);

    if let Some(ty) = check_int_literal_coercion(engine, expr_id, &expr.kind, expected_tag) {
        return ty;
    }

    // Why: bidirectional inference for `iter.collect()` with expected `Set<T>`
    // resolves to `Set<T>` rather than default `[T]`.
    if let Some(ty) = check_collect_method_call(
        engine,
        arena,
        expr_id,
        &expr.kind,
        expected,
        expected_tag,
        span,
    ) {
        return ty;
    }

    // Why: Thread expected type through `infer_try_seq` to check `x` against `T`
    // and avoid double-wrapping. Non-Result falls through to synthesis.
    if let ExprKind::FunctionSeq(seq_id) = &expr.kind {
        if let ori_ir::FunctionSeq::Try { stmts, result, .. } = arena.get_function_seq(*seq_id) {
            let ty = sequences::infer_try_seq(engine, arena, *stmts, *result, span, expected);
            engine.store_type(expr_id.raw() as usize, ty);
            let _ = engine.check_type(ty, expected, span);
            return ty;
        }
    }

    if let Some(ty) = check_sum_constructors(engine, arena, expr_id, &expr.kind, expected, span) {
        return ty;
    }

    if let Some(ty) = check_method_calls(engine, arena, expr_id, &expr.kind, expected, span) {
        return ty;
    }

    // Default: infer the type and check against expected
    let inferred = infer_expr(engine, arena, expr_id);
    let _ = engine.check_type(inferred, expected, span);
    inferred
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
#[expect(clippy::expect_used, reason = "Tests use expect for clarity")]
mod tests;
