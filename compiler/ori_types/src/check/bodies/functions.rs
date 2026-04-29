//! Function and test body type checking (Passes 2 and 3 per `typeck.md` CK-1).
//!
//! Owns `check_function_bodies` (Pass 2) and `check_test_bodies` (Pass 3) and
//! their private body-checking helpers. See `bodies/mod.rs` for the architecture
//! docstring that covers all four body passes.

use ori_ir::{Function, Module, Name, TestDef};
use rustc_hash::FxHashSet;

use crate::check::validators::build_exempt_var_ids;
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
    // Look up the pre-collected signature. Cloned into a mutable local so the
    // end-of-body defaulting pass can refresh `param_types`,
    // `return_type`, and Merkle hashes before `validate_body_types` runs and
    // before the sig is written back to `checker.signatures`.
    let Some(mut sig) = checker.get_signature(func.name).cloned() else {
        // This should never happen if Pass 1 ran correctly
        checker.error_undefined(func.name, func.span);
        return;
    };

    // Build the exempt var-id set once, before entering the inference
    // closure. The engine method receives `&FxHashSet<u32>` — avoids an
    // `infer → check::validators` upward import per `compiler.md §Architecture`.
    let exempt = build_exempt_var_ids(checker.pool(), &sig.scheme_var_ids);

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
    let (
        expr_types,
        errors,
        warnings,
        pat_resolutions,
        mono_instances,
        mono_dispatch_pre_dedup,
        deferred_mono_calls,
    ) = checker.with_function_scope(fn_type, capabilities, |c| {
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

        // default unbound vars reachable from empty-literal
        // expr roots to `Idx::NEVER` before exporting `expr_types` and
        // before `validate_body_types` runs. Mutates `sig.param_types`,
        // `sig.return_type`, and refreshes `param_hashes` / `return_hash`
        // via `substitute_in_pool` + direct-assign to `VarState::Link`.
        let mut expr_types = engine.take_expr_types();
        engine.default_unbound_vars_from_empty_literals(arena, &mut expr_types, &mut sig, &exempt);

        // §08.3b.1 — normalize `Tag::Var(Generalized)` leaves in
        // `expr_types` / sig positions to `Tag::BoundVar` per
        // `types.md §SC-1`. Drains `pending_generalized_vars` from
        // inner let-polymorphism AND rewrites the sig's scheme var
        // ids (populated by signatures pass for top-level polymorphic
        // functions). MUST run after defaulting (keeps `Idx::NEVER`
        // substitutions intact) and before `validate_body_types`
        // (validator's `Generalized` exemption is stripped — the
        // rewrite is now the only path keeping scheme vars legitimate).
        engine.normalize_body_generalized_to_bound_var_sig(&mut expr_types, &mut sig);

        (
            expr_types,
            engine.take_errors(),
            engine.take_warnings(),
            engine.take_pattern_resolutions(),
            engine.take_mono_instances(),
            engine.take_mono_dispatch_pre_dedup(),
            engine.take_deferred_mono_calls(),
        )
    });

    // Shared PC-2 validation + store/push/accumulate spine (§03.1–§03.4).
    super::finalize_body_and_export(
        checker,
        &sig,
        func.span,
        super::BodyOutputs {
            expr_types,
            errors,
            warnings,
            pat_resolutions,
            mono_instances,
            mono_dispatch_pre_dedup,
            deferred_mono_calls,
        },
    );

    // (Plan TPR Round 1 Codex-F2): write the defaulted signature
    // back to the checker's signature map so cross-function lookups and the
    // cross-module identity channel (`output/mod.rs:442-457` param_hashes /
    // return_hash) see the post-defaulted types instead of the pre-defaulted
    // ones. Prevents incremental-cache divergence on re-checks.
    checker.signatures.insert(func.name, sig);
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
    // Look up pre-collected signature. Cloned into a mutable local so the
    // defaulting pass can refresh `param_types` / `return_type` /
    // Merkle hashes before export.
    let Some(mut sig) = checker.get_signature(test.name).cloned() else {
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

    // Build the exempt var-id set before the engine takes a mut borrow.
    // See check_function for the rationale — engine method receives
    // `&FxHashSet<u32>` to avoid an `infer → check::validators` upward import.
    let exempt = build_exempt_var_ids(checker.pool(), &sig.scheme_var_ids);

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

    // default unbound vars reachable from empty-literal expr
    // roots before exporting types. Defaulting lands BEFORE Section 03.2's
    // validate_body_types wiring arrives, so when that wiring lands tests
    // with empty literals still type-check without E2005.
    let mut expr_types = engine.take_expr_types();
    engine.default_unbound_vars_from_empty_literals(arena, &mut expr_types, &mut sig, &exempt);

    // §08.3b.1 — normalize scheme vars to `Tag::BoundVar` per `types.md §SC-1`.
    // See `check_function` for the full rationale.
    engine.normalize_body_generalized_to_bound_var_sig(&mut expr_types, &mut sig);

    // Extract results
    let errors = engine.take_errors();
    let warnings = engine.take_warnings();
    let pat_resolutions = engine.take_pattern_resolutions();
    let mono_instances = engine.take_mono_instances();
    let mono_dispatch_pre_dedup = engine.take_mono_dispatch_pre_dedup();
    let deferred_mono_calls = engine.take_deferred_mono_calls();

    // Shared PC-2 validation + store/push/accumulate spine (§03.1–§03.4).
    super::finalize_body_and_export(
        checker,
        &sig,
        test.span,
        super::BodyOutputs {
            expr_types,
            errors,
            warnings,
            pat_resolutions,
            mono_instances,
            mono_dispatch_pre_dedup,
            deferred_mono_calls,
        },
    );

    // write the defaulted test signature back so the hash channel
    // (cross-module identity) reflects post-defaulted types, matching
    // check_function's behavior.
    checker.signatures.insert(test.name, sig);
}
