//! Impl and def-impl method body type checking (Passes 4 and 5 per `typeck.md` CK-1).
//!
//! Owns `check_impl_bodies` (Pass 4), `check_def_impl_bodies` (Pass 5), their
//! block/method helpers, and the shared `build_method_sig` constructor. See
//! `bodies/mod.rs` for the architecture docstring that covers all four body passes.

use ori_ir::{ExprId, ImplMethod, Module, Name, Param, TraitDef, TraitItem};
use rustc_hash::FxHashSet;

use crate::check::registration::{resolve_parsed_type_simple, resolve_type_with_self};
use crate::check::ModuleChecker;
use crate::{check_expr, ContextKind, Expected, ExpectedOrigin, FunctionSig, Idx, Pool};

/// Build a `FunctionSig` from the resolved method parameters and return type.
///
/// Used by both `check_impl_method` and `check_def_impl_method` to construct the
/// signature eagerly enough for `PC-2` validator coverage of param/return
/// `Tag::Var` positions. The method body is not yet checked when this helper runs —
/// `param_types` and `return_type` are the types resolved from the AST annotations
/// (with `Tag::Var` for unannotated positions).
///
/// The `type_params: &[Name]` parameter type matches `FunctionSig.type_params:
/// Vec<Name>` (`compiler/ori_types/src/output/mod.rs`) and the caller's in-scope
/// generic-parameter vocabulary (`check_impl_method` already receives
/// `type_params: &[Name]`). The AST-level `TypeParam` node is deliberately NOT
/// accepted here — it is the declaration-site form, not the signature-level
/// identifier form.
fn build_method_sig(
    method_name: Name,
    params: &[Param],
    param_types: Vec<Idx>,
    return_type: Idx,
    type_params: &[Name],
    pool: &Pool,
) -> FunctionSig {
    let param_names: Vec<Name> = params.iter().map(|p| p.name).collect();
    let param_defaults: Vec<Option<ExprId>> = params.iter().map(|p| p.default).collect();
    let required_params = param_defaults.iter().filter(|d| d.is_none()).count();
    let param_hashes: Vec<u64> = param_types.iter().map(|&idx| pool.hash(idx)).collect();
    let return_hash = pool.hash(return_type);
    FunctionSig {
        name: method_name,
        type_params: type_params.to_vec(),
        const_params: vec![],
        param_names,
        param_types,
        return_type,
        capabilities: vec![],
        is_public: false,
        is_test: false,
        is_main: false,
        is_fbip: false,
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        scheme_var_ids: vec![],
        required_params,
        param_defaults,
        param_hashes,
        return_hash,
    }
}

/// Check all impl method bodies.
///
/// For trait impls, this also checks unoverridden default methods from the trait
/// definition, registering their signatures for LLVM codegen.
#[tracing::instrument(level = "debug", skip_all, fields(count = module.impls.len()))]
pub fn check_impl_bodies(checker: &mut ModuleChecker<'_>, module: &Module) {
    for impl_def in &module.impls {
        check_impl_block(checker, impl_def, &module.traits);
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
) {
    // Resolve the Self type for this impl block
    let arena = checker.arena();
    let self_type = resolve_parsed_type_simple(checker, &impl_def.self_ty, arena);

    // Collect generic params for type resolution within methods
    let generic_params: Vec<Name> = checker
        .arena()
        .get_generic_params(impl_def.generics)
        .iter()
        .map(|p| p.name)
        .collect();

    let is_trait_impl = impl_def.trait_path.is_some();

    // Check explicitly defined methods
    for method in &impl_def.methods {
        check_impl_method(checker, method, self_type, &generic_params);
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
                            check_impl_method(checker, &as_impl, self_type, &generic_params);
                            checker.register_trait_impl_fn_name(self_type, default.name);
                        }
                    }
                }
            }
        }
    }
}

