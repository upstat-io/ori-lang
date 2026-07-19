//! Function and test body type checking.
//!
//! Owns `check_function_bodies` (Pass 2) and `check_test_bodies` (Pass 3) and
//! their private body-checking helpers. See `bodies/mod.rs` for the architecture
//! docstring that covers all four body passes.

use ori_ir::{ExprArena, Function, Module, Name, Span, TestDef};
use rustc_hash::FxHashSet;

use super::contracts::{validate_post_contracts, validate_pre_contracts};
use crate::check::validators::build_exempt_var_ids;
use crate::check::ModuleChecker;
use crate::output::FunctionSig;
use crate::{
    check_expr, infer_expr, ContextKind, Expected, ExpectedOrigin, Idx, InferEngine, TypeEnv,
};

/// Check all function bodies.
///
/// This pass runs after signature collection (Pass 1). Each function body
/// is type-checked against its declared return type.
#[tracing::instrument(level = "debug", skip_all, fields(count = module.functions.len()))]
pub(in crate::check) fn check_function_bodies(checker: &mut ModuleChecker<'_>, module: &Module) {
    for func in &module.functions {
        check_function(checker, func);
    }
}

/// Type check a single function body.
fn check_function(checker: &mut ModuleChecker<'_>, func: &Function) {
    let Some(mut sig) = checker.get_signature(func.name).cloned() else {
        checker.error_undefined(func.name, func.span);
        return;
    };
    let Some(setup) = prepare_function(checker, func, &sig) else {
        return;
    };

    let outputs = infer_function_body(checker, func, &mut sig, setup);
    super::finalize_body_and_export(checker, &sig, func.span, func.body, outputs);
    checker.signatures.insert(func.name, sig);
}

struct FunctionSetup {
    param_env: TypeEnv,
    fn_type: Idx,
    capabilities: FxHashSet<Name>,
    guard_span: Option<Span>,
    body_span: Span,
    capability_var_bounds: Vec<(u32, Name)>,
    capability_var_ids: Vec<u32>,
    exempt_var_ids: FxHashSet<u32>,
}

fn prepare_function(
    checker: &mut ModuleChecker<'_>,
    func: &Function,
    sig: &FunctionSig,
) -> Option<FunctionSetup> {
    let mut exempt_var_ids = build_exempt_var_ids(checker.pool(), &sig.scheme_var_ids);
    let mut param_env = checker.child_of_base()?;

    for (&name, &ty) in sig.param_names.iter().zip(&sig.param_types) {
        param_env.bind(name, ty);
    }
    for param in &sig.const_params {
        param_env.bind(param.name, param.const_type);
    }

    let mut capability_var_bounds = Vec::new();
    let mut capability_var_ids = Vec::new();
    for &capability in &sig.capabilities {
        let retained = sig
            .capability_params
            .iter()
            .copied()
            .find(|param| param.capability() == capability)
            .and_then(crate::CapabilityParam::provider);
        let (capability_ty, var_id) = if let Some((provider_type, provider_var_id)) = retained {
            (provider_type, provider_var_id)
        } else {
            let capability_ty = checker.pool_mut().fresh_var();
            (capability_ty, checker.pool().data(capability_ty))
        };
        param_env.bind(capability, capability_ty);
        capability_var_bounds.push((var_id, capability));
        capability_var_ids.push(var_id);
    }
    exempt_var_ids.extend(capability_var_ids.iter().copied());

    let fn_type = checker
        .pool_mut()
        .function(&sig.param_types, sig.return_type);
    let capabilities = sig.capabilities.iter().copied().collect();
    let guard_span = func.guard.map(|id| checker.arena().get_expr(id).span);
    let body_span = checker.arena().get_expr(func.body).span;

    Some(FunctionSetup {
        param_env,
        fn_type,
        capabilities,
        guard_span,
        body_span,
        capability_var_bounds,
        capability_var_ids,
        exempt_var_ids,
    })
}

