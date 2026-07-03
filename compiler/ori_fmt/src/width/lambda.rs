//! Shared lambda emit-width / emit-shape SSOT.
//!
//! Single canonical home for "what does a lambda render to?" Both width
//! measurement and rendering (inline + broken) call into this module so
//! they cannot drift.
//!
//! Spec: annex-d-formatting.md — typed lambdas with annotations inline;
//! mandatory parens for typed-lambda forms; expression body inline when
//! fits, broken at `=` when over-width.

use ori_ir::{ExprArena, Param, ParsedTypeId, StringLookup};

use super::WidthCalculator;
use crate::declarations::parsed_types::{calculate_type_width, const_expr_render_width};

/// Compute the inline-rendered width of a lambda head + body.
///
/// `body_w` is the pre-computed width of the lambda body. This function
/// adds the lambda head ceremony: parens (when applicable), per-param
/// `name[: type]`, ` -> `, optional `ret_ty = `.
pub(crate) fn lambda_emit_width<I: StringLookup>(
    wc: &mut WidthCalculator<'_, I>,
    arena: &ExprArena,
    params: &[Param],
    ret_ty: ParsedTypeId,
    body_w: usize,
) -> usize {
    let typed = ret_ty.is_valid();
    let needs_parens = needs_parens_for_lambda(params, ret_ty);

    let params_w = params_width_for_lambda(wc, arena, params);
    let head_w = if needs_parens {
        1 + params_w + 1 // "(" + params + ")"
    } else {
        params_w // bare name
    };

    let arrow_w = 4; // " -> "

    let ret_segment_w = if typed {
        // "ret_ty = " — recursive type measurement, NOT magic constant.
        // head + arrow(=4) + ret_ty_w + " = "(=3) + body
        type_width(wc, arena, ret_ty) + 3
    } else {
        0
    };

    head_w + arrow_w + ret_segment_w + body_w
}

/// Compute the inline-rendered width of a lambda's param list.
///
/// Per Annex D, lambda params emit as `name` (untyped) or `name: type`
/// (typed). Type measurement is RECURSIVE on the AST, NOT a hardcoded
/// estimate.
pub(crate) fn params_width_for_lambda<I: StringLookup>(
    wc: &mut WidthCalculator<'_, I>,
    arena: &ExprArena,
    params: &[Param],
) -> usize {
    if params.is_empty() {
        return 0;
    }
    let mut total = 0;
    let interner = wc.interner;
    let mut width_of_expr = |e| const_expr_render_width(arena, interner, e);
    for (i, param) in params.iter().enumerate() {
        total += interner.lookup(param.name).len();
        if let Some(ref ty) = param.ty {
            total += 2; // ": "
            total += calculate_type_width(ty, arena, interner, &mut width_of_expr);
        }
        if i < params.len() - 1 {
            total += 2; // ", "
        }
    }
    total
}

/// Recursive type-width measurement for arena-stored `ParsedTypeId`.
pub(crate) fn type_width<I: StringLookup>(
    wc: &mut WidthCalculator<'_, I>,
    arena: &ExprArena,
    ty: ParsedTypeId,
) -> usize {
    let parsed = arena.get_parsed_type(ty);
    let interner = wc.interner;
    let mut width_of_expr = |e| const_expr_render_width(arena, interner, e);
    calculate_type_width(parsed, arena, interner, &mut width_of_expr)
}

/// Single canonical predicate for the lambda-head parens decision.
///
/// Returns true iff the lambda must be rendered with parens around its
/// param list per Annex D: when `ret_ty` is present (`typed_lambda` grammar
/// requires parens), when there are zero or multiple params, or when
/// ANY param carries a type annotation. Both width measurement (this
/// module) and inline / broken render (`formatter/inline/mod.rs`,
/// `formatter/broken/mod.rs`) call this predicate so the parens decision
/// has exactly one home.
pub(crate) fn needs_parens_for_lambda(params: &[Param], ret_ty: ParsedTypeId) -> bool {
    let typed = ret_ty.is_valid();
    let single = params.len() == 1;
    typed || !single || params.iter().any(|p| p.ty.is_some())
}
