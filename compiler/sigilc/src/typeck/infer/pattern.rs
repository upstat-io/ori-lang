//! Pattern expression type inference.
//!
//! Handles `function_seq` (run, try, match) and `function_exp` (map, filter, fold, etc.).

use crate::ir::{Name, Span, FunctionSeq, FunctionExp, SeqBinding};
use sigil_patterns::TypeCheckContext;
use crate::types::Type;
use super::super::checker::TypeChecker;
use super::infer_expr;
use std::collections::HashMap;

/// Infer type for a `function_seq` expression (run, try, match).
pub fn infer_function_seq(
    checker: &mut TypeChecker<'_>,
    func_seq: &FunctionSeq,
    _span: Span,
) -> Type {
    match func_seq {
        FunctionSeq::Run { bindings, result, .. } => {
            // Create child scope for bindings
            let run_env = checker.inference.env.child();
            let old_env = std::mem::replace(&mut checker.inference.env, run_env);

            // Type check each binding/statement and add to scope
            let seq_bindings = checker.context.arena.get_seq_bindings(*bindings);
            for binding in seq_bindings {
                match binding {
                    SeqBinding::Let { pattern, value, span: binding_span, .. } => {
                        // Check for closure self-capture
                        checker.check_closure_self_capture(pattern, *value, *binding_span);

                        let init_ty = infer_expr(checker, *value);
                        checker.bind_pattern(pattern, init_ty);
                    }
                    SeqBinding::Stmt { expr, .. } => {
                        // Type check for side effects (e.g., assignment)
                        infer_expr(checker, *expr);
                    }
                }
            }

            // Type check result expression
            let result_ty = infer_expr(checker, *result);

            // Restore parent scope
            checker.inference.env = old_env;
            result_ty
        }

        FunctionSeq::Try { bindings, result, .. } => {
            // Similar to Run, but bindings unwrap Result/Option
            let try_env = checker.inference.env.child();
            let old_env = std::mem::replace(&mut checker.inference.env, try_env);

            let seq_bindings = checker.context.arena.get_seq_bindings(*bindings);
            for binding in seq_bindings {
                match binding {
                    SeqBinding::Let { pattern, value, span: binding_span, .. } => {
                        // Check for closure self-capture
                        checker.check_closure_self_capture(pattern, *value, *binding_span);

                        let init_ty = infer_expr(checker, *value);
                        // Unwrap Result<T, E> or Option<T> to get T
                        let unwrapped = match &init_ty {
                            Type::Result { ok, .. } => (**ok).clone(),
                            Type::Option(some_ty) => (**some_ty).clone(),
                            other => other.clone(),
                        };
                        checker.bind_pattern(pattern, unwrapped);
                    }
                    SeqBinding::Stmt { expr, .. } => {
                        // Type check for side effects
                        infer_expr(checker, *expr);
                    }
                }
            }

            // Result expression should be Result or Option
            let result_ty = infer_expr(checker, *result);

            checker.inference.env = old_env;
            result_ty
        }

        FunctionSeq::Match { scrutinee, arms, .. } => {
            let scrutinee_ty = infer_expr(checker, *scrutinee);
            let match_arms = checker.context.arena.get_arms(*arms);

            if match_arms.is_empty() {
                checker.inference.ctx.fresh_var()
            } else {
                let mut result_ty: Option<Type> = None;

                for arm in match_arms {
                    // 1. Unify pattern with scrutinee type
                    super::unify_pattern_with_scrutinee(checker, &arm.pattern, &scrutinee_ty, arm.span);

                    // 2. Extract bindings from the pattern
                    let bindings = super::extract_match_pattern_bindings(checker, &arm.pattern, &scrutinee_ty);

                    // 3. Create child scope with pattern bindings
                    let mut arm_env = checker.inference.env.child();
                    for (name, ty) in bindings {
                        arm_env.bind(name, ty);
                    }
                    let old_env = std::mem::replace(&mut checker.inference.env, arm_env);

                    // 4. Type check guard if present
                    if let Some(guard_id) = arm.guard {
                        let guard_ty = infer_expr(checker, guard_id);
                        if let Err(e) = checker.inference.ctx.unify(&guard_ty, &Type::Bool) {
                            checker.report_type_error(&e, checker.context.arena.get_expr(guard_id).span);
                        }
                    }

                    // 5. Type check body
                    let arm_ty = infer_expr(checker, arm.body);

                    // 6. Restore scope
                    checker.inference.env = old_env;

                    // 7. Unify arm types
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

        FunctionSeq::ForPattern { over, map, arm, default, .. } => {
            // Type check the collection to iterate over
            let over_ty = infer_expr(checker, *over);

            // Determine element type for the iteration
            let resolved_over = checker.inference.ctx.resolve(&over_ty);
            let elem_ty = match &resolved_over {
                Type::List(elem) | Type::Set(elem) | Type::Range(elem) => (**elem).clone(),
                Type::Map { key, .. } => (**key).clone(),
                _ => checker.inference.ctx.fresh_var(),
            };

            // If there's a mapping function, apply it to get the scrutinee type
            let scrutinee_ty = if let Some(map_fn) = map {
                let map_fn_ty = infer_expr(checker, *map_fn);
                // The map function takes the element type and returns the scrutinee type
                match checker.inference.ctx.resolve(&map_fn_ty) {
                    Type::Function { ret, .. } => (*ret).clone(),
                    _ => elem_ty.clone(),
                }
            } else {
                elem_ty
            };

            // Unify pattern with the scrutinee type
            super::unify_pattern_with_scrutinee(checker, &arm.pattern, &scrutinee_ty, arm.span);

            // Extract bindings from the pattern
            let bindings = super::extract_match_pattern_bindings(checker, &arm.pattern, &scrutinee_ty);

            // Create child scope with pattern bindings
            let mut arm_env = checker.inference.env.child();
            for (name, ty) in bindings {
                arm_env.bind(name, ty);
            }
            let old_env = std::mem::replace(&mut checker.inference.env, arm_env);

            // Type check guard if present
            if let Some(guard_id) = arm.guard {
                let guard_ty = infer_expr(checker, guard_id);
                if let Err(e) = checker.inference.ctx.unify(&guard_ty, &Type::Bool) {
                    checker.report_type_error(&e, checker.context.arena.get_expr(guard_id).span);
                }
            }

            // Type check the arm body
            let arm_ty = infer_expr(checker, arm.body);

            // Restore scope
            checker.inference.env = old_env;

            // Type check the default value
            let default_ty = infer_expr(checker, *default);

            // Unify arm result with default
            if let Err(e) = checker.inference.ctx.unify(&arm_ty, &default_ty) {
                checker.report_type_error(&e, arm.span);
            }

            arm_ty
        }
    }
}

/// Infer type for a `function_exp` expression (map, filter, fold, etc.).
///
/// Uses the pattern registry for Open/Closed principle compliance.
/// Each pattern implementation is in a separate file under `patterns/`.
pub fn infer_function_exp(
    checker: &mut TypeChecker<'_>,
    func_exp: &FunctionExp,
) -> Type {
    let props = checker.context.arena.get_named_exprs(func_exp.props);

    // Type check all property values
    let prop_types: HashMap<Name, Type> = props.iter()
        .map(|prop| (prop.name, infer_expr(checker, prop.value)))
        .collect();

    // Look up pattern definition from registry
    let Some(pattern) = checker.registries.pattern.get(func_exp.kind) else {
        // Unknown pattern kind - should not happen if registry is complete
        return Type::Error;
    };

    // Create type check context with property types
    let mut ctx = TypeCheckContext::new(checker.context.interner, &mut checker.inference.ctx, prop_types);

    // Delegate to pattern's type_check implementation
    pattern.type_check(&mut ctx)
}