fn infer_function_body(
    checker: &mut ModuleChecker<'_>,
    func: &Function,
    sig: &mut FunctionSig,
    setup: FunctionSetup,
) -> super::BodyOutputs {
    let FunctionSetup {
        param_env,
        fn_type,
        capabilities,
        guard_span,
        body_span,
        capability_var_bounds,
        capability_var_ids,
        exempt_var_ids,
    } = setup;

    checker.with_function_scope(fn_type, capabilities, |checker| {
        let arena = checker.arena();
        let mut engine = checker.create_engine_with_env(param_env);
        configure_function_engine(&mut engine, fn_type, func.name, sig, &capability_var_bounds);
        check_function_expressions(&mut engine, arena, func, sig, guard_span, body_span);
        finish_function_inference(&mut engine, arena, sig, &exempt_var_ids, capability_var_ids)
    })
}

fn configure_function_engine(
    engine: &mut InferEngine<'_>,
    fn_type: Idx,
    func_name: Name,
    sig: &FunctionSig,
    capability_var_bounds: &[(u32, Name)],
) {
    engine.set_self_type(fn_type);
    engine.set_deferred_mono_caller(crate::DeferredMonoCaller::TopLevel(func_name), Vec::new());
    engine.set_capability_parameters(&sig.capability_params);
    for param in &sig.const_params {
        engine.bind_const_param(param.name, param.const_type);
    }

    for (index, &var_id) in sig.scheme_var_ids.iter().enumerate() {
        if let Some(bounds) = sig.type_param_bounds.get(index) {
            for &trait_name in bounds {
                engine.bind_rigid_bound_by_var_id(var_id, trait_name);
            }
        }
    }
    for clause in &sig.where_clauses {
        if clause.projection.is_some() {
            continue;
        }
        let Some(index) = sig
            .type_params
            .iter()
            .position(|&name| name == clause.param)
        else {
            continue;
        };
        let Some(&var_id) = sig.scheme_var_ids.get(index) else {
            continue;
        };
        for &trait_name in &clause.bounds {
            engine.bind_rigid_bound_by_var_id(var_id, trait_name);
        }
    }
    for &(var_id, capability) in capability_var_bounds {
        engine.bind_rigid_bound_by_var_id(var_id, capability);
    }
}

fn check_function_expressions(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    func: &Function,
    sig: &FunctionSig,
    guard_span: Option<Span>,
    body_span: Span,
) {
    engine.push_context(ContextKind::FunctionReturn {
        func_name: Some(func.name),
    });

    if let (Some(guard_id), Some(span)) = (func.guard, guard_span) {
        let guard_ty = infer_expr(engine, arena, guard_id);
        let expected = Expected {
            ty: Idx::BOOL,
            origin: ExpectedOrigin::Context {
                span,
                kind: ContextKind::MatchArmGuard { arm_index: 0 },
            },
        };
        let _ = engine.check_type(guard_ty, &expected, span);
    }

    validate_pre_contracts(engine, arena, func);
    let expected = Expected {
        ty: sig.return_type,
        origin: ExpectedOrigin::Context {
            span: body_span,
            kind: ContextKind::FunctionReturn {
                func_name: Some(func.name),
            },
        },
    };
    let _ = check_expr(engine, arena, func.body, &expected, body_span);
    check_parameter_defaults(engine, arena, func, sig);
    validate_post_contracts(engine, arena, func, sig.return_type);

    engine.pop_context();
    engine.mark_body_inference_complete();
}

fn check_parameter_defaults(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    func: &Function,
    sig: &FunctionSig,
) {
    for (index, param) in arena.get_params(func.params).iter().enumerate() {
        let (Some(default_id), Some(&param_ty)) = (param.default, sig.param_types.get(index))
        else {
            continue;
        };
        let span = arena.get_expr(default_id).span;
        let expected = Expected {
            ty: param_ty,
            origin: ExpectedOrigin::Annotation {
                name: param.name,
                span,
                const_terms: Vec::new(),
            },
        };
        let _ = check_expr(engine, arena, default_id, &expected, span);
    }
}

