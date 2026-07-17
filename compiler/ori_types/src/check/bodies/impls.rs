//! Impl method body type checking.
//!
//! Owns `check_impl_bodies` (Pass 4), its block/method helpers. See
//! `bodies/mod.rs` for the architecture docstring that covers all four body passes.

use ori_ir::{ExprArena, ExprId, ImplMethod, Module, Name, Param, Span, TraitDef, TraitItem};
use rustc_hash::{FxHashMap, FxHashSet};

use super::method_sig::{allocate_generic_binders, build_method_sig};
use crate::check::registration::resolve_type_with_method_generics;
use crate::check::ModuleChecker;
use crate::output::ConstParamInfo;
use crate::{
    check_expr, ContextKind, Expected, ExpectedOrigin, Idx, ImplMethodId, InferEngine, TypeEnv,
};

/// Check all impl method bodies.
///
/// For trait impls, this also checks unoverridden default methods from the trait
/// definition, registering their signatures for LLVM codegen.
#[tracing::instrument(level = "debug", skip_all, fields(count = module.impls.len()))]
pub(in crate::check) fn check_impl_bodies(checker: &mut ModuleChecker<'_>, module: &Module) {
    for (impl_index, impl_def) in module.impls.iter().enumerate() {
        check_impl_block(checker, impl_def, &module.traits, impl_index);
    }
}

/// Type check methods in an impl block.
///
/// Processes explicit methods first, then unoverridden default methods from the
/// trait definition. Both register signatures via `register_impl_sig` for LLVM
/// codegen consumption (signatures are consumed positionally by `compile_impls`).
fn check_impl_block(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
    traits: &[TraitDef],
    impl_index: usize,
) {
    // Impl-level generics (`impl<T: Bound>`) bind as `RigidVar`s so a
    // body-internal `receiver.method()` whose receiver is an impl-level type
    // parameter reaches the bound-chain (bounded calls dispatch, unbounded
    // calls surface method-not-found) instead of resolving to an unresolved
    // `Tag::Named`. The `RigidVar`s are
    // allocated at `register_impls` (Pass 0c) and stored on the checker keyed by
    // `module.impls` position; REUSE them here (Pass 4) via `prealloc` so a
    // method mono recorded at a Pass-3 call site already sees the impl binder in
    // `var_states` (the constructor composite `Pair<RigidVar(B), RigidVar(A)>`
    // registers because the binder exists by Pass 0c, not Pass 4). `Self` +
    // params + return resolve through the same overlay, keeping every reference
    // to the impl-level binder at one identity.
    let impl_prealloc: Option<FxHashMap<Name, Idx>> =
        checker.impl_rigid_var_map(impl_index).cloned();
    let (impl_substitutions, impl_generic_params, _impl_const_params, impl_inline_bounds) =
        allocate_generic_binders(checker, impl_def.generics, impl_prealloc.as_ref());

    // Resolve the Self type for this impl block through the impl-level overlay so
    // `Box<T>`'s `T` is the impl `RigidVar`, not a fresh `Tag::Named`.
    let self_type = resolve_type_with_method_generics(
        checker,
        &impl_def.self_ty,
        &impl_substitutions,
        &impl_generic_params,
        Idx::ERROR,
    );

    let is_trait_impl = impl_def.trait_path.is_some();

    // Build this impl's associated-type bindings (`assoc_name → Idx`) so a
    // method whose declared param/return carries `Self.Item` resolves the
    // projection at body-check time too (the registration arm resolves the
    // registered signature; body-check re-resolves the declared type). Mirrors
    // the registration scope in `register_impl`.
    let trait_idx = impl_def
        .trait_path
        .as_ref()
        .and_then(|path| path.last().copied())
        .map(|trait_name| checker.pool_mut().named(trait_name));
    let mut assoc_bindings: FxHashMap<Name, Idx> = FxHashMap::default();
    for impl_assoc in &impl_def.assoc_types {
        let ty = resolve_type_with_method_generics(
            checker,
            &impl_assoc.ty,
            &impl_substitutions,
            &impl_generic_params,
            self_type,
        );
        assoc_bindings.insert(impl_assoc.name, ty);
    }

    let impl_context = ImplBodyContext {
        impl_index,
        self_type,
        trait_type: trait_idx,
        type_params: &impl_generic_params,
        substitutions: &impl_substitutions,
        inline_bounds: &impl_inline_bounds,
    };

    checker.with_impl_assoc_scope(assoc_bindings, trait_idx, |checker| {
        // Check explicitly defined methods
        for method in &impl_def.methods {
            check_impl_method(checker, method, &impl_context);
            if is_trait_impl {
                checker.register_trait_impl_fn_name(self_type, method.name);
            }
        }

        // For trait impls, also check unoverridden default methods.
        // This registers their signatures so LLVM codegen can compile them.
        if let Some(trait_path) = &impl_def.trait_path {
            if let Some(&trait_name) = trait_path.last() {
                let overridden: FxHashSet<Name> = impl_def.methods.iter().map(|m| m.name).collect();

                if let Some(trait_def) = traits.iter().find(|t| t.name == trait_name) {
                    for item in &trait_def.items {
                        if let TraitItem::DefaultMethod(default) = item {
                            if !overridden.contains(&default.name) {
                                let as_impl = ImplMethod::from(default);
                                check_impl_method(checker, &as_impl, &impl_context);
                                checker.register_trait_impl_fn_name(self_type, default.name);
                            }
                        }
                    }
                }
            }
        }
    });
}

