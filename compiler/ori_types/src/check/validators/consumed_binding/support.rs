//! Consume-target recognition and binding-pattern support.

use ori_ir::{ExprId, ExprKind, MatchPattern, Name, Span};

use crate::TypeCheckError;

use super::{Consumed, WalkCtx};

/// Record one consume and diagnose a repeated consume of the same binding.
pub(super) fn record_consume(
    bound: Name,
    span: Span,
    consumed: &mut Consumed,
    errors: &mut Vec<TypeCheckError>,
) {
    if consumed.contains_key(&bound) {
        errors.push(TypeCheckError::use_after_drop_early(span, bound));
    }
    consumed.insert(bound, span);
}

fn is_drop_early_callee(ctx: &WalkCtx<'_>, func: ExprId) -> bool {
    matches!(*ctx.arena.expr_kind(func), ExprKind::Ident(callee) if callee == ctx.drop_early_name)
}

fn ident_consume_target(ctx: &WalkCtx<'_>, value: ExprId) -> Option<(Name, Span)> {
    if let ExprKind::Ident(bound) = *ctx.arena.expr_kind(value) {
        Some((bound, ctx.arena.get_expr(value).span))
    } else {
        None
    }
}

pub(super) fn drop_early_named_target(
    ctx: &WalkCtx<'_>,
    func: ExprId,
    args: ori_ir::CallArgRange,
) -> Option<(Name, Span)> {
    if !is_drop_early_callee(ctx, func) {
        return None;
    }
    let call_args = ctx.arena.get_call_args(args);
    if call_args.len() != 1 || call_args[0].is_spread {
        return None;
    }
    ident_consume_target(ctx, call_args[0].value)
}

pub(super) fn drop_early_positional_target(
    ctx: &WalkCtx<'_>,
    func: ExprId,
    args: ori_ir::ExprRange,
) -> Option<(Name, Span)> {
    if !is_drop_early_callee(ctx, func) {
        return None;
    }
    let arg_ids = ctx.arena.get_expr_list(args);
    if arg_ids.len() != 1 {
        return None;
    }
    ident_consume_target(ctx, arg_ids[0])
}

pub(super) fn unbind_binding_pattern(
    ctx: &WalkCtx<'_>,
    pattern: ori_ir::BindingPatternId,
    consumed: &mut Consumed,
) {
    for name in binding_pattern_bound_names(ctx, pattern) {
        consumed.remove(&name);
    }
}

pub(super) fn binding_pattern_bound_names(
    ctx: &WalkCtx<'_>,
    pattern: ori_ir::BindingPatternId,
) -> Vec<Name> {
    let mut names = Vec::new();
    collect_binding_pattern_names(ctx.arena.get_binding_pattern(pattern), &mut names);
    names
}

fn collect_binding_pattern_names(pattern: &ori_ir::BindingPattern, names: &mut Vec<Name>) {
    match pattern {
        ori_ir::BindingPattern::Name { name, .. } => names.push(*name),
        ori_ir::BindingPattern::Tuple(elements) => {
            for element in elements {
                collect_binding_pattern_names(element, names);
            }
        }
        ori_ir::BindingPattern::Struct { fields } => {
            for field in fields {
                match &field.pattern {
                    Some(pattern) => collect_binding_pattern_names(pattern, names),
                    None => names.push(field.name),
                }
            }
        }
        ori_ir::BindingPattern::List { elements, rest } => {
            for element in elements {
                collect_binding_pattern_names(element, names);
            }
            if let Some((name, _)) = rest {
                names.push(*name);
            }
        }
        ori_ir::BindingPattern::Wildcard => {}
    }
}

pub(super) fn match_pattern_bound_names(ctx: &WalkCtx<'_>, pattern: &MatchPattern) -> Vec<Name> {
    let mut names = Vec::new();
    collect_match_pattern_names(ctx, pattern, &mut names);
    names
}

fn collect_match_pattern_names(ctx: &WalkCtx<'_>, pattern: &MatchPattern, names: &mut Vec<Name>) {
    match pattern {
        MatchPattern::Binding(name) => names.push(*name),
        MatchPattern::At { name, pattern } => {
            names.push(*name);
            collect_match_pattern_names(ctx, ctx.arena.get_match_pattern(*pattern), names);
        }
        MatchPattern::Struct { fields, .. } => {
            for (field_name, subpattern) in fields {
                match subpattern {
                    Some(id) => {
                        collect_match_pattern_names(ctx, ctx.arena.get_match_pattern(*id), names);
                    }
                    None => names.push(*field_name),
                }
            }
        }
        MatchPattern::Variant { inner, .. }
        | MatchPattern::Tuple(inner)
        | MatchPattern::Or(inner) => {
            for subpattern in ctx.arena.get_match_pattern_list(*inner) {
                collect_match_pattern_names(ctx, ctx.arena.get_match_pattern(*subpattern), names);
            }
        }
        MatchPattern::List { elements, rest } => {
            for subpattern in ctx.arena.get_match_pattern_list(*elements) {
                collect_match_pattern_names(ctx, ctx.arena.get_match_pattern(*subpattern), names);
            }
            if let Some(name) = rest {
                names.push(*name);
            }
        }
        MatchPattern::Wildcard | MatchPattern::Literal(_) | MatchPattern::Range { .. } => {}
    }
}
