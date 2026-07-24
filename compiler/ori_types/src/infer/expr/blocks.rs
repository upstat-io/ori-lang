//! Block and let inference.

use ori_ir::{
    BindingPattern, BindingPatternId, ExprArena, ExprId, ExprKind, Mutability, Name, ParsedType,
    ParsedTypeId, Span,
};

use crate::type_error::TypeCheckError;
use crate::{ConstGenericTerm, ConstValue, Expected, Idx};

use super::super::InferEngine;
use super::fixed_list_capacity::generic_const_value;
use super::lambdas::maybe_generalize;
use super::{
    bind_pattern, check_expr, infer_expr, infer_optional_or_unit, pattern_is_irrefutable,
    resolve_and_check_parsed_type,
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
        infer_stmt(engine, arena, stmt);
    }

    let block_ty = infer_optional_or_unit(engine, arena, result);

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
    let _ = infer_let_binding_impl(engine, arena, pattern, ty, init, span);
    Idx::UNIT
}

/// Process one statement: infer an expression statement for its side
/// effects, or infer and bind a let initializer at its actual type.
pub(crate) fn infer_stmt(engine: &mut InferEngine<'_>, arena: &ExprArena, stmt: &ori_ir::Stmt) {
    match &stmt.kind {
        ori_ir::StmtKind::Expr(expr_id) => {
            let _ = infer_expr(engine, arena, *expr_id);
        }

        ori_ir::StmtKind::Let {
            pattern, ty, init, ..
        } => {
            let _ = infer_let_binding_impl(engine, arena, *pattern, *ty, *init, stmt.span);
        }
    }
}

/// Shared skeleton for expression-position and statement-position let
/// bindings. The initializer's source type is always the binding's type;
/// only an explicit `?` expression unwraps `Result` or `Option`.
fn infer_let_binding_impl(
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

    // Why: Env scope would hide subsequent block statements from this let-binding.
    engine.enter_rank_scope();

    let final_ty = if ty_id.is_valid() {
        let parsed_ty = arena.get_parsed_type(ty_id);
        let expected_ty = resolve_and_check_parsed_type(engine, arena, parsed_ty, span);
        let expected = fixed_list_const_capacity(engine, arena, parsed_ty).map_or_else(
            || Expected::from_annotation(expected_ty, binding_name.unwrap_or(Name::EMPTY), span),
            |capacity| {
                Expected::from_fixed_list_annotation(
                    expected_ty,
                    binding_name.unwrap_or(Name::EMPTY),
                    span,
                    capacity,
                )
            },
        );
        // Direct bidirectional Check(T) propagates the annotation into `init`.
        let _init_ty = check_expr(engine, arena, init, &expected, span);
        expected_ty
    } else {
        let init_ty = infer_expr(engine, arena, init);

        // Spec: Clause 14 Value Restriction: only non-capturing lambdas are generalized.
        maybe_generalize(engine, arena, init, init_ty)
    };

    rewrite_lambda_self_capture(engine, arena, init, binding_name, errors_before);

    let const_binding = if engine.error_count() == errors_before {
        immutable_const_binding(engine, arena, pat, init)
    } else {
        None
    };

    engine.exit_rank_scope();

    if let Err(reason) = pattern_is_irrefutable(engine, pat, final_ty) {
        let err = TypeCheckError::refutable_pattern(span, reason);
        engine.push_error(err);
    }
    bind_pattern(engine, arena, pat, final_ty);
    if let Some((name, value)) = const_binding {
        let recorded = engine.env_mut().record_local_const_value(name, value);
        debug_assert!(
            recorded,
            "immutable const evidence requires a local binding"
        );
    }

    final_ty
}

/// Preserve a concrete fixed-list capacity as a value-domain constraint.
///
/// The ordinary type representation erases fixed-list capacity to the list
/// carrier. This side constraint prevents a call such as
/// `let xs: [int, max K] = value.first_n()` from losing `N = 3` before the
/// shared method-monomorphization producer runs when `$K` is a compile-time
/// integer binding.
fn fixed_list_const_capacity(
    engine: &InferEngine<'_>,
    arena: &ExprArena,
    parsed: &ParsedType,
) -> Option<ConstGenericTerm> {
    let ParsedType::FixedList { capacity, .. } = parsed else {
        return None;
    };
    match generic_const_value(engine, arena, *capacity) {
        Some(value @ ConstValue::Int(_)) => Some(ConstGenericTerm::Value(value)),
        _ => None,
    }
}

/// Recover a simple immutable local whose initializer is in the generic-const
/// value domain. The ordinary type binding remains authoritative; this only
/// publishes value evidence after inference accepted the initializer.
fn immutable_const_binding(
    engine: &InferEngine<'_>,
    arena: &ExprArena,
    pattern: &BindingPattern,
    init: ExprId,
) -> Option<(Name, ConstValue)> {
    let BindingPattern::Name {
        name,
        mutable: Mutability::Immutable,
    } = pattern
    else {
        return None;
    };
    generic_const_value(engine, arena, init).map(|value| (*name, value))
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
        // A renamed field (`{ x: px }`) binds `px`, not the field name `x` —
        // recurse into the sub-pattern when present; shorthand (`{ x }`) has
        // no sub-pattern, so the field name IS the bound variable.
        ori_ir::BindingPattern::Struct { fields } => fields.first().and_then(|field| {
            field
                .pattern
                .as_ref()
                .map_or(Some(field.name), find_first_name)
        }),
        // `let` only accepts rest-only list patterns (irrefutable per
        // Spec: Clause 15.4), so `elements` is always empty in practice —
        // `rest` is the sole source of a bound name (`let [..tail] = ...`).
        ori_ir::BindingPattern::List { elements, rest } => elements
            .first()
            .and_then(find_first_name)
            .or_else(|| rest.map(|(name, _)| name)),
        ori_ir::BindingPattern::Wildcard => None,
    }
}
