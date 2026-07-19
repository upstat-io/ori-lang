//! Central expression-kind dispatch.

use ori_ir::{ExprArena, ExprId, ExprKind, Span};
use ori_stack::ensure_sufficient_stack;

use crate::Idx;

use super::super::InferEngine;
use super::calls::{infer_method_call, infer_method_call_named, MethodCallSite};
use super::{
    infer_assign, infer_assign_target, infer_await, infer_binary, infer_block, infer_break,
    infer_call, infer_call_named, infer_cast, infer_const, infer_continue, infer_err, infer_field,
    infer_for, infer_function_exp, infer_function_ref, infer_function_seq, infer_ident, infer_if,
    infer_index, infer_lambda, infer_let, infer_list, infer_list_spread, infer_loop,
    infer_map_literal, infer_map_spread, infer_match, infer_none, infer_ok, infer_range,
    infer_self_ref, infer_some, infer_struct, infer_struct_spread, infer_template_literal,
    infer_try, infer_tuple, infer_unary, infer_while, infer_with_capability,
};

/// Infer the type of an expression.
///
/// This is the main entry point for expression type inference.
/// It dispatches to specialized handlers based on expression kind.
#[tracing::instrument(level = "trace", skip(engine, arena))]
pub(crate) fn infer_expr(engine: &mut InferEngine<'_>, arena: &ExprArena, expr_id: ExprId) -> Idx {
    ensure_sufficient_stack(|| infer_expr_inner(engine, arena, expr_id))
}

/// Infer the type of `expr_id`, or `()` when absent.
pub(in crate::infer) fn infer_optional_or_unit(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    expr_id: ExprId,
) -> Idx {
    if expr_id.is_present() {
        infer_expr(engine, arena, expr_id)
    } else {
        Idx::UNIT
    }
}

fn infer_lit_ident_op_call(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    expr_id: ExprId,
    kind: &ExprKind,
    span: Span,
) -> Idx {
    match kind {
        ExprKind::Int(_) | ExprKind::HashLength => Idx::INT,
        ExprKind::Float(_) => Idx::FLOAT,
        ExprKind::Bool(_) => Idx::BOOL,
        ExprKind::String(_) | ExprKind::TemplateFull(_) => Idx::STR,
        ExprKind::Char(_) => Idx::CHAR,
        ExprKind::Duration { .. } => Idx::DURATION,
        ExprKind::Size { .. } => Idx::SIZE,
        ExprKind::Unit => Idx::UNIT,
        ExprKind::Ident(name) => infer_ident(engine, *name, span),
        ExprKind::FunctionRef(name) => infer_function_ref(engine, *name, span),
        ExprKind::SelfRef => infer_self_ref(engine, span),
        ExprKind::Const(name) => infer_const(engine, *name, span),
        ExprKind::Binary { op, left, right } => {
            infer_binary(engine, arena, *op, *left, *right, span)
        }
        ExprKind::Unary { op, operand } => infer_unary(engine, arena, *op, *operand, span),
        ExprKind::Call { func, args } => infer_call(engine, arena, expr_id, *func, *args, span),
        ExprKind::CallNamed { func, args } => {
            infer_call_named(engine, arena, expr_id, *func, *args, span)
        }
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => infer_method_call(
            engine,
            arena,
            MethodCallSite::new(expr_id, *receiver, *method, span, None),
            *args,
        ),
        ExprKind::MethodCallNamed {
            receiver,
            method,
            args,
        } => infer_method_call_named(
            engine,
            arena,
            MethodCallSite::new(expr_id, *receiver, *method, span, None),
            *args,
        ),
        unexpected @ (ExprKind::If { .. }
        | ExprKind::Match { .. }
        | ExprKind::For { .. }
        | ExprKind::Loop { .. }
        | ExprKind::While { .. }
        | ExprKind::Block { .. }
        | ExprKind::Let { .. }
        | ExprKind::Lambda { .. }
        | ExprKind::List(_)
        | ExprKind::ListWithSpread(_)
        | ExprKind::Tuple(_)
        | ExprKind::Map(_)
        | ExprKind::MapWithSpread(_)
        | ExprKind::Range { .. }
        | ExprKind::Struct { .. }
        | ExprKind::StructWithSpread { .. }
        | ExprKind::Ok(_)
        | ExprKind::Err(_)
        | ExprKind::Some(_)
        | ExprKind::None
        | ExprKind::Field { .. }
        | ExprKind::Index { .. }
        | ExprKind::Break { .. }
        | ExprKind::Continue { .. }
        | ExprKind::Unsafe(_)
        | ExprKind::Try(_)
        | ExprKind::Await(_)
        | ExprKind::Cast { .. }
        | ExprKind::Assign { .. }
        | ExprKind::AssignTarget { .. }
        | ExprKind::WithCapability { .. }
        | ExprKind::FunctionSeq(_)
        | ExprKind::FunctionExp(_)
        | ExprKind::TemplateLiteral { .. }
        | ExprKind::Error) => unreachable!(
            "expression kind routed to literal/operator/call inference: {unexpected:?}"
        ),
    }
}