/// Impl-level inputs shared by every method body in one impl block.
struct ImplBodyContext<'a> {
    impl_index: usize,
    self_type: Idx,
    trait_type: Option<Idx>,
    type_params: &'a [Name],
    substitutions: &'a FxHashMap<Name, Idx>,
    inline_bounds: &'a [(Idx, Vec<Name>)],
}

/// Type check a single impl method body.
fn check_impl_method(
    checker: &mut ModuleChecker<'_>,
    method: &ImplMethod,
    impl_context: &ImplBodyContext<'_>,
) {
    let method_id = ImplMethodId::new(impl_context.impl_index, method.body);
    let role = checker.impl_method_role(method_id);
    let Some(mut setup) = prepare_impl_method(checker, method, impl_context) else {
        return;
    };

    let outputs = infer_impl_method(checker, method, impl_context, &mut setup);
    let sig = build_method_sig(
        method.name,
        &setup.params,
        setup.param_types,
        setup.return_type,
        &setup.combined_type_params,
        setup.method_const_params,
        checker.pool(),
    );

    super::finalize_body_and_export(checker, &sig, method.span, method.body, outputs);
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

    let mut combined_substitutions = impl_context.substitutions.clone();
    combined_substitutions.extend(method_substitutions);
    let combined_type_params = impl_context
        .type_params
        .iter()
        .copied()
        .chain(method_generic_params)
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
    impl_context: &ImplBodyContext<'_>,
    setup: &mut MethodSetup,
) -> super::BodyOutputs {
    checker.with_impl_scope(impl_context.self_type, |checker| {
        checker.with_function_scope(setup.fn_type, FxHashSet::default(), |checker| {
            let arena = checker.arena();
            let mut engine = checker.create_engine_with_env(setup.param_env.clone());
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
        engine.bind_method_const(param.name, param.const_type);
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
) -> super::BodyOutputs {
    let mut expr_types = engine.take_expr_types();
    engine.default_unbound_vars_in_scope(
        arena,
        &mut expr_types,
        param_types,
        return_type,
        &FxHashSet::default(),
    );
    engine.normalize_body_generalized_to_bound_var(&mut expr_types, param_types, return_type, &[]);
    engine.exit_rank_scope();
    engine.compose_body_type_burdens(&expr_types);

    super::BodyOutputs {
        expr_types,
        errors: engine.take_errors(),
        warnings: engine.take_warnings(),
        pat_resolutions: engine.take_pattern_resolutions(),
        mono_instances: engine.take_mono_instances(),
        mono_dispatch_pre_dedup: engine.take_mono_dispatch_pre_dedup(),
        deferred_mono_calls: engine.take_deferred_mono_calls(),
        composed_burdens: engine.take_composed_burdens(),
        capability_exempt_var_ids: Vec::new(),
        assign_desugars: engine.take_assign_desugars(),
        module_alias_calls: engine.take_module_alias_calls(),
        iter_route_desugars: engine.take_iter_routes(),
    }
}
