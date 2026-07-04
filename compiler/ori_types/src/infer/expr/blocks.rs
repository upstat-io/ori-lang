//! Block and let inference.

use ori_ir::{BindingPatternId, ExprArena, ExprId, ExprKind, Name, ParsedTypeId, Span};

use crate::type_error::TypeCheckError;
use crate::{Expected, Idx};

use super::super::InferEngine;
use super::lambdas::maybe_generalize;
use super::{
    bind_pattern, check_expr, infer_expr, pattern_is_irrefutable, resolve_and_check_parsed_type,
};

/// Distinguishes a plain let-binding from a try-block let-binding.
///
/// A try-block binding auto-unwraps `Result`/`Option` initializers (Spec
/// Clause 17); a plain binding checks the initializer against the annotation
/// directly. The two binding shapes share every other step, so this enum is
/// the sole parameter separating [`infer_let_binding`] from the try-block let
/// path in `infer_let_binding_impl`, and — one level up — the sole parameter
/// separating a block's statement loop from a try-block's statement loop in
/// [`infer_stmt`].
#[derive(Clone, Copy)]
pub(crate) enum LetInitUnwrap {
    Direct,
    ResultOrOption,
}

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
        infer_stmt(engine, arena, stmt, LetInitUnwrap::Direct);
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
    infer_let_binding_impl(
        engine,
        arena,
        pattern_id,
        ty_id,
        init,
        span,
        LetInitUnwrap::Direct,
    )
}

/// Process one statement — infer an expression statement for its side
/// effects, or run a let-binding under `unwrap`'s mode.
///
/// Shared skeleton for [`infer_block`]'s statement loop and the try-block
/// statement loop in `sequences::infer_try_stmt` — both dispatch on the
/// same `StmtKind` shape, differing only in which [`LetInitUnwrap`] mode
/// the let-binding checks against.
pub(crate) fn infer_stmt(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    stmt: &ori_ir::Stmt,
    unwrap: LetInitUnwrap,
) {
    match &stmt.kind {
        ori_ir::StmtKind::Expr(expr_id) => {
            let _ = infer_expr(engine, arena, *expr_id);
        }

        ori_ir::StmtKind::Let {
            pattern, ty, init, ..
        } => {
            let _ = infer_let_binding_impl(engine, arena, *pattern, *ty, *init, stmt.span, unwrap);
        }
    }
}

/// Shared skeleton for [`infer_let_binding`] and the try-block let path
/// (invoked by [`infer_stmt`] with `unwrap: LetInitUnwrap::ResultOrOption`) —
/// the two binding shapes differ only in whether the initializer's type is
/// unwrapped from `Result`/`Option` before checking (`unwrap`).
fn infer_let_binding_impl(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    pattern_id: BindingPatternId,
    ty_id: ParsedTypeId,
    init: ExprId,
    span: Span,
    unwrap: LetInitUnwrap,
) -> Idx {
    let pat = arena.get_binding_pattern(pattern_id);

    let binding_name = find_first_name(pat);
    let errors_before = engine.error_count();

    // Why: env scope would hide subsequent block statements from this let-binding.
    engine.enter_rank_scope();

    let final_ty = if ty_id.is_valid() {
        let parsed_ty = arena.get_parsed_type(ty_id);
        let expected_ty = resolve_and_check_parsed_type(engine, arena, parsed_ty, span);
        let expected =
            Expected::from_annotation(expected_ty, binding_name.unwrap_or(Name::EMPTY), span);
        match unwrap {
            // Direct bidirectional Check(T) propagates the annotation into `init`.
            LetInitUnwrap::Direct => {
                let _init_ty = check_expr(engine, arena, init, &expected, span);
            }
            // The raw initializer type is Result<T, E>/Option<T>, not T, so it
            // must be inferred bottom-up and unwrapped before unifying with T.
            LetInitUnwrap::ResultOrOption => {
                let init_ty = infer_expr(engine, arena, init);
                let unwrapped = engine.unwrap_result_or_option(init_ty);
                let _ = engine.check_type(unwrapped, &expected, span);
            }
        }
        expected_ty
    } else {
        let init_ty = infer_expr(engine, arena, init);

        let bound_ty = match unwrap {
            LetInitUnwrap::Direct => init_ty,
            LetInitUnwrap::ResultOrOption => engine.unwrap_result_or_option(init_ty),
        };

        // Spec: Clause 14 Value Restriction: only non-capturing lambdas are generalized.
        maybe_generalize(engine, arena, init, bound_ty)
    };

    rewrite_lambda_self_capture(engine, arena, init, binding_name, errors_before);

    engine.exit_rank_scope();

    if let Err(reason) = pattern_is_irrefutable(engine, pat, final_ty) {
        let err = TypeCheckError::refutable_pattern(span, reason);
        engine.push_error(err);
    }
    bind_pattern(engine, arena, pat, final_ty);

    final_ty
}

/// Rewrite a self-referencing lambda initializer's `UnknownIdent` error (for
/// its own not-yet-bound binding name) into the friendlier `ClosureSelfCapture`
/// diagnostic, e.g. `let f = () -> f`.
///
/// Called once after `infer_let_binding_impl`'s annotated/unannotated branches
/// both resolve `init`'s type, so an annotated self-capturing closure gets the
/// same actionable message as an unannotated one, instead of a bare "unknown
/// identifier".
fn rewrite_lambda_self_capture(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    init: ExprId,
    binding_name: Option<Name>,
    errors_before: usize,
) {
    if let Some(name) = binding_name {
        if matches!(arena.get_expr(init).kind, ExprKind::Lambda { .. }) {
            engine.rewrite_self_capture_errors(name, errors_before);
        }
    }
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