fn finish_function_inference(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    sig: &mut FunctionSig,
    exempt_var_ids: &FxHashSet<u32>,
    capability_var_ids: Vec<u32>,
) -> super::BodyOutputs {
    let mut expr_types = engine.take_expr_types();
    engine.default_unbound_vars_from_empty_literals(arena, &mut expr_types, sig, exempt_var_ids);
    engine.normalize_body_generalized_to_bound_var_sig(&mut expr_types, sig);
    engine.compose_body_type_burdens(&expr_types);

    super::BodyOutputs {
        expr_types,
        errors: engine.take_errors(),
        warnings: engine.take_warnings(),
        pat_resolutions: engine.take_pattern_resolutions(),
        mono_instances: engine.take_mono_instances(),
        mono_dispatch_pre_dedup: engine.take_mono_dispatch_pre_dedup(),
        index_dispatch_selections: engine.take_index_dispatch_selections(),
        deferred_mono_calls: engine.take_deferred_mono_calls(),
        composed_burdens: engine.take_composed_burdens(),
        capability_exempt_var_ids: capability_var_ids,
        assign_desugars: engine.take_assign_desugars(),
        module_alias_calls: engine.take_module_alias_calls(),
        iter_route_desugars: engine.take_iter_routes(),
        capability_calls: engine.take_capability_calls(),
    }
}

/// Check all test bodies.
///
/// Tests are similar to functions but:
/// - Always return unit (void)
/// - May have special test parameters
#[tracing::instrument(level = "debug", skip_all, fields(count = module.tests.len()))]
pub(in crate::check) fn check_test_bodies(checker: &mut ModuleChecker<'_>, module: &Module) {
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

    let Some(child_env) = checker.child_of_base() else {
        return;
    };

    // Bind parameters
    let mut param_env = child_env;
    for (name, ty) in sig.param_names.iter().zip(&sig.param_types) {
        param_env.bind(*name, *ty);
    }

    // Why: precompute exemptions to avoid an inference-to-validator dependency.
    let exempt = build_exempt_var_ids(checker.pool(), &sig.scheme_var_ids);

    let arena = checker.arena();

    let fn_type = checker
        .pool_mut()
        .function(&sig.param_types, sig.return_type);
    let mut engine = checker.create_engine_with_env(param_env);
    engine.set_self_type(fn_type);

    // INVARIANT: test bodies supply the caller identity required by deferred mono calls.
    engine.set_deferred_mono_caller(crate::DeferredMonoCaller::TopLevel(test.name), Vec::new());

    engine.push_context(ContextKind::TestBody);

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

    // Mark body inference complete before the defaulting pre-pass runs;
    // defaulting helpers debug-assert this flag (see `check_function`).
    engine.mark_body_inference_complete();

    // default unbound vars reachable from empty-literal expr roots before
    // exporting types, so tests with empty literals type-check without E2005.
    let mut expr_types = engine.take_expr_types();
    engine.default_unbound_vars_from_empty_literals(arena, &mut expr_types, &mut sig, &exempt);

    // Normalize scheme vars to `Tag::BoundVar`.
    // See `check_function` for the full rationale.
    engine.normalize_body_generalized_to_bound_var_sig(&mut expr_types, &mut sig);

    // Deep-resolve var-links so late-resolved generic-builtin instantiations are
    // var-free in the exported IR and composed by the burden sweep, matching
    // the main-body path and `intern_link_resolved_body_types`.
    engine.compose_body_type_burdens(&expr_types);

    let errors = engine.take_errors();
    let warnings = engine.take_warnings();
    let pat_resolutions = engine.take_pattern_resolutions();
    let mono_instances = engine.take_mono_instances();
    let mono_dispatch_pre_dedup = engine.take_mono_dispatch_pre_dedup();
    let index_dispatch_selections = engine.take_index_dispatch_selections();
    let deferred_mono_calls = engine.take_deferred_mono_calls();
    let composed_burdens = engine.take_composed_burdens();
    let assign_desugars = engine.take_assign_desugars();
    let module_alias_calls = engine.take_module_alias_calls();
    let iter_route_desugars = engine.take_iter_routes();
    let capability_calls = engine.take_capability_calls();

    // Shared PC-2 validation + store/push/accumulate spine.
    super::finalize_body_and_export(
        checker,
        &sig,
        test.span,
        test.body,
        super::BodyOutputs {
            expr_types,
            errors,
            warnings,
            pat_resolutions,
            mono_instances,
            mono_dispatch_pre_dedup,
            index_dispatch_selections,
            deferred_mono_calls,
            composed_burdens,
            capability_exempt_var_ids: Vec::new(),
            assign_desugars,
            module_alias_calls,
            iter_route_desugars,
            capability_calls,
        },
    );

    // write the defaulted test signature back so the hash channel
    // (cross-module identity) reflects post-defaulted types, matching
    // check_function's behavior.
    checker.signatures.insert(test.name, sig);
}
