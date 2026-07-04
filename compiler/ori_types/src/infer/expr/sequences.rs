//! Sequence pattern inference — `function_seq`, try, and for-pattern.

use ori_ir::{ExprArena, ExprId, Span};

use crate::{ContextKind, Expected, Idx, PatternKey, Tag};

use super::super::InferEngine;
use super::{
    check_expr, check_match_pattern, for_loop_elem_ty, infer_expr, infer_match, infer_stmt,
    LetInitUnwrap,
};

/// Infer type for a `function_seq` expression (try, match, for).
///
/// `FunctionSeq` represents sequential expressions where order matters:
/// - **Try**: `try { stmts; result }` - auto-unwrap `Result`/`Option`
/// - **Match**: `match scrutinee { Pattern -> expr, ... }` - pattern matching
/// - **`ForPattern`**: `for(over: items, match: Pattern -> expr, default: fallback)`
pub(crate) fn infer_function_seq(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    func_seq: &ori_ir::FunctionSeq,
    span: Span,
) -> Idx {
    use ori_ir::FunctionSeq;

    match func_seq {
        FunctionSeq::Try { stmts, result, .. } => infer_try_seq(
            engine,
            arena,
            *stmts,
            *result,
            span,
            &Expected::no_expectation(Idx::ERROR),
        ),

        FunctionSeq::Match {
            scrutinee,
            arms,
            span: match_span,
        } => infer_match(engine, arena, *scrutinee, *arms, *match_span),

        FunctionSeq::ForPattern {
            over,
            map,
            arm,
            default,
            ..
        } => infer_for_pattern(engine, arena, *over, *map, arm, *default, span),
    }
}

/// Infer type for `try { let x = fallible()?; result }`.
///
/// Like a block, but auto-unwraps [`Result`]/[`Option`] types in let bindings.
/// The entire expression returns a [`Result`] or [`Option`] wrapping the result.
///
/// ### Propagation
///
/// Bidirectional propagation: when `expected` resolves to a
/// concrete `Result<T, E>`, the result expression is checked against
/// `Check(T)` and the propagating let-binding error type is reconciled
/// against `E`. For non-Result expectations (`NoExpectation`, fresh Var,
/// or any other tag), the function falls back to bottom-up synthesis
/// so unification later catches mismatches.
///
/// # Returns
///
/// The wrapped `Result` or `Option` type.
pub(crate) fn infer_try_seq(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    stmts: ori_ir::StmtRange,
    result: ExprId,
    span: Span,
    expected: &Expected,
) -> Idx {
    let propagation = if expected.has_expectation() {
        let resolved = engine.resolve(expected.ty);
        if engine.pool().tag(resolved) == Tag::Result {
            Some((
                resolved,
                engine.pool().result_ok(resolved),
                engine.pool().result_err(resolved),
            ))
        } else {
            None
        }
    } else {
        None
    };

    engine.enter_scope();

    let mut error_ty: Option<Idx> = None;

    let stmts_list = arena.get_stmt_range(stmts);
    for stmt in stmts_list {
        infer_try_stmt(engine, arena, stmt);
        if let ori_ir::StmtKind::Let { init, .. } = &stmt.kind {
            if let Some(value_ty) = engine.get_type(init.raw() as usize) {
                let resolved = engine.resolve(value_ty);
                let tag = engine.pool().tag(resolved);
                if tag == Tag::Result && error_ty.is_none() {
                    error_ty = Some(engine.pool().result_err(resolved));
                }
            }
        }
    }

    let final_ty = if let Some((outer_result, inner_ok_ty, inner_err_ty)) = propagation {
        // Spec: Clause 16 (try), Clause 11 (block value): reconcile void block against expected Ok payload.
        let result_expected = Expected::from_context(inner_ok_ty, span, ContextKind::TryExpression);
        if result.is_present() {
            let _ = check_expr(engine, arena, result, &result_expected, span);
        } else {
            // Why: Reconcile void block against expected Ok payload so check_type reports mismatch (unify_types would be silent).
            let _ = engine.check_type(Idx::UNIT, &result_expected, span);
        }
        if let Some(et) = error_ty {
            let expected = Expected::from_context(inner_err_ty, span, ContextKind::TryExpression);
            let _ = engine.check_type(et, &expected, span);
        }
        outer_result
    } else {
        let result_ty = if result.is_present() {
            infer_expr(engine, arena, result)
        } else {
            Idx::UNIT
        };
        let resolved = engine.resolve(result_ty);
        let tag = engine.pool().tag(resolved);
        match (tag, error_ty) {
            // Why: Unify inner Err with let-binding error type to yield Result<T, E> instead of double-wrapping.
            (Tag::Result, Some(et)) => {
                let inner_err = engine.pool().result_err(resolved);
                let expected = Expected::from_context(et, span, ContextKind::TryExpression);
                let _ = engine.check_type(inner_err, &expected, span);
                result_ty
            }

            (Tag::Result, None) | (Tag::Option, _) => result_ty,
            (_, Some(et)) => engine.pool_mut().result(result_ty, et),

            (_, None) => {
                let err_var = engine.fresh_var();
                engine.pool_mut().result(result_ty, err_var)
            }
        }
    };

    engine.exit_scope();
    final_ty
}

