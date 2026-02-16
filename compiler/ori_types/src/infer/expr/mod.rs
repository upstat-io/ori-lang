//! Expression type inference.
//!
//! This module provides expression-level type inference using the
//! `InferEngine` infrastructure. It dispatches on `ExprKind` to
//! specialized inference functions.
//!
//! # Architecture
//!
//! Expression inference follows Hindley-Milner with bidirectional enhancements:
//!
//! - **Synthesis (infer)**: Bottom-up type derivation from expression structure
//! - **Checking (check)**: Top-down verification against expected type
//!
//! The dispatch is structured to match `ori_ir::ExprKind` variants,
//! with each category delegating to specialized modules:
//!
//! - Literals -> direct primitive type
//! - Identifiers -> environment lookup + instantiation
//! - Operators -> operator inference (binary, unary)
//! - Calls -> function/method call inference
//! - Control flow -> if/match/loop inference
//! - Lambdas -> lambda inference with scope management
//! - Collections -> list/map/tuple inference
//!
//! # Usage
//!
//! ```ignore
//! use ori_types::infer::{InferEngine, infer_expr};
//!
//! let mut pool = Pool::new();
//! let mut engine = InferEngine::new(&mut pool);
//!
//! // Infer type of expression
//! let ty = infer_expr(&mut engine, &arena, expr_id);
//! ```

mod blocks;
mod calls;
mod collections;
mod concurrency;
mod constructors;
mod control_flow;
mod identifiers;
mod methods;
mod operators;
mod sequences;
mod structs;
mod type_resolution;

// Re-export submodule contents for tests and sibling access
pub(super) use blocks::*;
pub(super) use calls::*;
pub(super) use collections::*;
pub(super) use concurrency::*;
pub(super) use constructors::*;
pub(super) use control_flow::*;
pub(super) use identifiers::*;
pub(super) use methods::*;
pub(super) use operators::*;
pub(super) use sequences::*;
pub(super) use structs::*;
// Public re-exports for the crate's public API
// (re-exported through infer/mod.rs)
pub use methods::TYPECK_BUILTIN_METHODS;
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
fn infer_expr_inner(engine: &mut InferEngine<'_>, arena: &ExprArena, expr_id: ExprId) -> Idx {
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
        ExprKind::Call { func, args } => infer_call(engine, arena, *func, *args, span),
        ExprKind::CallNamed { func, args } => infer_call_named(engine, arena, *func, *args, span),
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => infer_method_call(engine, arena, *receiver, *method, *args, span),
        ExprKind::MethodCallNamed {
            receiver,
            method,
            args,
        } => infer_method_call_named(engine, arena, *receiver, *method, *args, span),

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
            binding,
            iter,
            guard,
            body,
            is_yield,
            ..
        } => infer_for(
            engine, arena, *binding, *iter, *guard, *body, *is_yield, span,
        ),
        ExprKind::Loop { body, .. } => infer_loop(engine, arena, *body, span),

        // Blocks and Bindings
        ExprKind::Block { stmts, result } => infer_block(engine, arena, *stmts, *result, span),
        ExprKind::Let {
            pattern,
            ty,
            init,
            mutable,
        } => {
            let pat = arena.get_binding_pattern(*pattern);
            let ty_ref = if ty.is_valid() {
                Some(arena.get_parsed_type(*ty))
            } else {
                None
            };
            infer_let(engine, arena, pat, ty_ref, *init, *mutable, span)
        }

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
        ExprKind::Break { value, .. } => infer_break(engine, arena, *value, span),
        ExprKind::Continue { value, .. } => infer_continue(engine, arena, *value, span),
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
            // Infer each interpolated expression (for error reporting), result is always str
            for part in arena.get_template_parts(*parts) {
                infer_expr(engine, arena, part.expr);
            }
            Idx::STR
        }

        // Error
        ExprKind::Error => Idx::ERROR,
    };

    // Store the inferred type
    engine.store_type(expr_id.raw() as usize, ty);
    ty
}

