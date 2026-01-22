//! Pattern expression type checking.
//!
//! This module handles type checking for Sigil's built-in patterns:
//! run, try, match, map, filter, fold, find, collect, recurse,
//! parallel, timeout, retry, cache, validate.

use crate::intern::{Name, TypeId, TypeInterner, TypeKind, StringInterner};
use crate::syntax::{Span, PatternKind, PatternArgsId, ExprArena};
use super::context::TypeContext;
use super::{TypeError, TypeErrorKind};

/// Type check a pattern expression.
pub fn check_pattern(
    ctx: &mut TypeContext<'_>,
    kind: PatternKind,
    args: PatternArgsId,
    span: Span,
) -> TypeId {
    match kind {
        PatternKind::Run => check_run(ctx, args, span),
        PatternKind::Try => check_try(ctx, args, span),
        PatternKind::Match => check_match(ctx, args, span),
        PatternKind::Map => check_map(ctx, args, span),
        PatternKind::Filter => check_filter(ctx, args, span),
        PatternKind::Fold => check_fold(ctx, args, span),
        PatternKind::Find => check_find(ctx, args, span),
        PatternKind::Collect => check_collect(ctx, args, span),
        PatternKind::Recurse => check_recurse(ctx, args, span),
        PatternKind::Parallel => check_parallel(ctx, args, span),
        PatternKind::Timeout => check_timeout(ctx, args, span),
        PatternKind::Retry => check_retry(ctx, args, span),
        PatternKind::Cache => check_cache(ctx, args, span),
        PatternKind::Validate => check_validate(ctx, args, span),
    }
}

/// run(stmt1, stmt2, ..., result) -> type of result
fn check_run(ctx: &mut TypeContext<'_>, args: PatternArgsId, span: Span) -> TypeId {
    let pattern_args = ctx.arena.get_pattern_args(args);
    let positional = ctx.arena.get_expr_list(pattern_args.positional);

    if positional.is_empty() {
        return TypeId::VOID;
    }

    // Infer all statements, return type of last one
    let mut last_ty = TypeId::VOID;
    for &expr in positional {
        last_ty = ctx.infer(expr);
    }
    last_ty
}

/// try(expr?, fallback) -> T
fn check_try(ctx: &mut TypeContext<'_>, args: PatternArgsId, span: Span) -> TypeId {
    // Simplified - would need proper argument handling
    ctx.unifier.fresh_var()
}

/// match(value, pat -> expr, ...) -> T
fn check_match(ctx: &mut TypeContext<'_>, args: PatternArgsId, span: Span) -> TypeId {
    ctx.unifier.fresh_var()
}

/// map(.over: [T], .transform: T -> U) -> [U]
fn check_map(ctx: &mut TypeContext<'_>, args: PatternArgsId, span: Span) -> TypeId {
    let pattern_args = ctx.arena.get_pattern_args(args);

    let over_name = ctx.interner.intern("over");
    let transform_name = ctx.interner.intern("transform");

    let mut over_ty = None;
    let mut transform_ty = None;

    for arg in &pattern_args.named {
        if arg.name == over_name {
            over_ty = Some(ctx.infer(arg.value));
        } else if arg.name == transform_name {
            transform_ty = Some(ctx.infer(arg.value));
        }
    }

    match (over_ty, transform_ty) {
        (Some(over), Some(transform)) => {
            // over should be [T]
            if let Some(TypeKind::List(elem_ty)) = ctx.types.lookup(over) {
                // transform should be T -> U
                if let Some(TypeKind::Function { params, ret }) = ctx.types.lookup(transform) {
                    let param_types = ctx.types.get_list(params);
                    if param_types.len() == 1 {
                        ctx.unify_or_error(param_types[0], elem_ty, span);
                        return ctx.types.intern_list(ret);
                    }
                }
            }
            ctx.types.intern(TypeKind::Error)
        }
        _ => {
            ctx.error(TypeError {
                kind: TypeErrorKind::MissingArg(over_name),
                span,
            });
            ctx.types.intern(TypeKind::Error)
        }
    }
}

/// filter(.over: [T], .predicate: T -> bool) -> [T]
fn check_filter(ctx: &mut TypeContext<'_>, args: PatternArgsId, span: Span) -> TypeId {
    let pattern_args = ctx.arena.get_pattern_args(args);

    let over_name = ctx.interner.intern("over");

    for arg in &pattern_args.named {
        if arg.name == over_name {
            let over_ty = ctx.infer(arg.value);
            if let Some(TypeKind::List(_)) = ctx.types.lookup(over_ty) {
                return over_ty; // filter preserves list type
            }
        }
    }

    ctx.unifier.fresh_var()
}

