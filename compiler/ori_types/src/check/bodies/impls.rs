//! Impl method body type checking.
//!
//! Owns `check_impl_bodies` (Pass 4), its block/method helpers. See
//! `bodies/mod.rs` for the architecture docstring that covers all four body passes.

use ori_ir::{ImplMethod, Module, Name, TraitDef, TraitItem};
use rustc_hash::{FxHashMap, FxHashSet};

use super::method_sig::{allocate_generic_binders, build_method_sig};
use crate::check::registration::resolve_type_with_method_generics;
use crate::check::ModuleChecker;
use crate::{check_expr, ContextKind, Expected, ExpectedOrigin, Idx};

/// Check all impl method bodies.
///
/// For trait impls, this also checks unoverridden default methods from the trait
/// definition, registering their signatures for LLVM codegen.
#[tracing::instrument(level = "debug", skip_all, fields(count = module.impls.len()))]
pub fn check_impl_bodies(checker: &mut ModuleChecker<'_>, module: &Module) {
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
    // §10.1.2: impl-level generics (`impl<T: Bound>`) bind as `RigidVar`s so a
    // body-internal `receiver.method()` whose receiver is an impl-level type
    // parameter reaches the §10.1 bound-chain (bounded calls dispatch, unbounded
    // calls surface method-not-found) instead of resolving to an unresolved
    // `Tag::Named` (`registration/type_resolution.rs`). The `RigidVar`s are
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

    // Check explicitly defined methods
    for method in &impl_def.methods {
        check_impl_method(
            checker,
            method,
            self_type,
            &impl_generic_params,
            &impl_substitutions,
            &impl_inline_bounds,
        );
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
                            check_impl_method(
                                checker,
                                &as_impl,
                                self_type,
                                &impl_generic_params,
                                &impl_substitutions,
                                &impl_inline_bounds,
                            );
                            checker.register_trait_impl_fn_name(self_type, default.name);
                        }
                    }
                }
            }
        }
    }
}

