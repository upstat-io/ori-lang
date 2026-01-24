//! Pattern expression type inference.
//!
//! Handles function_seq (run, try, match) and function_exp (map, filter, fold, etc.).

use crate::ir::{Name, Span, FunctionSeq, FunctionExp, SeqBinding};
use crate::patterns::TypeCheckContext;
use crate::types::Type;
use super::super::checker::TypeChecker;
use super::infer_expr;
use std::collections::HashMap;

/// Infer type for a function_seq expression (run, try, match).
pub fn infer_function_seq(
    checker: &mut TypeChecker<'_>,
    func_seq: &FunctionSeq,
    _span: Span,
) -> Type {
    match func_seq {
        FunctionSeq::Run { bindings, result, .. } => {
            // Create child scope for bindings
            let run_env = checker.env.child();
            let old_env = std::mem::replace(&mut checker.env, run_env);

            // Type check each binding/statement and add to scope
            let seq_bindings = checker.arena.get_seq_bindings(*bindings);
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
            checker.env = old_env;
            result_ty
        }

        FunctionSeq::Try { bindings, result, .. } => {
            // Similar to Run, but bindings unwrap Result/Option
            let try_env = checker.env.child();
            let old_env = std::mem::replace(&mut checker.env, try_env);

            let seq_bindings = checker.arena.get_seq_bindings(*bindings);
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

            checker.env = old_env;
            result_ty
        }

        FunctionSeq::Match { scrutinee, arms, .. } => {
            let scrutinee_ty = infer_expr(checker, *scrutinee);
            let match_arms = checker.arena.get_arms(*arms);

            if match_arms.is_empty() {
                checker.ctx.fresh_var()
            } else {
                // All arms must have the same type
                let first_arm_ty = infer_expr(checker, match_arms[0].body);
                for arm in &match_arms[1..] {
                    let arm_ty = infer_expr(checker, arm.body);
                    if let Err(e) = checker.ctx.unify(&first_arm_ty, &arm_ty) {
                        checker.report_type_error(e, arm.span);
                    }
                }
                let _ = scrutinee_ty; // TODO: pattern matching type checking
                first_arm_ty
            }
        }
    }
}

/// Infer type for a function_exp expression (map, filter, fold, etc.).
///
/// Uses the pattern registry for Open/Closed principle compliance.
/// Each pattern implementation is in a separate file under `patterns/`.
pub fn infer_function_exp(
    checker: &mut TypeChecker<'_>,
    func_exp: &FunctionExp,
) -> Type {
    let props = checker.arena.get_named_exprs(func_exp.props);

    // Type check all property values
    let prop_types: HashMap<Name, Type> = props.iter()
        .map(|prop| (prop.name, infer_expr(checker, prop.value)))
        .collect();

    // Look up pattern definition from registry
    let Some(pattern) = checker.registry.get(func_exp.kind) else {
        // Unknown pattern kind - should not happen if registry is complete
        return Type::Error;
    };

    // Create type check context with property types
    let mut ctx = TypeCheckContext::new(checker.interner, &mut checker.ctx, prop_types);

    // Delegate to pattern's type_check implementation
    pattern.type_check(&mut ctx)
}
