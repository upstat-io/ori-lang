//! Def-impl (default implementation) method body type checking.
//!
//! Owns `check_def_impl_bodies` (Pass 5) and its block/method helpers. See
//! `bodies/mod.rs` for the architecture docstring that covers all four body passes.

use ori_ir::{ImplMethod, Module, Name, Param, Span};
use rustc_hash::FxHashSet;

use super::method_sig::{allocate_method_binders, build_method_sig};
use crate::check::registration::resolve_type_with_method_generics;
use crate::check::ModuleChecker;
use crate::{
    check_expr, ConstParamInfo, ContextKind, Expected, ExpectedOrigin, Idx, InferEngine, TypeEnv,
};

struct DefImplSetup {
    params: Vec<Param>,
    param_env: TypeEnv,
    param_types: Vec<Idx>,
    return_type: Idx,
    const_params: Vec<ConstParamInfo>,
    inline_bounds: Vec<(Idx, Vec<Name>)>,
    self_rigid: Idx,
    fn_type: Idx,
    body_span: Span,
}

struct DefImplEngineInput<'a> {
    method: &'a ImplMethod,
    def_impl_trait: Name,
    param_env: TypeEnv,
    param_types: &'a mut Vec<Idx>,
    return_type: &'a mut Idx,
    const_params: &'a [ConstParamInfo],
    inline_bounds: &'a [(Idx, Vec<Name>)],
    self_rigid: Idx,
    body_span: Span,
}

/// Check all def impl method bodies.
#[tracing::instrument(level = "debug", skip_all, fields(count = module.def_impls.len()))]
pub(in crate::check) fn check_def_impl_bodies(checker: &mut ModuleChecker<'_>, module: &Module) {
    for def_impl in &module.def_impls {
        check_def_impl_block(checker, def_impl);
    }
}

/// Type check methods in a def impl block.
///
/// `def impl` methods are stateless (no `self` parameter) and don't have
/// a `for Type` clause. They provide default behavior for a capability trait.
fn check_def_impl_block(checker: &mut ModuleChecker<'_>, def_impl: &ori_ir::DefImplDef) {
    for method in &def_impl.methods {
        check_def_impl_method(checker, method, def_impl.trait_name);
    }
}

/// Type check a single def impl method body.
fn check_def_impl_method(
    checker: &mut ModuleChecker<'_>,
    method: &ImplMethod,
    def_impl_trait: ori_ir::Name,
) {
    // Create child environment from frozen base
    let Some(child_env) = checker.child_of_base() else {
        return;
    };

    let DefImplSetup {
        params,
        param_env,
        mut param_types,
        mut return_type,
        const_params: method_const_params,
        inline_bounds: method_inline_bounds,
        self_rigid,
        fn_type,
        body_span,
    } = prepare_def_impl(checker, method, child_env);

    let outputs = checker.with_impl_scope(self_rigid, |checker| {
        checker.with_function_scope(fn_type, FxHashSet::default(), |checker| {
            infer_def_impl_body(
                checker,
                DefImplEngineInput {
                    method,
                    def_impl_trait,
                    param_env,
                    param_types: &mut param_types,
                    return_type: &mut return_type,
                    const_params: &method_const_params,
                    inline_bounds: &method_inline_bounds,
                    self_rigid,
                    body_span,
                },
            )
        })
    });

    finalize_def_impl(
        checker,
        method,
        &params,
        param_types,
        return_type,
        method_const_params,
        outputs,
    );
}

