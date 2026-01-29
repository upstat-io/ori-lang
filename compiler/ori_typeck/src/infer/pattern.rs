//! Pattern expression type inference.
//!
//! Handles `function_seq` (run, try, match) and `function_exp` (map, filter, fold, etc.).

use super::infer_expr;
use crate::checker::TypeChecker;
use ori_ir::{FunctionExp, FunctionSeq, Name, SeqBinding, Span};
use ori_patterns::TypeCheckContext;
use ori_types::Type;
use std::collections::HashMap;

/// Infer type for a `function_seq` expression (run, try, match).
pub fn infer_function_seq(
    checker: &mut TypeChecker<'_>,
    func_seq: &FunctionSeq,
    _span: Span,
) -> Type {
    match func_seq {
        FunctionSeq::Run {
            bindings, result, ..
        } => checker.with_infer_env_scope(|checker| {
            let seq_bindings = checker.context.arena.get_seq_bindings(*bindings);
            for binding in seq_bindings {
                match binding {
                    SeqBinding::Let {
                        pattern,
                        ty,
                        value,
                        span: binding_span,
                        ..
                    } => {
                        let init_ty =
                            super::infer_let_init(checker, pattern, *value, *binding_span);
                        let final_ty =
                            super::check_type_annotation(checker, ty.as_ref(), init_ty, *value);
                        checker.bind_pattern(pattern, final_ty, *binding_span);
                    }
                    SeqBinding::Stmt { expr, .. } => {
                        infer_expr(checker, *expr);
                    }
                }
            }

            infer_expr(checker, *result)
        }),

        FunctionSeq::Try {
            bindings, result, ..
        } => {
            checker.with_infer_env_scope(|checker| {
                let seq_bindings = checker.context.arena.get_seq_bindings(*bindings);
                for binding in seq_bindings {
                    match binding {
                        SeqBinding::Let {
                            pattern,
                            ty,
                            value,
                            span: binding_span,
                            ..
                        } => {
                            let init_ty =
                                super::infer_let_init(checker, pattern, *value, *binding_span);
                            // Unwrap Result/Option for try bindings
                            let unwrapped = match &init_ty {
                                Type::Result { ok, .. } => (**ok).clone(),
                                Type::Option(some_ty) => (**some_ty).clone(),
                                other => other.clone(),
                            };
                            let final_ty = super::check_type_annotation(
                                checker,
                                ty.as_ref(),
                                unwrapped,
                                *value,
                            );
                            checker.bind_pattern(pattern, final_ty, *binding_span);
                        }
                        SeqBinding::Stmt { expr, .. } => {
                            infer_expr(checker, *expr);
                        }
                    }
                }

                infer_expr(checker, *result)
            })
        }

        FunctionSeq::Match {
            scrutinee, arms, ..
        } => {
            let scrutinee_ty = infer_expr(checker, *scrutinee);
            let match_arms = checker.context.arena.get_arms(*arms);

            if match_arms.is_empty() {
                checker.inference.ctx.fresh_var()
            } else {
                let mut result_ty: Option<Type> = None;

                for arm in match_arms {
                    super::unify_pattern_with_scrutinee(
                        checker,
                        &arm.pattern,
                        &scrutinee_ty,
                        arm.span,
                    );

                    let bindings =
                        super::extract_match_pattern_bindings(checker, &arm.pattern, &scrutinee_ty);

                    let arm_ty = checker.with_infer_bindings(bindings, |checker| {
                        if let Some(guard_id) = arm.guard {
                            let guard_ty = infer_expr(checker, guard_id);
                            if let Err(e) = checker.inference.ctx.unify(&guard_ty, &Type::Bool) {
                                checker.report_type_error(
                                    &e,
                                    checker.context.arena.get_expr(guard_id).span,
                                );
                            }
                        }

                        infer_expr(checker, arm.body)
                    });

                    match &result_ty {
                        Some(expected) => {
                            if let Err(e) = checker.inference.ctx.unify(expected, &arm_ty) {
                                checker.report_type_error(&e, arm.span);
                            }
                        }
                        None => {
                            result_ty = Some(arm_ty);
                        }
                    }
                }

                result_ty.unwrap_or_else(|| checker.inference.ctx.fresh_var())
            }
        }

        FunctionSeq::ForPattern {
            over,
            map,
            arm,
            default,
            ..
        } => {
            let over_ty = infer_expr(checker, *over);

            let resolved_over = checker.inference.ctx.resolve(&over_ty);
            let elem_ty = match &resolved_over {
                Type::List(elem) | Type::Set(elem) | Type::Range(elem) => (**elem).clone(),
                Type::Map { key, .. } => (**key).clone(),
                _ => checker.inference.ctx.fresh_var(),
            };

            let scrutinee_ty = if let Some(map_fn) = map {
                let map_fn_ty = infer_expr(checker, *map_fn);
                match checker.inference.ctx.resolve(&map_fn_ty) {
                    Type::Function { ret, .. } => (*ret).clone(),
                    _ => elem_ty.clone(),
                }
            } else {
                elem_ty
            };

            super::unify_pattern_with_scrutinee(checker, &arm.pattern, &scrutinee_ty, arm.span);

            let bindings =
                super::extract_match_pattern_bindings(checker, &arm.pattern, &scrutinee_ty);

            let arm_ty = checker.with_infer_bindings(bindings, |checker| {
                if let Some(guard_id) = arm.guard {
                    let guard_ty = infer_expr(checker, guard_id);
                    if let Err(e) = checker.inference.ctx.unify(&guard_ty, &Type::Bool) {
                        checker
                            .report_type_error(&e, checker.context.arena.get_expr(guard_id).span);
                    }
                }

                infer_expr(checker, arm.body)
            });

            let default_ty = infer_expr(checker, *default);

            if let Err(e) = checker.inference.ctx.unify(&arm_ty, &default_ty) {
                checker.report_type_error(&e, arm.span);
            }

            arm_ty
        }
    }
}