fn infer_control_block_lambda(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    kind: &ExprKind,
    span: Span,
) -> Idx {
    match kind {
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
        ExprKind::Block { stmts, result } => infer_block(engine, arena, *stmts, *result, span),
        ExprKind::Let {
            pattern,
            ty,
            init,
            mutable,
        } => infer_let(engine, arena, *pattern, *ty, *init, *mutable, span),

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

        unexpected @ (ExprKind::Int(_)
        | ExprKind::HashLength
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::TemplateFull(_)
        | ExprKind::Char(_)
        | ExprKind::Duration { .. }
        | ExprKind::Size { .. }
        | ExprKind::Unit
        | ExprKind::Ident(_)
        | ExprKind::FunctionRef(_)
        | ExprKind::SelfRef
        | ExprKind::Const(_)
        | ExprKind::Binary { .. }
        | ExprKind::Unary { .. }
        | ExprKind::Call { .. }
        | ExprKind::CallNamed { .. }
        | ExprKind::MethodCall { .. }
        | ExprKind::MethodCallNamed { .. }
        | ExprKind::List(_)
        | ExprKind::ListWithSpread(_)
        | ExprKind::Tuple(_)
        | ExprKind::Map(_)
        | ExprKind::MapWithSpread(_)
        | ExprKind::Range { .. }
        | ExprKind::Struct { .. }
        | ExprKind::StructWithSpread { .. }
        | ExprKind::Ok(_)
        | ExprKind::Err(_)
        | ExprKind::Some(_)
        | ExprKind::None
        | ExprKind::Field { .. }
        | ExprKind::Index { .. }
        | ExprKind::Break { .. }
        | ExprKind::Continue { .. }
        | ExprKind::Unsafe(_)
        | ExprKind::Try(_)
        | ExprKind::Await(_)
        | ExprKind::Cast { .. }
        | ExprKind::Assign { .. }
        | ExprKind::AssignTarget { .. }
        | ExprKind::WithCapability { .. }
        | ExprKind::FunctionSeq(_)
        | ExprKind::FunctionExp(_)
        | ExprKind::TemplateLiteral { .. }
        | ExprKind::Error) => {
            unreachable!("expression kind routed to control-flow inference: {unexpected:?}")
        }
    }
}

fn infer_collection_struct_misc(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    expr_id: ExprId,
    kind: &ExprKind,
    span: Span,
) -> Idx {
    match kind {
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
        ExprKind::Struct { name, fields } => infer_struct(engine, arena, *name, *fields, span),
        ExprKind::StructWithSpread { name, fields } => {
            infer_struct_spread(engine, arena, *name, *fields, span)
        }
        ExprKind::Ok(inner) => infer_ok(engine, arena, *inner, span),
        ExprKind::Err(inner) => infer_err(engine, arena, *inner, span),
        ExprKind::Some(inner) => infer_some(engine, arena, *inner, span),
        ExprKind::None => infer_none(engine),
        ExprKind::Field { receiver, field } => infer_field(engine, arena, *receiver, *field, span),
        ExprKind::Index { receiver, index } => {
            infer_index(engine, arena, expr_id, *receiver, *index, span)
        }
        unexpected @ (ExprKind::Int(_)
        | ExprKind::HashLength
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::TemplateFull(_)
        | ExprKind::Char(_)
        | ExprKind::Duration { .. }
        | ExprKind::Size { .. }
        | ExprKind::Unit
        | ExprKind::Ident(_)
        | ExprKind::FunctionRef(_)
        | ExprKind::SelfRef
        | ExprKind::Const(_)
        | ExprKind::Binary { .. }
        | ExprKind::Unary { .. }
        | ExprKind::Call { .. }
        | ExprKind::CallNamed { .. }
        | ExprKind::MethodCall { .. }
        | ExprKind::MethodCallNamed { .. }
        | ExprKind::If { .. }
        | ExprKind::Match { .. }
        | ExprKind::For { .. }
        | ExprKind::Loop { .. }
        | ExprKind::While { .. }
        | ExprKind::Block { .. }
        | ExprKind::Let { .. }
        | ExprKind::Lambda { .. }
        | ExprKind::Break { .. }
        | ExprKind::Continue { .. }
        | ExprKind::Unsafe(_)
        | ExprKind::Try(_)
        | ExprKind::Await(_)
        | ExprKind::Cast { .. }
        | ExprKind::Assign { .. }
        | ExprKind::AssignTarget { .. }
        | ExprKind::WithCapability { .. }
        | ExprKind::FunctionSeq(_)
        | ExprKind::FunctionExp(_)
        | ExprKind::TemplateLiteral { .. }
        | ExprKind::Error) => {
            unreachable!("expression kind routed to collection/struct inference: {unexpected:?}")
        }
    }
}