/// fold(.over: [T], .init: U, .op: (U, T) -> U) -> U
fn check_fold(ctx: &mut TypeContext<'_>, args: PatternArgsId, span: Span) -> TypeId {
    let pattern_args = ctx.arena.get_pattern_args(args);

    let init_name = ctx.interner.intern("init");

    for arg in &pattern_args.named {
        if arg.name == init_name {
            return ctx.infer(arg.value);
        }
    }

    ctx.unifier.fresh_var()
}

/// find(.over: [T], .where: T -> bool) -> Option<T>
fn check_find(ctx: &mut TypeContext<'_>, args: PatternArgsId, span: Span) -> TypeId {
    let pattern_args = ctx.arena.get_pattern_args(args);

    let over_name = ctx.interner.intern("over");

    for arg in &pattern_args.named {
        if arg.name == over_name {
            let over_ty = ctx.infer(arg.value);
            if let Some(TypeKind::List(elem)) = ctx.types.lookup(over_ty) {
                return ctx.types.intern_option(elem);
            }
        }
    }

    let elem = ctx.unifier.fresh_var();
    ctx.types.intern_option(elem)
}

/// collect(.range: Range<T>, .transform: T -> U) -> [U]
fn check_collect(ctx: &mut TypeContext<'_>, args: PatternArgsId, span: Span) -> TypeId {
    let elem = ctx.unifier.fresh_var();
    ctx.types.intern_list(elem)
}

/// recurse(.cond: bool, .base: T, .step: T) -> T
fn check_recurse(ctx: &mut TypeContext<'_>, args: PatternArgsId, span: Span) -> TypeId {
    let pattern_args = ctx.arena.get_pattern_args(args);

    let base_name = ctx.interner.intern("base");

    for arg in &pattern_args.named {
        if arg.name == base_name {
            return ctx.infer(arg.value);
        }
    }

    ctx.unifier.fresh_var()
}

/// parallel(.task1: T, .task2: U, ...) -> (T, U, ...)
fn check_parallel(ctx: &mut TypeContext<'_>, args: PatternArgsId, span: Span) -> TypeId {
    let pattern_args = ctx.arena.get_pattern_args(args);

    let types: Vec<_> = pattern_args.named.iter()
        .map(|arg| ctx.infer(arg.value))
        .collect();

    ctx.types.intern_tuple(&types)
}

/// timeout(.op: T, .after: Duration) -> Result<T, Error>
fn check_timeout(ctx: &mut TypeContext<'_>, args: PatternArgsId, span: Span) -> TypeId {
    let pattern_args = ctx.arena.get_pattern_args(args);

    let op_name = ctx.interner.intern("op");

    for arg in &pattern_args.named {
        if arg.name == op_name {
            let op_ty = ctx.infer(arg.value);
            let error_ty = ctx.types.intern(TypeKind::Named {
                name: ctx.interner.intern("Error"),
                type_args: crate::intern::TypeRange::EMPTY,
            });
            return ctx.types.intern_result(op_ty, error_ty);
        }
    }

    ctx.unifier.fresh_var()
}

/// retry(.op: T, .attempts: int, .backoff: Strategy) -> T
fn check_retry(ctx: &mut TypeContext<'_>, args: PatternArgsId, span: Span) -> TypeId {
    let pattern_args = ctx.arena.get_pattern_args(args);

    let op_name = ctx.interner.intern("op");

    for arg in &pattern_args.named {
        if arg.name == op_name {
            return ctx.infer(arg.value);
        }
    }

    ctx.unifier.fresh_var()
}

/// cache(.key: K, .compute: () -> V) -> V
fn check_cache(ctx: &mut TypeContext<'_>, args: PatternArgsId, span: Span) -> TypeId {
    let pattern_args = ctx.arena.get_pattern_args(args);

    let compute_name = ctx.interner.intern("compute");

    for arg in &pattern_args.named {
        if arg.name == compute_name {
            let compute_ty = ctx.infer(arg.value);
            if let Some(TypeKind::Function { ret, .. }) = ctx.types.lookup(compute_ty) {
                return ret;
            }
        }
    }

    ctx.unifier.fresh_var()
}

/// validate(.value: T, .rules: [...]) -> Result<T, Error>
fn check_validate(ctx: &mut TypeContext<'_>, args: PatternArgsId, span: Span) -> TypeId {
    let pattern_args = ctx.arena.get_pattern_args(args);

    let value_name = ctx.interner.intern("value");

    for arg in &pattern_args.named {
        if arg.name == value_name {
            let value_ty = ctx.infer(arg.value);
            let error_ty = ctx.types.intern(TypeKind::Named {
                name: ctx.interner.intern("Error"),
                type_args: crate::intern::TypeRange::EMPTY,
            });
            return ctx.types.intern_result(value_ty, error_ty);
        }
    }

    ctx.unifier.fresh_var()
}