/// Type check a single impl method body.
fn check_impl_method(
    checker: &mut ModuleChecker<'_>,
    method: &ImplMethod,
    self_type: Idx,
    type_params: &[Name],
) {
    // Create child environment from frozen base
    let Some(child_env) = checker.child_of_base() else {
        return;
    };

    // Resolve parameter types with Self substitution
    let params: Vec<_> = checker.arena().get_params(method.params).to_vec();
    let mut param_env = child_env;

    let mut param_types = Vec::with_capacity(params.len());
    for p in &params {
        let is_self = p.name == checker.well_known().self_kw;
        let ty = match &p.ty {
            Some(parsed_ty) => resolve_type_with_self(checker, parsed_ty, type_params, self_type),
            None if is_self => self_type,
            None => checker.pool_mut().fresh_var(),
        };
        param_env.bind(p.name, ty);
        param_types.push(ty);
    }

    // Resolve return type with Self substitution. `mut` so the
    // defaulting pass can refresh it to `Idx::NEVER` before `build_method_sig`
    // bakes it into the exported sig.
    let mut return_type =
        resolve_type_with_self(checker, &method.return_ty, type_params, self_type);

    // Build function type for recursion support
    let fn_type = checker.pool_mut().function(&param_types, return_type);

    // Get body span before entering scope
    let body_span = checker.arena().get_expr(method.body).span;

    // Check body within impl scope + function scope
    //
    // the inner closure defaults unbound vars reachable from
    // empty-literal expr roots BEFORE returning. `param_types` and
    // `return_type` are captured mutably so `build_method_sig` (below) sees
    // the defaulted values. Exempt set is empty — impl-block generic params
    // are RigidVars (not Unbound), so `collect_unbound_reachable_vars`
    // naturally skips them per `VarState::Rigid` branch.
    let param_types_ref = &mut param_types;
    let return_type_ref = &mut return_type;
    let (expr_types, errors, warnings, pat_resolutions, mono_instances, deferred_mono_calls) =
        checker.with_impl_scope(self_type, |c| {
            c.with_function_scope(fn_type, FxHashSet::default(), |c| {
                let arena = c.arena();
                let mut engine = c.create_engine_with_env(param_env);

                engine.push_context(ContextKind::FunctionReturn {
                    func_name: Some(method.name),
                });

                // Check body against declared return type (bidirectional)
                let expected = Expected {
                    ty: *return_type_ref,
                    origin: ExpectedOrigin::Context {
                        span: body_span,
                        kind: ContextKind::FunctionReturn {
                            func_name: Some(method.name),
                        },
                    },
                };
                let _body_ty = check_expr(&mut engine, arena, method.body, &expected, body_span);

                engine.pop_context();

                let mut expr_types = engine.take_expr_types();
                engine.default_unbound_vars_in_scope(
                    arena,
                    &mut expr_types,
                    param_types_ref,
                    return_type_ref,
                    &FxHashSet::default(),
                );

                // §08.3b.1 — normalize `Tag::Var(Generalized)` leaves to
                // `Tag::BoundVar` per `types.md §SC-1`. Impl methods have no
                // top-level scheme_var_ids (generic params are RigidVars),
                // so only pending_generalized_vars from inner let-polymorphism
                // drives the rewrite here. See `check_function` for full rationale.
                engine.normalize_body_generalized_to_bound_var(
                    &mut expr_types,
                    param_types_ref,
                    return_type_ref,
                    &[],
                );

                (
                    expr_types,
                    engine.take_errors(),
                    engine.take_warnings(),
                    engine.take_pattern_resolutions(),
                    engine.take_mono_instances(),
                    engine.take_deferred_mono_calls(),
                )
            })
        });

    // Build the post-defaulted signature. `param_types` and `return_type` have
    // been refreshed in place by `default_unbound_vars_in_scope` inside the
    // inference closure, so the sig reflects end-of-body truth — the exact
    // inputs `run_validator` needs to enforce `PC-2` across sig positions.
    let sig = build_method_sig(
        method.name,
        &params,
        param_types,
        return_type,
        type_params,
        checker.pool(),
    );

    // Shared PC-2 validation + store/push/accumulate spine (§03.1–§03.4).
    super::finalize_body_and_export(
        checker,
        &sig,
        method.span,
        super::BodyOutputs {
            expr_types,
            errors,
            warnings,
            pat_resolutions,
            mono_instances,
            deferred_mono_calls,
        },
    );

    // Export impl method signature for codegen.
    // Codegen needs param_types, return_type, and type_params to compute ABI.
    checker.register_impl_sig(method.name, sig);
}