/// Infer type for `for(over: items, [map: transform,] match: Pattern -> expr, default: fallback)`.
///
/// Iterates over a collection, applies optional map, finds first matching pattern,
/// or returns default.
fn infer_for_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    over: ExprId,
    map: Option<ExprId>,
    arm: &ori_ir::MatchArm,
    default: ExprId,
    span: Span,
) -> Idx {
    let over_ty = infer_expr(engine, arena, over);
    let elem_ty = for_loop_elem_ty(engine, over_ty, span);

    let scrutinee_ty = if let Some(map_fn) = map {
        let map_fn_ty = infer_expr(engine, arena, map_fn);
        let resolved_map = engine.resolve(map_fn_ty);

        if engine.pool().tag(resolved_map) == Tag::Function {
            engine.pool().function_return(resolved_map)
        } else {
            elem_ty
        }
    } else {
        elem_ty
    };

    engine.enter_scope();

    // Why: a for-pattern carries one inline MatchArm, not an ArmRange, so no
    // real arm_index exists per the PatternKey::Arm contract; u32::MAX is an
    // out-of-range placeholder key ori_canon never reads via resolve_pattern.
    check_match_pattern(
        engine,
        arena,
        &arm.pattern,
        scrutinee_ty,
        PatternKey::Arm(u32::MAX),
        arm.span,
    );

    if let Some(guard_id) = arm.guard {
        engine.push_context(ContextKind::MatchArmGuard { arm_index: 0 });
        let guard_ty = infer_expr(engine, arena, guard_id);
        let expected = Expected::from_context(
            Idx::BOOL,
            arena.get_expr(guard_id).span,
            ContextKind::MatchArmGuard { arm_index: 0 },
        );
        let _ = engine.check_type(guard_ty, &expected, arena.get_expr(guard_id).span);
        engine.pop_context();
    }

    let arm_ty = infer_expr(engine, arena, arm.body);

    engine.exit_scope();

    let default_ty = infer_expr(engine, arena, default);

    let expected = Expected::from_context(
        arm_ty,
        arena.get_expr(default).span,
        ContextKind::MatchArm { arm_index: 0 },
    );
    let _ = engine.check_type(default_ty, &expected, arena.get_expr(default).span);

    arm_ty
}

/// Process a try-block statement (let or expression).
///
/// Auto-unwraps `Result`/`Option` types in let bindings. Delegates to the
/// shared [`infer_stmt`] dispatcher — the [`LetInitUnwrap::ResultOrOption`]
/// twin of [`super::blocks::infer_block`]'s [`LetInitUnwrap::Direct`]
/// statement loop.
pub(crate) fn infer_try_stmt(engine: &mut InferEngine<'_>, arena: &ExprArena, stmt: &ori_ir::Stmt) {
    infer_stmt(engine, arena, stmt, LetInitUnwrap::ResultOrOption);
}
