//! Def-impl (default implementation) method body type checking.
//!
//! Owns `check_def_impl_bodies` (Pass 5) and its block/method helpers. See
//! `bodies/mod.rs` for the architecture docstring that covers all four body passes.

use ori_ir::{ImplMethod, Module};
use rustc_hash::FxHashSet;

use super::method_sig::{allocate_method_binders, build_method_sig};
use crate::check::registration::resolve_type_with_method_generics;
use crate::check::ModuleChecker;
use crate::{check_expr, ContextKind, Expected, ExpectedOrigin};

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
        check_def_impl_method(checker, method, def_impl.trait_name);
    }
}

/// Type check a single def impl method body.
#[expect(
    clippy::too_many_lines,
    reason = "rank-scope-wrapped body-inference closure with method-binder setup \
              matches the canonical body-checking shape shared with check_function; \
              splitting across helpers would obscure §SG-5 enter/exit pairing"
)]
fn check_def_impl_method(
    checker: &mut ModuleChecker<'_>,
    method: &ImplMethod,
    def_impl_trait: ori_ir::Name,
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
        composed_burdens,
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

            // §09.2/§10.1: the def-impl's `Self` RigidVar is bound by the
            // implemented trait — register it so a body-internal `self.m()`
            // call resolves the trait's methods via the §10.1 bound-chain
            // (`impl_lookup.rs` `find_trait_method_via_bound_chain`). Without
            // this, `self.m()` on the Self RigidVar finds no bound, returns
            // NotFound, and (post-§06.5) reports a spurious "no method on
            // generic type" — the dispatch never truly resolved (it relied on
            // the silent-accept poison §06.5 removed).
            engine.bind_method_rigid_bound(self_rigid, def_impl_trait);

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
        },
    );
}