/// Type check a single impl method body.
#[expect(
    clippy::too_many_lines,
    reason = "rank-scope-wrapped body-inference closure with method-binder setup \
              matches the canonical body-checking shape shared with check_function; \
              splitting across helpers would obscure §SG-5 enter/exit pairing"
)]
fn check_impl_method(
    checker: &mut ModuleChecker<'_>,
    method: &ImplMethod,
    self_type: Idx,
    type_params: &[Name],
    impl_substitutions: &FxHashMap<Name, Idx>,
    impl_inline_bounds: &[(Idx, Vec<Name>)],
) {
    // Create child environment from frozen base
    let Some(child_env) = checker.child_of_base() else {
        return;
    };

    // Phase B Step 5: bind method-level type generics as fresh RigidVars.
    // Phase B-Residual-2 (a): also collect method-level const generics for
    // body-scope binding below.
    // Phase B-Residual-2 (c): also collect inline `<T: Bound>` assumptions
    // for body-internal trait dispatch.
    // BUG-04-146: REUSE the method-level `RigidVar`s allocated at Pass 0c
    // (`register_impls`), keyed by body `ExprId`, so the body's generic types
    // reference the identical `RigidVar` Idxs the pass-3 call-site recording
    // scan substituted into `body_type_map`. Falls back to fresh allocation
    // (`None`) for any method without a registered map (e.g. a synthesized
    // trait default whose body was not registered), preserving prior behavior.
    let method_prealloc: Option<FxHashMap<Name, Idx>> =
        checker.method_rigid_var_map_for(method.body).cloned();
    let (method_substitutions, method_generic_params, method_const_params, method_inline_bounds) =
        allocate_generic_binders(checker, method.generics, method_prealloc.as_ref());

    // §10.1.2: merge impl-level (`impl<T>`) + method-level (`@m<U>`) RigidVar
    // overlays so an impl-level type-param annotation (`x: T`) resolves to the
    // impl `RigidVar` allocated once in `check_impl_block`, not a fresh
    // `Tag::Named`. Method-level binders win on a name collision (inner scope).
    let mut combined_substitutions = impl_substitutions.clone();
    combined_substitutions.extend(method_substitutions.iter().map(|(&n, &i)| (n, i)));

    // Combined scope for type resolution: impl-level (parent) + method-level
    // (child). Without method-level names in scope, `(self, f: T -> U) -> Box<U>`
    // shapes would fail to resolve `U` at body-check time.
    let combined_type_params: Vec<Name> = type_params
        .iter()
        .copied()
        .chain(method_generic_params.iter().copied())
        .collect();

    // Resolve parameter types with Self substitution + method-level overlay
    let params: Vec<_> = checker.arena().get_params(method.params).to_vec();
    let mut param_env = child_env;

    let mut param_types = Vec::with_capacity(params.len());
    for p in &params {
        let is_self = p.name == checker.well_known().self_kw;
        let ty = match &p.ty {
            Some(parsed_ty) => resolve_type_with_method_generics(
                checker,
                parsed_ty,
                &combined_substitutions,
                &combined_type_params,
                self_type,
            ),
            None if is_self => self_type,
            None => checker.pool_mut().fresh_var(),
        };
        param_env.bind(p.name, ty);
        param_types.push(ty);
    }

    // BUG-04-190: collect (default expr, param name, declared type) for every
    // method param carrying a default, so the body-check closure can type-check
    // each default against its declared type (mirrors functions.rs). Captured
    // here before `param_types` is borrowed mutably below.
    let default_checks: Vec<_> = params
        .iter()
        .zip(param_types.iter())
        .filter_map(|(p, &ty)| p.default.map(|d| (d, p.name, ty)))
        .collect();

    // Resolve return type with Self substitution + method-level overlay. `mut`
    // so the defaulting pass can refresh it to `Idx::NEVER` before
    // `build_method_sig` bakes it into the exported sig.
    let mut return_type = resolve_type_with_method_generics(
        checker,
        &method.return_ty,
        &combined_substitutions,
        &combined_type_params,
        self_type,
    );

    // §B.2 step 2: push method-level RigidVar bindings into the TypeEnv child
    // map. Body-level type-annotation lookups (e.g., `let x: T = expr;` inside
    // the method body) consult `param_env`; the child-map shadowing here is
    // what makes those lookups see the method-level `RigidVar` rather than
    // any impl-level `Tag::Named("T")` that happens to share the name.
    for (&mname, &rigid_idx) in &combined_substitutions {
        param_env.bind(mname, rigid_idx);
    }

    // Phase B-Residual-2 (a): bind method-level const generics as their
    // declared type. For `@first_n<$N: int>`, bind N -> int so the body can
    // reference N (e.g., `take(count: N)`). Mirrors functions.rs:54-58.
    for cp in &method_const_params {
        param_env.bind(cp.name, cp.const_type);
    }

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
    let const_params_for_engine = method_const_params.clone();
    let inline_bounds_for_engine = method_inline_bounds.clone();
    // §10.1.2: impl-level bounds (`impl<T: Bound>`) registered on the engine
    // alongside method-level ones so body-internal dispatch on an impl-level
    // `RigidVar` resolves via the §10.1 bound-chain.
    let impl_bounds_for_engine = impl_inline_bounds.to_vec();
    let (
        expr_types,
        errors,
        warnings,
        pat_resolutions,
        mono_instances,
        mono_dispatch_pre_dedup,
        deferred_mono_calls,
        composed_burdens,
        assign_desugars,
    ) = checker.with_impl_scope(self_type, |c| {
        c.with_function_scope(fn_type, FxHashSet::default(), |c| {
            let arena = c.arena();
            let mut engine = c.create_engine_with_env(param_env);

            // Phase B-Residual-2 (a): bind method-level const generics on the
            // engine for `$N` const-position lookups inside the body
            // (e.g., `to_fixed<$N>()`). Identifier-position lookups (`count: N`)
            // are already covered by `param_env.bind` above.
            for cp in &const_params_for_engine {
                engine.bind_method_const(cp.name, cp.const_type);
            }

            // Phase B-Residual-2 (c): register inline `<T: Bound>` trait-bound
            // assumptions on the engine so body-internal trait dispatch
            // (e.g., `Printable.to_str(prefix)` in string interpolation when
            // `prefix : T` and `T: Printable`) succeeds for the RigidVar.
            for (rigid_idx, trait_names) in &inline_bounds_for_engine {
                for &tname in trait_names {
                    engine.bind_method_rigid_bound(*rigid_idx, tname);
                }
            }
            // §10.1.2: register impl-level bounds on the same engine.
            for (rigid_idx, trait_names) in &impl_bounds_for_engine {
                for &tname in trait_names {
                    engine.bind_method_rigid_bound(*rigid_idx, tname);
                }
            }

            // Rank scope per §CK-2 / §GN-1. Method-level binders live at
            // strictly higher rank than impl-level bindings; the push/pop
            // pair here is manually matched (no RAII) — exit MUST happen on
            // every path, including error recovery, hence the explicit
            // `engine.exit_rank_scope()` immediately before the result
            // tuple is built.
            engine.enter_rank_scope();

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

            // BUG-04-190: type-check each method parameter's default expression
            // against its declared type, mirroring the free-function path in
            // functions.rs. Canon fills omitted method defaults at the call site;
            // without resolved type info a composite default (e.g. `[9, 9]`)
            // lowers with Tag::Error and the LLVM backend rejects it.
            for &(default_id, param_name, param_ty) in &default_checks {
                let default_span = arena.get_expr(default_id).span;
                let default_expected = Expected {
                    ty: param_ty,
                    origin: ExpectedOrigin::Annotation {
                        name: param_name,
                        span: default_span,
                    },
                };
                let _ = check_expr(
                    &mut engine,
                    arena,
                    default_id,
                    &default_expected,
                    default_span,
                );
            }

            // Mark body inference complete before the defaulting pre-pass runs;
            // defaulting helpers debug-assert this flag (see `check_function`).
            engine.mark_body_inference_complete();

            let mut expr_types = engine.take_expr_types();
            engine.default_unbound_vars_in_scope(
                arena,
                &mut expr_types,
                param_types_ref,
                return_type_ref,
                &FxHashSet::default(),
            );

            // Normalize `Tag::Var(Generalized)` leaves to
            // `Tag::BoundVar`. Impl methods have no
            // top-level scheme_var_ids (generic params are RigidVars),
            // so only pending_generalized_vars from inner let-polymorphism
            // drives the rewrite here. See `check_function` for full rationale.
            engine.normalize_body_generalized_to_bound_var(
                &mut expr_types,
                param_types_ref,
                return_type_ref,
                &[],
            );

            // Pop rank scope (matching push above per §SG-5 one-to-one rule).
            engine.exit_rank_scope();

            // Deep-resolve var-links so late-resolved generic-builtin
            // instantiations are var-free in the exported IR + composed by the
            // burden sweep (see `intern_link_resolved_body_types`).
            engine.compose_body_type_burdens(&expr_types);

            (
                expr_types,
                engine.take_errors(),
                engine.take_warnings(),
                engine.take_pattern_resolutions(),
                engine.take_mono_instances(),
                engine.take_mono_dispatch_pre_dedup(),
                engine.take_deferred_mono_calls(),
                engine.take_composed_burdens(),
                engine.take_assign_desugars(),
            )
        })
    });

    // Build the post-defaulted signature. `param_types` and `return_type` have
    // been refreshed in place by `default_unbound_vars_in_scope` inside the
    // inference closure, so the sig reflects end-of-body truth — the exact
    // inputs `run_validator` needs to enforce `PC-2` across sig positions.
    // BUG-04-146: include the method's own generic params (`@wrap<T>`) in the
    // registered sig's `type_params`, not just the impl-level ones. `compile_impls`
    // skips a method whose sig `is_generic()` (relying on mono instances); a
    // concrete-receiver impl has empty impl `type_params`, so without the method
    // binders here a method-level-generic template (`@wrap<T> -> [T]`) reported
    // non-generic and was codegen'd directly, emitting `[Rigid(T)]` (PC-2 break).
    // `combined_type_params` is impl-level + method-level in declaration order.
    let sig = build_method_sig(
        method.name,
        &params,
        param_types,
        return_type,
        &combined_type_params,
        method_const_params,
        checker.pool(),
    );

    // Shared PC-2 validation + store/push/accumulate spine (§03.1–§03.4).
    super::finalize_body_and_export(
        checker,
        &sig,
        method.span,
        method.body,
        super::BodyOutputs {
            expr_types,
            errors,
            warnings,
            pat_resolutions,
            mono_instances,
            mono_dispatch_pre_dedup,
            deferred_mono_calls,
            composed_burdens,
            capability_exempt_var_ids: Vec::new(),
            assign_desugars,
        },
    );

    // Export impl method signature for codegen.
    // Codegen needs param_types, return_type, and type_params to compute ABI.
    checker.register_impl_sig(method.name, sig);
}
