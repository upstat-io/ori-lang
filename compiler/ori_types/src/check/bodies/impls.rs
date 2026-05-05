//! Impl and def-impl method body type checking.
//!
//! Owns `check_impl_bodies` (Pass 4), `check_def_impl_bodies` (Pass 5), their
//! block/method helpers, and the shared `build_method_sig` constructor. See
//! `bodies/mod.rs` for the architecture docstring that covers all four body passes.

use ori_ir::{ExprId, ImplMethod, Module, Name, Param, TraitBound, TraitDef, TraitItem};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::check::registration::{resolve_parsed_type_simple, resolve_type_with_method_generics};
use crate::check::signatures::resolve_const_param_type;
use crate::check::ModuleChecker;
use crate::output::ConstParamInfo;
use crate::{check_expr, ContextKind, Expected, ExpectedOrigin, FunctionSig, Idx, Pool};

/// Result of `allocate_method_binders` — bundles the four method-binder data
/// products: the substitution map (binder name → fresh `RigidVar` Idx), the
/// type-generic param-name list, per-const-generic info, and inline trait-bound
/// assumptions per non-const binder. Tuple-typedef keeps the helper signature
/// readable; clippy `type_complexity` flagged the bare 4-tuple.
type MethodBinderInfo = (
    FxHashMap<Name, Idx>,
    Vec<Name>,
    Vec<ConstParamInfo>,
    Vec<(Idx, Vec<Name>)>,
);

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
    const_params: Vec<ConstParamInfo>,
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
        const_params,
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