/// Infer type for a `function_exp` expression (map, filter, fold, etc.).
pub fn infer_function_exp(checker: &mut TypeChecker<'_>, func_exp: &FunctionExp) -> Type {
    let props = checker.context.arena.get_named_exprs(func_exp.props);

    // Get pattern definition (all FunctionExpKind variants are covered)
    let pattern = checker.registries.pattern.get(func_exp.kind);

    let scoped_bindings = pattern.scoped_bindings();

    // If there are no scoped bindings, use the simple path
    if scoped_bindings.is_empty() {
        let prop_types: HashMap<Name, Type> = props
            .iter()
            .map(|prop| (prop.name, infer_expr(checker, prop.value)))
            .collect();

        let mut ctx = TypeCheckContext::new(
            checker.context.interner,
            &mut checker.inference.ctx,
            prop_types,
        );
        return pattern.type_check(&mut ctx);
    }

    // Handle scoped bindings: type-check in phases
    infer_function_exp_with_scoped_bindings(checker, props, &*pattern, scoped_bindings)
}

/// Infer type for a `function_exp` that has scoped bindings.
///
/// This handles patterns like `recurse` where certain properties (like `step`)
/// need identifiers (like `self`) to be in scope during type checking.
fn infer_function_exp_with_scoped_bindings(
    checker: &mut TypeChecker<'_>,
    props: &[ori_ir::NamedExpr],
    pattern: &dyn ori_patterns::PatternDefinition,
    scoped_bindings: &[ori_patterns::ScopedBinding],
) -> Type {
    use ori_patterns::ScopedBindingType;

    let props_needing_scope: std::collections::HashSet<Name> = scoped_bindings
        .iter()
        .flat_map(|b| {
            b.for_props
                .iter()
                .map(|p| checker.context.interner.intern(p))
        })
        .collect();

    let mut prop_types: HashMap<Name, Type> = HashMap::new();

    for prop in props {
        if !props_needing_scope.contains(&prop.name) {
            let ty = infer_expr(checker, prop.value);
            prop_types.insert(prop.name, ty);
        }
    }

    for prop in props {
        if props_needing_scope.contains(&prop.name) {
            let bindings_for_prop: Vec<(Name, Type)> = scoped_bindings
                .iter()
                .filter(|b| {
                    b.for_props
                        .iter()
                        .any(|p| checker.context.interner.intern(p) == prop.name)
                })
                .map(|binding| {
                    let binding_name = checker.context.interner.intern(binding.name);
                    let binding_type = match &binding.type_from {
                        ScopedBindingType::SameAs(source_prop) => {
                            let source_name = checker.context.interner.intern(source_prop);
                            prop_types
                                .get(&source_name)
                                .cloned()
                                .unwrap_or_else(|| checker.inference.ctx.fresh_var())
                        }
                        ScopedBindingType::FunctionReturning(source_prop) => {
                            let source_name = checker.context.interner.intern(source_prop);
                            let ret_type = prop_types
                                .get(&source_name)
                                .cloned()
                                .unwrap_or_else(|| checker.inference.ctx.fresh_var());
                            Type::Function {
                                params: vec![],
                                ret: Box::new(ret_type),
                            }
                        }
                        ScopedBindingType::EnclosingFunction => {
                            // Use the enclosing function's type for recursive patterns like `recurse`
                            checker
                                .scope
                                .current_function_type
                                .clone()
                                .unwrap_or_else(|| checker.inference.ctx.fresh_var())
                        }
                    };
                    (binding_name, binding_type)
                })
                .collect();

            let ty = checker
                .with_infer_bindings(bindings_for_prop, |checker| infer_expr(checker, prop.value));
            prop_types.insert(prop.name, ty);
        }
    }

    let mut ctx = TypeCheckContext::new(
        checker.context.interner,
        &mut checker.inference.ctx,
        prop_types,
    );
    pattern.type_check(&mut ctx)
}
