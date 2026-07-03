//! Block and let inference.

use ori_ir::{BindingPatternId, ExprArena, ExprId, ExprKind, Name, ParsedTypeId, Span};

use crate::type_error::TypeCheckError;
use crate::{Expected, ExpectedOrigin, Idx};

use super::super::InferEngine;
use super::lambdas::maybe_generalize;
use super::{
    bind_pattern, check_expr, infer_expr, pattern_is_irrefutable, resolve_and_check_parsed_type,
    unwrap_result_or_option,
};

/// Infer the type of a block expression.
pub(crate) fn infer_block(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    stmts: ori_ir::StmtRange,
    result: ExprId,
    _span: Span,
) -> Idx {
    engine.enter_scope();

    for stmt in arena.get_stmt_range(stmts) {
        match &stmt.kind {
            ori_ir::StmtKind::Expr(expr_id) => {
                let _ = infer_expr(engine, arena, *expr_id);
            }
            ori_ir::StmtKind::Let {
                pattern,
                ty,
                init,
                mutable: _,
            } => {
                let _ = infer_let_binding(engine, arena, *pattern, *ty, *init, stmt.span);
            }
        }
    }

    let block_ty = if result.is_present() {
        infer_expr(engine, arena, result)
    } else {
        Idx::UNIT
    };

    engine.exit_scope();

    block_ty
}

/// Infer the type of a let expression.
pub(crate) fn infer_let(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    pattern: BindingPatternId,
    ty: ParsedTypeId,
    init: ExprId,
    // Why: Mutability is not a type property in Ori's HM inference.
    _mutable: ori_ir::Mutability,
    span: Span,
) -> Idx {
    let _ = infer_let_binding(engine, arena, pattern, ty, init, span);
    Idx::UNIT
}

/// Infer and check a standard let-binding.
pub(crate) fn infer_let_binding(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    pattern_id: BindingPatternId,
    ty_id: ParsedTypeId,
    init: ExprId,
    span: Span,
) -> Idx {
    let pat = arena.get_binding_pattern(pattern_id);

    let binding_name = find_first_name(pat);
    let errors_before = engine.error_count();

    // Why: Enter a rank scope for let-polymorphism. Pushing an env scope
    // here would hide subsequent block statements from this let binding,
    // which must stay visible to the remainder of the block.
    engine.enter_rank_scope();

    let final_ty = if ty_id.is_valid() {
        let parsed_ty = arena.get_parsed_type(ty_id);
        let expected_ty = resolve_and_check_parsed_type(engine, arena, parsed_ty, span);
        let expected = Expected {
            ty: expected_ty,
            origin: ExpectedOrigin::Annotation {
                name: binding_name.unwrap_or(Name::EMPTY),
                span,
            },
        };
        let _init_ty = check_expr(engine, arena, init, &expected, span);
        expected_ty
    } else {
        let init_ty = infer_expr(engine, arena, init);

        // Why: Rewrite UnknownIdent errors matching the binding name to a
        // "closure cannot capture itself" message for lambda self-captures.
        if let Some(name) = binding_name {
            if matches!(arena.get_expr(init).kind, ExprKind::Lambda { .. }) {
                engine.rewrite_self_capture_errors(name, errors_before);
            }
        }

        // Spec: Value Restriction: only non-capturing lambdas are generalized.
        // Other initializers are monomorphic so E2005 surfaces on empty containers.
        // Spec: Clause 14
        maybe_generalize(engine, arena, init, init_ty)
    };

    engine.exit_rank_scope();

    if let Err(reason) = pattern_is_irrefutable(engine, pat, final_ty) {
        let err = TypeCheckError::refutable_pattern(span, reason);
        engine.push_error(err);
    }
    bind_pattern(engine, arena, pat, final_ty);

    final_ty
}

/// Infer and check a let-binding inside a try block, which auto-unwraps
/// Result or Option types.
pub(crate) fn infer_try_let_binding(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    pattern_id: BindingPatternId,
    ty_id: ParsedTypeId,
    init: ExprId,
    span: Span,
) -> Idx {
    let pat = arena.get_binding_pattern(pattern_id);

    let binding_name = find_first_name(pat);
    let errors_before = engine.error_count();

    // Why: Enter a rank scope for let-polymorphism. Pushing an env scope
    // here would hide subsequent block statements from this let binding,
    // which must stay visible to the remainder of the block.
    engine.enter_rank_scope();

    let final_ty = if ty_id.is_valid() {
        let parsed_ty = arena.get_parsed_type(ty_id);
        let expected_ty = resolve_and_check_parsed_type(engine, arena, parsed_ty, span);
        let expected = Expected {
            ty: expected_ty,
            origin: ExpectedOrigin::Annotation {
                name: binding_name.unwrap_or(Name::EMPTY),
                span,
            },
        };
        let init_ty = infer_expr(engine, arena, init);
        let unwrapped = unwrap_result_or_option(engine, init_ty);
        let _ = engine.check_type(unwrapped, &expected, span);
        expected_ty
    } else {
        let init_ty = infer_expr(engine, arena, init);

        // Why: Rewrite UnknownIdent errors matching the binding name to a
        // "closure cannot capture itself" message for lambda self-captures.
        if let Some(name) = binding_name {
            if matches!(arena.get_expr(init).kind, ExprKind::Lambda { .. }) {
                engine.rewrite_self_capture_errors(name, errors_before);
            }
        }

        let bound_ty = unwrap_result_or_option(engine, init_ty);

        // Spec: Value Restriction: only non-capturing lambdas are generalized.
        // Other initializers are monomorphic so E2005 surfaces on empty containers.
        // Spec: Clause 14
        maybe_generalize(engine, arena, init, bound_ty)
    };

    engine.exit_rank_scope();

    if let Err(reason) = pattern_is_irrefutable(engine, pat, final_ty) {
        let err = TypeCheckError::refutable_pattern(span, reason);
        engine.push_error(err);
    }
    bind_pattern(engine, arena, pat, final_ty);

    final_ty
}

/// Get the first name from a binding pattern (for error messages).
fn find_first_name(pattern: &ori_ir::BindingPattern) -> Option<Name> {
    match pattern {
        ori_ir::BindingPattern::Name { name, .. } => Some(*name),
        ori_ir::BindingPattern::Tuple(pats) => pats.first().and_then(find_first_name),
        ori_ir::BindingPattern::Struct { fields } => fields.first().map(|field| field.name),
        ori_ir::BindingPattern::List { elements, .. } => elements.first().and_then(find_first_name),
        ori_ir::BindingPattern::Wildcard => None,
    }
}