/// Allocate fresh `RigidVar`s for a method's type-level generics.
///
/// Phase B Step 5 (BUG-01-002): `pool.rigid_var(name)` allocates a fresh
/// `var_id` per call, so the resulting `Idx` is distinct from any other
/// `Tag::RigidVar` or `Tag::Named` with the same name. This is the
/// binder-identity guarantee + §B.2 line 139 — method-level
/// `T@method` and impl-level `T@impl` resolve to distinct pool entries even
/// when names collide.
///
/// Returns `(method_substitutions, method_generic_param_names)`. Callers use
/// the substitution map as the overlay for `resolve_type_with_method_generics`
/// and the param-name vec to build the combined resolver scope (impl-level +
/// method-level) and to bind the `RigidVar`s in `param_env` for body-level
/// type-annotation lookups.
fn allocate_method_binders(
    checker: &mut ModuleChecker<'_>,
    method: &ImplMethod,
) -> MethodBinderInfo {
    let generic_params = checker.arena().get_generic_params(method.generics).to_vec();
    let method_generic_params: Vec<Name> = generic_params
        .iter()
        .filter(|p| !p.is_const)
        .map(|p| p.name)
        .collect();
    let mut method_substitutions: FxHashMap<Name, Idx> = FxHashMap::default();
    for &mname in &method_generic_params {
        let rigid_idx = checker.pool_mut().rigid_var(mname);
        method_substitutions.insert(mname, rigid_idx);
    }
    // Phase B-Residual-2 (a) — collect method-level const generics. Their names are
    // bound into `param_env` at body-check entry as their declared type (`int` /
    // `bool`) so the body can reference them as values, mirroring the top-level
    // function pattern in `check/bodies/functions.rs`.
    let const_params: Vec<ConstParamInfo> = generic_params
        .iter()
        .filter(|p| p.is_const)
        .map(|p| ConstParamInfo {
            name: p.name,
            const_type: resolve_const_param_type(checker, p),
            default_value: p.default_value,
        })
        .collect();
    // Phase B-Residual-2 (c) — collect inline `<T: Bound>` trait-bound assumptions
    // per non-const binder. Each entry pairs the binder's allocated RigidVar Idx
    // with the simple Names of every declared trait bound. The body-check entry
    // registers these on the InferEngine so body-internal trait dispatch (e.g.,
    // `Printable.to_str(prefix)` in string interpolation) treats `prefix : T` as
    // satisfying the bound. Empty-bounds entries are skipped to keep the table
    // tight.
    let inline_bounds: Vec<(Idx, Vec<Name>)> = generic_params
        .iter()
        .filter(|p| !p.is_const && !p.bounds.is_empty())
        .map(|p| {
            let rigid_idx = method_substitutions[&p.name];
            let trait_names: Vec<Name> = p.bounds.iter().map(TraitBound::name).collect();
            (rigid_idx, trait_names)
        })
        .collect();
    (
        method_substitutions,
        method_generic_params,
        const_params,
        inline_bounds,
    )
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
    let (method_substitutions, method_generic_params, method_const_params, method_inline_bounds) =
        allocate_method_binders(checker, method);

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
                &method_substitutions,
                &combined_type_params,
                self_type,
            ),
            None if is_self => self_type,
            None => checker.pool_mut().fresh_var(),
        };
        param_env.bind(p.name, ty);
        param_types.push(ty);
    }

    // Resolve return type with Self substitution + method-level overlay. `mut`
    // so the defaulting pass can refresh it to `Idx::NEVER` before
    // `build_method_sig` bakes it into the exported sig.
    let mut return_type = resolve_type_with_method_generics(
        checker,
        &method.return_ty,
        &method_substitutions,
        &combined_type_params,
        self_type,
    );

    // §B.2 step 2: push method-level RigidVar bindings into the TypeEnv child
    // map. Body-level type-annotation lookups (e.g., `let x: T = expr;` inside
    // the method body) consult `param_env`; the child-map shadowing here is
    // what makes those lookups see the method-level `RigidVar` rather than
    // any impl-level `Tag::Named("T")` that happens to share the name.
    for (&mname, &rigid_idx) in &method_substitutions {
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
    let (
        expr_types,
        errors,
        warnings,
        pat_resolutions,
        mono_instances,
        mono_dispatch_pre_dedup,
        deferred_mono_calls,
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

            // Phase B Step 5 (BUG-01-002): rank scope
            // + §CK-2 / §GN-1. Method-level binders live at strictly
            // higher rank than impl-level bindings; the push/pop pair
            // here is manually matched (no RAII) — exit MUST happen on
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

            let mut expr_types = engine.take_expr_types();
            engine.default_unbound_vars_in_scope(
                arena,
                &mut expr_types,
                param_types_ref,
                return_type_ref,
                &FxHashSet::default(),
            );

            // §08.3b.1 — normalize `Tag::Var(Generalized)` leaves to
            // `Tag::BoundVar` per. Impl methods have no
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

            (
                expr_types,
                engine.take_errors(),
                engine.take_warnings(),
                engine.take_pattern_resolutions(),
                engine.take_mono_instances(),
                engine.take_mono_dispatch_pre_dedup(),
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
        method_const_params,
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
            mono_dispatch_pre_dedup,
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
#[expect(
    clippy::too_many_lines,
    reason = "rank-scope-wrapped body-inference closure with method-binder setup \
              matches the canonical body-checking shape shared with check_function; \
              splitting across helpers would obscure §SG-5 enter/exit pairing"
)]
fn check_def_impl_method(checker: &mut ModuleChecker<'_>, method: &ImplMethod) {
    // Create child environment from frozen base
    let Some(child_env) = checker.child_of_base() else {
        return;
    };

    // Phase B Step 5: bind method-level type generics as fresh RigidVars.
    // Phase B-Residual-2 (a): also collect method-level const generics for
    // body-scope binding below.
    // Phase B-Residual-2 (c): also collect inline `<T: Bound>` assumptions
    // for body-internal trait dispatch.
    // Def-impl methods have no impl-level type params and no `Self`, so the
    // resolver scope is method-level only and `self_type` is `Idx::ERROR`
    // (matching the pre-existing `resolve_parsed_type_simple` semantics for
    // `SelfType`).
    let (method_substitutions, method_generic_params, method_const_params, method_inline_bounds) =
        allocate_method_binders(checker, method);

    // §09.2: allocate the def-impl's `Self` `RigidVar` BEFORE resolving
    // params/return so `(self)` param annotations and `Self` return
    // annotations resolve to this RigidVar instead of the
    // `engine.fresh_var()` fallback at `infer/expr/type_resolution.rs:184`
    // (which fires when no `impl_self_type` is set). Without this early
    // allocation, the `Tag::Var` for the elided self type leaks into
    // `param_types[0]` and surfaces as `E2005` at PC-2 validation.
    let self_rigid = checker.pool_mut().rigid_var(ori_ir::Name::EMPTY);

    // Resolve parameter types (Self bound to self_rigid; method-level overlay applies)
    let arena = checker.arena();
    let params: Vec<_> = arena.get_params(method.params).to_vec();
    let mut param_env = child_env;

    let self_kw = checker.well_known().self_kw;
    let mut param_types = Vec::with_capacity(params.len());
    for p in &params {
        let ty = match &p.ty {
            Some(parsed_ty) => resolve_type_with_method_generics(
                checker,
                parsed_ty,
                &method_substitutions,
                &method_generic_params,
                self_rigid,
            ),
            // `(self)` reaches here with `ty: None` (the parser doesn't
            // synthesize a SelfType annotation for it). Bind `self`'s type
            // to the def-impl's Self RigidVar instead of a fresh var so
            // the receiver type at body-internal `self.method()` calls
            // can dispatch via `§10.1` bound-chain on a stable RigidVar
            // identity rather than an unbound `Tag::Var` that leaks as
            // `E2005` at PC-2 validation.
            None if p.name == self_kw => self_rigid,
            None => checker.pool_mut().fresh_var(),
        };
        param_env.bind(p.name, ty);
        param_types.push(ty);
    }

    // Resolve return type. `mut` so defaulting can refresh it.
    let mut return_type = resolve_type_with_method_generics(
        checker,
        &method.return_ty,
        &method_substitutions,
        &method_generic_params,
        self_rigid,
    );

    // §B.2 step 2: bind method-level RigidVars in TypeEnv child for body-level
    // type-annotation lookups (e.g., `let x: T = expr;`).
    for (&mname, &rigid_idx) in &method_substitutions {
        param_env.bind(mname, rigid_idx);
    }

    // Phase B-Residual-2 (a): bind method-level const generics as their
    // declared type for body identifier resolution. Mirrors the
    // `check_impl_method` and `functions.rs:54-58` patterns.
    for cp in &method_const_params {
        param_env.bind(cp.name, cp.const_type);
    }

    // Build function type
    let fn_type = checker.pool_mut().function(&param_types, return_type);

    // Get body span
    let body_span = checker.arena().get_expr(method.body).span;

    // self_rigid was allocated earlier (before param resolution) so the
    // `(self)` param's Self annotation resolved to it; with_impl_scope
    // below makes it visible to body-internal Self references too.

    // Check body with function scope wrapped in impl scope so the engine's
    // `impl_self_type()` returns `self_rigid` for the duration. Defaulting
    // for unbound vars reachable from empty-literal expr roots fires before
    // returning from the closure.
    let param_types_ref = &mut param_types;
    let return_type_ref = &mut return_type;
    let const_params_for_engine = method_const_params.clone();
    let inline_bounds_for_engine = method_inline_bounds.clone();
    let (
        expr_types,
        errors,
        warnings,
        pat_resolutions,
        mono_instances,
        mono_dispatch_pre_dedup,
        deferred_mono_calls,
    ) = checker.with_impl_scope(self_rigid, |c| {
        c.with_function_scope(fn_type, FxHashSet::default(), |c| {
            let arena = c.arena();
            let mut engine = c.create_engine_with_env(param_env);

            // Phase B-Residual-2 (a): bind method-level const generics on the
            // engine for `$N` const-position lookups inside the body. Mirrors
            // the `check_impl_method` engine-binding pattern.
            for cp in &const_params_for_engine {
                engine.bind_method_const(cp.name, cp.const_type);
            }

            // Phase B-Residual-2 (c): register inline `<T: Bound>` trait-bound
            // assumptions on the engine for body-internal trait dispatch.
            // Mirrors the `check_impl_method` engine-binding pattern.
            for (rigid_idx, trait_names) in &inline_bounds_for_engine {
                for &tname in trait_names {
                    engine.bind_method_rigid_bound(*rigid_idx, tname);
                }
            }

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
            // `Tag::BoundVar` per. def-impl methods have
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
                engine.take_mono_dispatch_pre_dedup(),
                engine.take_deferred_mono_calls(),
            )
        })
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
        method_const_params,
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
            mono_dispatch_pre_dedup,
            deferred_mono_calls,
        },
    );
}
