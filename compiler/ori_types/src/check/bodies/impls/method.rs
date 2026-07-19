//! Per-method setup, inference, and typed-body export.

use ori_ir::{ExprArena, ExprId, ImplMethod, Name, Param, Span};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::check::registration::resolve_type_with_method_generics;
use crate::check::ModuleChecker;
use crate::output::ConstParamInfo;
use crate::{
    check_expr, ContextKind, Expected, ExpectedOrigin, Idx, ImplMethodId, InferEngine, TypeEnv,
};

use super::super::method_sig::{allocate_generic_binders, build_method_sig};
use super::ImplBodyContext;

/// Type check a single impl method body.
pub(super) fn check_impl_method(
    checker: &mut ModuleChecker<'_>,
    method: &ImplMethod,
    impl_context: &ImplBodyContext<'_>,
) {
    let method_id = ImplMethodId::new(impl_context.impl_index, method.body);
    let role = checker.impl_method_role(method_id);
    let Some(mut setup) = prepare_impl_method(checker, method, impl_context) else {
        return;
    };

    let outputs = infer_impl_method(checker, method, method_id, impl_context, &mut setup);
    let sig = build_method_sig(
        method.name,
        &setup.params,
        setup.param_types,
        setup.return_type,
        &setup.combined_type_params,
        setup.method_const_params,
        checker.pool(),
    );

    super::super::finalize_body_and_export(checker, &sig, method.span, method.body, outputs);
    checker.register_impl_sig(
        method_id,
        impl_context.self_type,
        impl_context.trait_type,
        method.name,
        role,
        sig,
    );
}

struct MethodSetup {
    params: Vec<Param>,
    param_env: TypeEnv,
    param_types: Vec<Idx>,
    return_type: Idx,
    combined_type_params: Vec<Name>,
    caller_binder_roots: Vec<Idx>,
    method_const_params: Vec<ConstParamInfo>,
    method_inline_bounds: Vec<(Idx, Vec<Name>)>,
    impl_inline_bounds: Vec<(Idx, Vec<Name>)>,
    fn_type: Idx,
    body_span: Span,
    default_checks: Vec<(ExprId, Name, Idx)>,
}

fn prepare_impl_method(
    checker: &mut ModuleChecker<'_>,
    method: &ImplMethod,
    impl_context: &ImplBodyContext<'_>,
) -> Option<MethodSetup> {
    let mut param_env = checker.child_of_base()?;
    let preallocated = checker.method_rigid_var_map_for(method.body).cloned();
    let (method_substitutions, method_generic_params, method_const_params, method_inline_bounds) =
        allocate_generic_binders(checker, method.generics, preallocated.as_ref());

    // Preserve both declaration axes before the method overlay is merged.
    // A method binder may shadow an impl binder with the same spelling; the
    // exact rigid roots must remain distinct and ordered impl-first.
    let caller_binder_roots = impl_context
        .type_params
        .iter()
        .filter_map(|name| impl_context.substitutions.get(name).copied())
        .chain(
            method_generic_params
                .iter()
                .filter_map(|name| method_substitutions.get(name).copied()),
        )
        .collect::<Vec<_>>();
    let mut combined_substitutions = impl_context.substitutions.clone();
    combined_substitutions.extend(method_substitutions);
    let combined_type_params = impl_context
        .type_params
        .iter()
        .copied()
        .chain(method_generic_params.iter().copied())
        .collect::<Vec<_>>();
    let params = checker.arena().get_params(method.params).to_vec();
    let param_types = resolve_method_parameters(
        checker,
        &params,
        &combined_substitutions,
        &combined_type_params,
        impl_context.self_type,
        &mut param_env,
    );
    let default_checks = params
        .iter()
        .zip(&param_types)
        .filter_map(|(param, &ty)| param.default.map(|id| (id, param.name, ty)))
        .collect();
    let return_type = resolve_type_with_method_generics(
        checker,
        &method.return_ty,
        &combined_substitutions,
        &combined_type_params,
        impl_context.self_type,
    );

    for (&name, &ty) in &combined_substitutions {
        param_env.bind(name, ty);
    }
    for param in &method_const_params {
        param_env.bind(param.name, param.const_type);
    }

    let fn_type = checker.pool_mut().function(&param_types, return_type);
    let body_span = checker.arena().get_expr(method.body).span;
    Some(MethodSetup {
        params,
        param_env,
        param_types,
        return_type,
        combined_type_params,
        caller_binder_roots,
        method_const_params,
        method_inline_bounds,
        impl_inline_bounds: impl_context.inline_bounds.to_vec(),
        fn_type,
        body_span,
        default_checks,
    })
}