/// Check an expression against an expected type.
///
/// This is the "check" direction of bidirectional type checking.
/// It handles cases where the expected type can guide literal typing:
///
/// - Integer literals in range 0-255 are coerced to `byte` when expected type is `byte`
/// - `iter.collect()` resolves to `Set<T>` when expected type is `Set<T>` (Collect trait)
///
/// For all other expressions, this infers the type and then checks against expected.
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

    // Special case: integer literals can coerce to byte when in range
    if let ExprKind::Int(value) = &expr.kind {
        if expected_tag == Tag::Byte {
            // Check if the literal is in the valid byte range (0-255)
            if *value >= 0 && *value <= 255 {
                // Coerce the literal to byte
                engine.store_type(expr_id.raw() as usize, Idx::BYTE);
                return Idx::BYTE;
            }
            // Out of range - infer as int and let check_type report the mismatch
        }
    }

    // Type-directed collect: when `iter.collect()` is expected to produce a Set,
    // resolve to `Set<T>` instead of the default `[T]`. This implements the
    // Collect trait's bidirectional type inference.
    if let ExprKind::MethodCall {
        receiver,
        method,
        args,
    } = &expr.kind
    {
        if expected_tag == Tag::Set {
            if let Some(ty) =
                check_collect_to_set(engine, arena, expr_id, *receiver, *method, *args)
            {
                return ty;
            }
        }
    }

    // Propagate expected type through `run(...)` to the result expression.
    // This enables bidirectional type checking to flow through function bodies,
    // which are always wrapped in `run(...)` (FunctionSeq::Run).
    if let ExprKind::FunctionSeq(seq_id) = &expr.kind {
        let func_seq = arena.get_function_seq(*seq_id);
        if let ori_ir::FunctionSeq::Run {
            pre_checks,
            bindings,
            result,
            post_checks,
            ..
        } = func_seq
        {
            let result_ty = check_run_seq(
                engine,
                arena,
                *pre_checks,
                *bindings,
                *result,
                *post_checks,
                expected,
                span,
            );
            engine.store_type(expr_id.raw() as usize, result_ty);
            return result_ty;
        }
    }

    // Default: infer the type and check against expected
    let inferred = infer_expr(engine, arena, expr_id);
    let _ = engine.check_type(inferred, expected, span);
    inferred
}

/// Bidirectional `run(...)`: propagate expected type to result expression.
///
/// Mirrors `infer_run_seq` but calls `check_expr` on the result expression
/// instead of `infer_expr`, enabling expected type propagation through function
/// bodies (which are always wrapped in `run(...)`).
#[expect(
    clippy::too_many_arguments,
    reason = "mirrors infer_run_seq signature plus expected/span"
)]
fn check_run_seq(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    pre_checks: ori_ir::CheckRange,
    bindings: ori_ir::SeqBindingRange,
    result: ExprId,
    post_checks: ori_ir::CheckRange,
    expected: &Expected,
    span: Span,
) -> Idx {
    engine.enter_scope();

    infer_pre_checks(engine, arena, pre_checks);

    let seq_bindings = arena.get_seq_bindings(bindings);
    for binding in seq_bindings {
        infer_seq_binding(engine, arena, binding, false);
    }

    // Check result expression against expected type (bidirectional)
    let result_ty = check_expr(engine, arena, result, expected, span);

    infer_post_checks(engine, arena, post_checks, result_ty);

    engine.exit_scope();
    result_ty
}

/// Bidirectional collect: resolve `iter.collect()` to `Set<T>` when expected.
///
/// Returns `Some(set_ty)` if the method is `collect` on an `Iterator<T>`,
/// storing the resolved `Set<T>` type. Returns `None` to fall through
/// to default inference.
fn check_collect_to_set(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    expr_id: ExprId,
    receiver: ExprId,
    method: ori_ir::Name,
    args: ori_ir::ExprRange,
) -> Option<Idx> {
    let method_str = engine.lookup_name(method)?;
    if method_str != "collect" {
        return None;
    }

    let recv_ty = infer_expr(engine, arena, receiver);
    let resolved = engine.resolve(recv_ty);
    if engine.pool().tag(resolved) != Tag::Iterator {
        return None;
    }

    // Infer arguments (collect has none, but be consistent)
    for &arg_id in arena.get_expr_list(args) {
        infer_expr(engine, arena, arg_id);
    }

    let elem = engine.pool().iterator_elem(resolved);
    let set_ty = engine.pool_mut().set(elem);
    engine.store_type(expr_id.raw() as usize, set_ty);
    Some(set_ty)
}

#[cfg(test)]
mod tests;