// Pass 5: Def Impl (Default Implementation) Method Bodies

/// Check all def impl method bodies.
#[tracing::instrument(level = "debug", skip_all, fields(count = module.def_impls.len()))]
pub fn check_def_impl_bodies(checker: &mut ModuleChecker<'_>, module: &Module) {
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
        check_def_impl_method(checker, method);
    }
}

/// Type check a single def impl method body.
fn check_def_impl_method(checker: &mut ModuleChecker<'_>, method: &ImplMethod) {
    // Create child environment from frozen base
    let Some(child_env) = checker.child_of_base() else {
        return;
    };

    // Resolve parameter types (no Self substitution for def impl)
    let arena = checker.arena();
    let params: Vec<_> = arena.get_params(method.params).to_vec();
    let mut param_env = child_env;

    let mut param_types = Vec::with_capacity(params.len());
    for p in &params {
        let ty = match &p.ty {
            Some(parsed_ty) => resolve_parsed_type_simple(checker, parsed_ty, arena),
            None => checker.pool_mut().fresh_var(),
        };
        param_env.bind(p.name, ty);
        param_types.push(ty);
    }

    // Resolve return type. `mut` so defaulting can refresh it.
    let mut return_type = resolve_parsed_type_simple(checker, &method.return_ty, arena);

    // Build function type
    let fn_type = checker.pool_mut().function(&param_types, return_type);

    // Get body span
    let body_span = checker.arena().get_expr(method.body).span;

    // Check body with function scope only (no impl scope for def impl).
    // default unbound vars reachable from empty-literal expr
    // roots before returning from the closure.
    let param_types_ref = &mut param_types;
    let return_type_ref = &mut return_type;
    let (expr_types, errors, warnings, pat_resolutions, mono_instances, deferred_mono_calls) =
        checker.with_function_scope(fn_type, FxHashSet::default(), |c| {
            let arena = c.arena();
            let mut engine = c.create_engine_with_env(param_env);

            engine.push_context(ContextKind::FunctionReturn {
                func_name: Some(method.name),
            });

            // Check body against declared return type (bidirectional)
            let expected = Expected {
                ty: *return_type_ref,
                origin: ExpectedOrigin::Context {
                    span: body_span,
                    kind: ContextKind::FunctionReturn {
                        func_name: Some(method.name),
                    },
                },
            };
            let _body_ty = check_expr(&mut engine, arena, method.body, &expected, body_span);

            engine.pop_context();

            let mut expr_types = engine.take_expr_types();
            engine.default_unbound_vars_in_scope(
                arena,
                &mut expr_types,
                param_types_ref,
                return_type_ref,
                &FxHashSet::default(),
            );

            // §08.3b.1 — normalize `Tag::Var(Generalized)` leaves to
            // `Tag::BoundVar` per `types.md §SC-1`. def-impl methods have
            // no top-level scheme_var_ids; only inner let-polymorphism
            // generalization contributes via pending_generalized_vars.
            engine.normalize_body_generalized_to_bound_var(
                &mut expr_types,
                param_types_ref,
                return_type_ref,
                &[],
            );

            (
                expr_types,
                engine.take_errors(),
                engine.take_warnings(),
                engine.take_pattern_resolutions(),
                engine.take_mono_instances(),
                engine.take_deferred_mono_calls(),
            )
        });

    // Build the post-defaulted signature. `param_types` and `return_type` have
    // been refreshed in place by `default_unbound_vars_in_scope` inside the
    // inference closure, so the sig reflects end-of-body truth — the exact
    // inputs `run_validator` needs to enforce `PC-2` across sig positions.
    // def-impl methods never register a sig externally (no `register_impl_sig`
    // call); the sig is validator-local. `type_params = &[]` because
    // `check_def_impl_method` does not receive type params at the method level.
    let sig = build_method_sig(
        method.name,
        &params,
        param_types,
        return_type,
        &[],
        checker.pool(),
    );

    // Shared PC-2 validation + store/push/accumulate spine (§03.1–§03.4).
    super::finalize_body_and_export(
        checker,
        &sig,
        method.span,
        super::BodyOutputs {
            expr_types,
            errors,
            warnings,
            pat_resolutions,
            mono_instances,
            deferred_mono_calls,
        },
    );
}