fn resolve_method_parameters(
    checker: &mut ModuleChecker<'_>,
    params: &[Param],
    substitutions: &FxHashMap<Name, Idx>,
    type_params: &[Name],
    self_type: Idx,
    param_env: &mut TypeEnv,
) -> Vec<Idx> {
    params
        .iter()
        .map(|param| {
            let ty = match &param.ty {
                Some(parsed) => resolve_type_with_method_generics(
                    checker,
                    parsed,
                    substitutions,
                    type_params,
                    self_type,
                ),
                None if param.name == checker.well_known().self_kw => self_type,
                None => checker.pool_mut().fresh_var(),
            };
            param_env.bind(param.name, ty);
            ty
        })
        .collect()
}

fn infer_impl_method(
    checker: &mut ModuleChecker<'_>,
    method: &ImplMethod,
    method_id: ImplMethodId,
    impl_context: &ImplBodyContext<'_>,
    setup: &mut MethodSetup,
) -> super::super::BodyOutputs {
    checker.with_impl_scope(impl_context.self_type, |checker| {
        checker.with_function_scope(setup.fn_type, FxHashSet::default(), |checker| {
            let arena = checker.arena();
            let mut engine = checker.create_engine_with_env(setup.param_env.clone());
            engine.set_deferred_mono_caller(
                crate::DeferredMonoCaller::ImplMethod {
                    name: method.name,
                    id: method_id,
                },
                setup.caller_binder_roots.clone(),
            );
            configure_method_engine(
                &mut engine,
                &setup.method_const_params,
                &setup.method_inline_bounds,
                &setup.impl_inline_bounds,
            );
            check_method_expressions(
                &mut engine,
                arena,
                method,
                setup.return_type,
                setup.body_span,
                &setup.default_checks,
            );
            finish_method_inference(
                &mut engine,
                arena,
                &mut setup.param_types,
                &mut setup.return_type,
            )
        })
    })
}

fn configure_method_engine(
    engine: &mut InferEngine<'_>,
    const_params: &[ConstParamInfo],
    method_bounds: &[(Idx, Vec<Name>)],
    impl_bounds: &[(Idx, Vec<Name>)],
) {
    for param in const_params {
        engine.bind_const_param(param.name, param.const_type);
    }
    for (rigid, trait_names) in method_bounds.iter().chain(impl_bounds) {
        for &trait_name in trait_names {
            engine.bind_method_rigid_bound(*rigid, trait_name);
        }
    }
    engine.enter_rank_scope();
}

fn check_method_expressions(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    method: &ImplMethod,
    return_type: Idx,
    body_span: Span,
    default_checks: &[(ExprId, Name, Idx)],
) {
    engine.push_context(ContextKind::FunctionReturn {
        func_name: Some(method.name),
    });
    let expected = Expected {
        ty: return_type,
        origin: ExpectedOrigin::Context {
            span: body_span,
            kind: ContextKind::FunctionReturn {
                func_name: Some(method.name),
            },
        },
    };
    let _ = check_expr(engine, arena, method.body, &expected, body_span);
    engine.pop_context();

    for &(default_id, param_name, param_ty) in default_checks {
        let span = arena.get_expr(default_id).span;
        let expected = Expected {
            ty: param_ty,
            origin: ExpectedOrigin::Annotation {
                name: param_name,
                span,
                const_terms: Vec::new(),
            },
        };
        let _ = check_expr(engine, arena, default_id, &expected, span);
    }
    engine.mark_body_inference_complete();
}

fn finish_method_inference(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    param_types: &mut [Idx],
    return_type: &mut Idx,
) -> super::super::BodyOutputs {
    let mut expr_types = engine.take_expr_types();
    engine.default_unbound_vars_in_scope(
        arena,
        &mut expr_types,
        param_types,
        return_type,
        &FxHashSet::default(),
    );
    engine.normalize_body_generalized_to_bound_var(&mut expr_types, param_types, return_type, &[]);
    engine.materialize_body_type_sites(&mut expr_types, param_types, return_type);
    engine.exit_rank_scope();
    engine.compose_body_type_burdens(&expr_types);

    super::super::BodyOutputs {
        expr_types,
        errors: engine.take_errors(),
        warnings: engine.take_warnings(),
        pat_resolutions: engine.take_pattern_resolutions(),
        mono_instances: engine.take_mono_instances(),
        mono_dispatch_pre_dedup: engine.take_mono_dispatch_pre_dedup(),
        index_dispatch_selections: engine.take_index_dispatch_selections(),
        deferred_mono_calls: engine.take_deferred_mono_calls(),
        composed_burdens: engine.take_composed_burdens(),
        capability_exempt_var_ids: Vec::new(),
        assign_desugars: engine.take_assign_desugars(),
        module_alias_calls: engine.take_module_alias_calls(),
        iter_route_desugars: engine.take_iter_routes(),
        capability_calls: engine.take_capability_calls(),
    }
}