fn infer_def_impl_body(
    checker: &mut ModuleChecker<'_>,
    input: DefImplEngineInput<'_>,
) -> super::BodyOutputs {
    let arena = checker.arena();
    let mut engine = checker.create_engine_with_env(input.param_env);
    bind_def_impl_engine(
        &mut engine,
        input.const_params,
        input.inline_bounds,
        input.self_rigid,
        input.def_impl_trait,
    );
    engine.push_context(ContextKind::FunctionReturn {
        func_name: Some(input.method.name),
    });
    let expected = Expected {
        ty: *input.return_type,
        origin: ExpectedOrigin::Context {
            span: input.body_span,
            kind: ContextKind::FunctionReturn {
                func_name: Some(input.method.name),
            },
        },
    };
    let _body_ty = check_expr(
        &mut engine,
        arena,
        input.method.body,
        &expected,
        input.body_span,
    );
    engine.pop_context();
    engine.mark_body_inference_complete();

    let mut expr_types = engine.take_expr_types();
    engine.default_unbound_vars_in_scope(
        arena,
        &mut expr_types,
        input.param_types,
        input.return_type,
        &FxHashSet::default(),
    );
    engine.normalize_body_generalized_to_bound_var(
        &mut expr_types,
        input.param_types,
        input.return_type,
        &[],
    );
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
        capability_exempt_var_ids: Vec::new(),
        assign_desugars: engine.take_assign_desugars(),
        module_alias_calls: engine.take_module_alias_calls(),
        iter_route_desugars: engine.take_iter_routes(),
        capability_calls: engine.take_capability_calls(),
    }
}

fn finalize_def_impl(
    checker: &mut ModuleChecker<'_>,
    method: &ImplMethod,
    params: &[Param],
    param_types: Vec<Idx>,
    return_type: Idx,
    const_params: Vec<ConstParamInfo>,
    outputs: super::BodyOutputs,
) {
    let signature = build_method_sig(
        method.name,
        params,
        param_types,
        return_type,
        &[],
        const_params,
        checker.pool(),
    );
    super::finalize_body_and_export(checker, &signature, method.span, method.body, outputs);
}

fn bind_def_impl_engine(
    engine: &mut InferEngine<'_>,
    const_params: &[ConstParamInfo],
    inline_bounds: &[(Idx, Vec<Name>)],
    self_rigid: Idx,
    def_impl_trait: Name,
) {
    for param in const_params {
        engine.bind_const_param(param.name, param.const_type);
    }
    for (rigid_idx, trait_names) in inline_bounds {
        for &trait_name in trait_names {
            engine.bind_method_rigid_bound(*rigid_idx, trait_name);
        }
    }
    engine.bind_method_rigid_bound(self_rigid, def_impl_trait);
}

fn resolve_def_impl_parameters(
    checker: &mut ModuleChecker<'_>,
    method: &ImplMethod,
    substitutions: &rustc_hash::FxHashMap<Name, Idx>,
    generic_params: &[Name],
    self_rigid: Idx,
    mut env: TypeEnv,
) -> (Vec<Param>, TypeEnv, Vec<Idx>) {
    let params = checker.arena().get_params(method.params).to_vec();
    let self_kw = checker.well_known().self_kw;
    let mut types = Vec::with_capacity(params.len());
    for param in &params {
        let ty = match &param.ty {
            Some(parsed) => resolve_type_with_method_generics(
                checker,
                parsed,
                substitutions,
                generic_params,
                self_rigid,
            ),
            None if param.name == self_kw => self_rigid,
            None => checker.pool_mut().fresh_var(),
        };
        env.bind(param.name, ty);
        types.push(ty);
    }
    (params, env, types)
}

fn prepare_def_impl(
    checker: &mut ModuleChecker<'_>,
    method: &ImplMethod,
    child_env: TypeEnv,
) -> DefImplSetup {
    let (substitutions, generic_params, const_params, inline_bounds) =
        allocate_method_binders(checker, method);
    let self_rigid = checker.pool_mut().rigid_var(Name::EMPTY);
    let (params, mut param_env, param_types) = resolve_def_impl_parameters(
        checker,
        method,
        &substitutions,
        &generic_params,
        self_rigid,
        child_env,
    );
    let return_type = resolve_type_with_method_generics(
        checker,
        &method.return_ty,
        &substitutions,
        &generic_params,
        self_rigid,
    );
    for (&name, &rigid_idx) in &substitutions {
        param_env.bind(name, rigid_idx);
    }
    for param in &const_params {
        param_env.bind(param.name, param.const_type);
    }
    let fn_type = checker.pool_mut().function(&param_types, return_type);
    let body_span = checker.arena().get_expr(method.body).span;
    DefImplSetup {
        params,
        param_env,
        param_types,
        return_type,
        const_params,
        inline_bounds,
        self_rigid,
        fn_type,
        body_span,
    }
}