fn infer_expr_inner(engine: &mut InferEngine<'_>, arena: &ExprArena, expr_id: ExprId) -> Idx {
    let expr = arena.get_expr(expr_id);
    let span = expr.span;

    let ty = match &expr.kind {
        ExprKind::Int(_)
        | ExprKind::HashLength
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::TemplateFull(_)
        | ExprKind::Char(_)
        | ExprKind::Duration { .. }
        | ExprKind::Size { .. }
        | ExprKind::Unit
        | ExprKind::Ident(_)
        | ExprKind::FunctionRef(_)
        | ExprKind::SelfRef
        | ExprKind::Const(_)
        | ExprKind::Binary { .. }
        | ExprKind::Unary { .. }
        | ExprKind::Call { .. }
        | ExprKind::CallNamed { .. }
        | ExprKind::MethodCall { .. }
        | ExprKind::MethodCallNamed { .. } => {
            infer_lit_ident_op_call(engine, arena, expr_id, &expr.kind, span)
        }

        ExprKind::If { .. }
        | ExprKind::Match { .. }
        | ExprKind::For { .. }
        | ExprKind::Loop { .. }
        | ExprKind::While { .. }
        | ExprKind::Block { .. }
        | ExprKind::Let { .. }
        | ExprKind::Lambda { .. } => infer_control_block_lambda(engine, arena, &expr.kind, span),

        ExprKind::List(_)
        | ExprKind::ListWithSpread(_)
        | ExprKind::Tuple(_)
        | ExprKind::Map(_)
        | ExprKind::MapWithSpread(_)
        | ExprKind::Range { .. }
        | ExprKind::Struct { .. }
        | ExprKind::StructWithSpread { .. }
        | ExprKind::Ok(_)
        | ExprKind::Err(_)
        | ExprKind::Some(_)
        | ExprKind::None
        | ExprKind::Field { .. }
        | ExprKind::Index { .. } => {
            infer_collection_struct_misc(engine, arena, expr_id, &expr.kind, span)
        }

        ExprKind::Break { label, value } => infer_break(engine, arena, *label, *value, span),
        ExprKind::Continue { label, value } => infer_continue(engine, arena, *label, *value, span),
        ExprKind::Unsafe(inner) => infer_expr(engine, arena, *inner),
        ExprKind::Try(inner) => infer_try(engine, arena, *inner, span),
        ExprKind::Await(inner) => infer_await(engine, arena, *inner, span),
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
            infer_assign_target_unit(engine, arena, expr_id, *root, *steps)
        }
        ExprKind::WithCapability {
            capability,
            provider,
            body,
        } => infer_with_capability(engine, arena, *capability, *provider, *body, span),
        ExprKind::FunctionSeq(seq_id) => infer_function_seq_expr(engine, arena, *seq_id, span),
        ExprKind::FunctionExp(exp_id) => infer_function_exp_expr(engine, arena, *exp_id),
        ExprKind::TemplateLiteral { parts, .. } => {
            infer_template_literal(engine, arena, *parts, span)
        }
        ExprKind::Error => Idx::ERROR,
    };

    engine.store_type(expr_id.raw() as usize, ty);
    ty
}

fn infer_assign_target_unit(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    expr_id: ExprId,
    root: ExprId,
    steps: ori_ir::AccessStepRange,
) -> Idx {
    let _ = infer_assign_target(engine, arena, expr_id, root, steps);
    Idx::UNIT
}

fn infer_function_seq_expr(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    seq_id: ori_ir::FunctionSeqId,
    span: ori_ir::Span,
) -> Idx {
    infer_function_seq(engine, arena, arena.get_function_seq(seq_id), span)
}

fn infer_function_exp_expr(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    exp_id: ori_ir::FunctionExpId,
) -> Idx {
    infer_function_exp(engine, arena, arena.get_function_exp(exp_id))
}
