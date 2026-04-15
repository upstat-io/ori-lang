//! Function and test body type checking (Passes 2 and 3 per `typeck.md` CK-1).
//!
//! Owns `check_function_bodies` (Pass 2) and `check_test_bodies` (Pass 3) and
//! their private body-checking helpers. See `bodies/mod.rs` for the architecture
//! docstring that covers all four body passes.

use ori_ir::{Function, Module, Name, TestDef};
use rustc_hash::FxHashSet;

use crate::check::ModuleChecker;
use crate::{check_expr, infer_expr, ContextKind, Expected, ExpectedOrigin, Idx};

/// Check all function bodies.
///
/// This pass runs after signature collection (Pass 1). Each function body
/// is type-checked against its declared return type.
#[tracing::instrument(level = "debug", skip_all, fields(count = module.functions.len()))]
pub fn check_function_bodies(checker: &mut ModuleChecker<'_>, module: &Module) {
    for func in &module.functions {
        check_function(checker, func);
    }
}

/// Type check a single function body.
fn check_function(checker: &mut ModuleChecker<'_>, func: &Function) {
    // Look up the pre-collected signature
    let Some(sig) = checker.get_signature(func.name).cloned() else {
        // This should never happen if Pass 1 ran correctly
        checker.error_undefined(func.name, func.span);
        return;
    };

    // Create child environment from frozen base
    let Some(child_env) = checker.child_of_base() else {
        // Base env not frozen - internal error
        return;
    };

    // Bind parameters in the child environment
    let mut param_env = child_env;
    for (name, ty) in sig.param_names.iter().zip(&sig.param_types) {
        param_env.bind(*name, *ty);
    }

    // Bind const generic parameters as their declared type.
    // E.g., for `@f<$N: int>`, bind N -> int so the body can reference N.
    for cp in &sig.const_params {
        param_env.bind(cp.name, cp.const_type);
    }

    // Bind capability names as fresh type variables so the body can
    // reference them (e.g., `@f () -> int uses Value = Value`).
    // The concrete type is provided by the caller via `with...in`.
    for &cap_name in &sig.capabilities {
        let cap_ty = checker.pool_mut().fresh_var();
        param_env.bind(cap_name, cap_ty);
    }

    // Build function type for recursion support
    let fn_type = checker
        .pool_mut()
        .function(&sig.param_types, sig.return_type);

    // Extract capabilities for scope context
    let capabilities: FxHashSet<Name> = sig.capabilities.iter().copied().collect();

    // Get spans before entering the checking scope
    let guard_span = func.guard.map(|id| checker.arena().get_expr(id).span);
    let body_span = checker.arena().get_expr(func.body).span;

    // Check body with function scope context
    let func_name = func.name;
    let (expr_types, errors, warnings, pat_resolutions, mono_instances, deferred_mono_calls) =
        checker.with_function_scope(fn_type, capabilities, |c| {
            // Get arena reference (lifetime 'a, not tied to c borrow)
            let arena = c.arena();

            // Create inference engine with prepared environment
            let mut engine = c.create_engine_with_env(param_env);

            // Set self type for recursive calls (self() in patterns like recurse)
            engine.set_self_type(fn_type);

            // Track current function for deferred mono call recording
            engine.set_current_function(Some(func_name));

            // Push context for better error messages
            engine.push_context(ContextKind::FunctionReturn {
                func_name: Some(func_name),
            });

            // Check guard expression if present
            if let (Some(guard_id), Some(span)) = (func.guard, guard_span) {
                let guard_ty = infer_expr(&mut engine, arena, guard_id);
                let expected_bool = Expected {
                    ty: Idx::BOOL,
                    origin: ExpectedOrigin::Context {
                        span,
                        kind: ContextKind::MatchArmGuard { arm_index: 0 }, // Reuse guard context
                    },
                };
                let _ = engine.check_type(guard_ty, &expected_bool, span);
            }

            // Check body against declared return type (bidirectional)
            let expected = Expected {
                ty: sig.return_type,
                origin: ExpectedOrigin::Context {
                    span: body_span,
                    kind: ContextKind::FunctionReturn {
                        func_name: Some(func_name),
                    },
                },
            };
            let _body_ty = check_expr(&mut engine, arena, func.body, &expected, body_span);

            engine.pop_context();

            (
                engine.take_expr_types(),
                engine.take_errors(),
                engine.take_warnings(),
                engine.take_pattern_resolutions(),
                engine.take_mono_instances(),
                engine.take_deferred_mono_calls(),
            )
        });

    // Store expression types
    for (expr_index, ty) in expr_types {
        checker.store_expr_type(expr_index, ty);
    }

    // Store errors and warnings
    for error in errors {
        checker.push_error(error);
    }
    for warning in warnings {
        checker.push_warning(warning);
    }

    // Accumulate pattern resolutions, mono instances, and deferred calls
    checker.pattern_resolutions.extend(pat_resolutions);
    checker.accumulate_mono_instances(mono_instances);
    checker.accumulate_deferred_mono_calls(deferred_mono_calls);
}

/// Check all test bodies.
///
/// Tests are similar to functions but:
/// - Always return unit (void)
/// - May have special test parameters
#[tracing::instrument(level = "debug", skip_all, fields(count = module.tests.len()))]
pub fn check_test_bodies(checker: &mut ModuleChecker<'_>, module: &Module) {
    for test in &module.tests {
        check_test(checker, test);
    }
}

/// Type check a single test body.
fn check_test(checker: &mut ModuleChecker<'_>, test: &TestDef) {
    // Look up pre-collected signature
    let Some(sig) = checker.get_signature(test.name).cloned() else {
        checker.error_undefined(test.name, test.span);
        return;
    };

    // Create child environment
    let Some(child_env) = checker.child_of_base() else {
        return;
    };

    // Bind parameters
    let mut param_env = child_env;
    for (name, ty) in sig.param_names.iter().zip(&sig.param_types) {
        param_env.bind(*name, *ty);
    }

    // Get arena reference (lifetime 'a, not tied to checker borrow)
    let arena = checker.arena();

    // Create inference engine and check body
    let fn_type = checker
        .pool_mut()
        .function(&sig.param_types, sig.return_type);
    let mut engine = checker.create_engine_with_env(param_env);
    engine.set_self_type(fn_type);

    // Push test context
    engine.push_context(ContextKind::TestBody);

    // Check body against declared return type (bidirectional)
    let body_span = arena.get_expr(test.body).span;
    let expected = Expected {
        ty: sig.return_type,
        origin: ExpectedOrigin::Context {
            span: body_span,
            kind: ContextKind::FunctionReturn {
                func_name: Some(test.name),
            },
        },
    };
    let _body_ty = check_expr(&mut engine, arena, test.body, &expected, body_span);

    engine.pop_context();

    // Extract results
    let expr_types = engine.take_expr_types();
    let errors = engine.take_errors();
    let warnings = engine.take_warnings();
    let pat_resolutions = engine.take_pattern_resolutions();
    let mono_instances = engine.take_mono_instances();
    let deferred_mono_calls = engine.take_deferred_mono_calls();

    // Store expression types
    for (expr_index, ty) in expr_types {
        checker.store_expr_type(expr_index, ty);
    }

    // Store errors and warnings
    for error in errors {
        checker.push_error(error);
    }
    for warning in warnings {
        checker.push_warning(warning);
    }

    // Accumulate pattern resolutions, mono instances, and deferred calls
    checker.pattern_resolutions.extend(pat_resolutions);
    checker.accumulate_mono_instances(mono_instances);
    checker.accumulate_deferred_mono_calls(deferred_mono_calls);
}
