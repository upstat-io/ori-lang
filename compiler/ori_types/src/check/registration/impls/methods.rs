//! Impl method signature construction.

use ori_ir::Name;
use rustc_hash::FxHashMap;

use super::super::type_resolution::{
    build_method_generic_metadata_from, resolve_type_with_method_generics_from,
};
use crate::check::bodies::allocate_rigid_var_map;
use crate::{Idx, ImplMethodDef, ModuleChecker};

/// Build an `ImplMethodDef` from an impl method.
///
/// When the method declares method-level type generics
/// (e.g. `@map<U> (self, f: T -> U) -> Box<U>` in `impl<T> Box<T>`),
/// the registered `signature` is wrapped in a `Tag::Scheme(scheme_var_ids,
/// fn_type)`. The scheme's body carries fresh `Tag::Var` Idx values (one per
/// method-level type-generic) whose `var_id`s match `scheme_var_ids`. At
/// call-site, `resolve_impl_signature` invokes `engine.instantiate(...)` —
/// the standard generalization-instantiation pattern — so each
/// call gets fresh unification vars that DO unify with concrete types.
/// Without this wrapping, method-level binders in the registered sig
/// would either be bare `Tag::Var`s (which lower-rank generalization would
/// incorrectly capture) or unresolved `Tag::Named` (which fails to unify
/// against function-typed arguments).
///
/// Body-checking (`check/bodies/impls.rs::check_impl_method`) allocates a
/// SEPARATE set of `Tag::RigidVar`s per method-level binder via
/// `pool.rigid_var(name)` — those `RigidVars` enforce body-internal
/// parametricity (the negative pin `shadow_negative_binder_identity.ori`).
/// Registration's fresh-Var-in-Scheme and body-check's `RigidVars` are distinct
/// pool entries serving distinct purposes; no sharing is required.
pub(super) fn build_impl_method(
    checker: &mut ModuleChecker<'_>,
    method: &ori_ir::ImplMethod,
    type_params: &[Name],
    self_type: Idx,
    trait_substitutions: &FxHashMap<Name, Idx>,
) -> ImplMethodDef {
    let arena = checker.arena();
    build_impl_method_from(
        checker,
        method,
        type_params,
        self_type,
        trait_substitutions,
        arena,
        true,
    )
}

pub(super) fn build_impl_method_from(
    checker: &mut ModuleChecker<'_>,
    method: &ori_ir::ImplMethod,
    type_params: &[Name],
    self_type: Idx,
    trait_substitutions: &FxHashMap<Name, Idx>,
    arena: &ori_ir::ExprArena,
    allocate_body_rigids: bool,
) -> ImplMethodDef {
    // Deep-copy method-level generics + where-clauses into arena-independent
    // owned form for downstream bound enforcement. Also collect the
    // `Name → Idx` overlay for fresh-Var substitution of method-level type
    // names in param/return resolution.
    let generic_params = arena.get_generic_params(method.generics).to_vec();
    let (scheme_var_ids, scheme_overlay, generic_param_metadata, where_clause_metadata) =
        build_method_generic_metadata_from(
            checker,
            &generic_params,
            &method.where_clauses,
            type_params,
            self_type,
            arena,
        );

    // allocate the method body's `RigidVar`s NOW (Pass 0c) and store
    // them keyed by body `ExprId`, so `check_impl_method` (Pass 4) REUSES them
    // via `prealloc`. Symmetric with the impl-level binder lifecycle: the rigid
    // vars must exist in `var_states` before any pass-3 call-site records a
    // method mono, so the recording's name-scan (`build_impl_rigid_var_subst`)
    // resolves `[Rigid(method_T)] -> concrete` in `body_type_map`. Without early
    // allocation the body's `RigidVar`s are born at Pass 4 (after the recording)
    // and the rigid leaf survives to executable projection. The registration-time scheme vars
    // (`scheme_var_ids`, fresh `Tag::Var` for call-site instantiation) and these
    // body rigid vars are distinct pool entries serving distinct purposes.
    if allocate_body_rigids {
        let method_rigid_var_map = allocate_rigid_var_map(checker, method.generics);
        checker.set_method_rigid_var_map(method.body, method_rigid_var_map);
    }

    // Merge `trait_substitutions` into the resolver overlay used for
    // param/return type resolution. The trait→impl
    // substitution carries impl-level `Idx` values for the trait's declared
    // type-params (e.g. `F → pool.named("X")` for `impl<X> Reducer<X>` over
    // `trait Reducer<F>`); the method-level `scheme_overlay` carries fresh
    // `Tag::Var` Idx values for method-level binders (e.g. `T → V_T_var`).
    // The two are at different scopes by construction (trait scope vs method
    // scope) — collisions are vanishingly rare, but if they happen the
    // method-level binder shadows per HM scoping, so use `or_insert` to
    // preserve `scheme_overlay`'s entries on collision. The combined overlay
    // is used ONLY for param/return resolution; `scheme_var_ids` and
    // `generic_param_metadata` describe method-level binders alone and are
    // not affected. (Where-clause resolution inside
    // `build_method_generic_metadata` still runs against `scheme_overlay`
    // only — inherited defaults whose where-clauses reference trait-level
    // binders are a known follow-up.)
    let mut combined_overlay = scheme_overlay.clone();
    for (&tname, &tidx) in trait_substitutions {
        combined_overlay.entry(tname).or_insert(tidx);
    }

    // Combined scope for type resolution: impl-level (outer) + method-level
    // (inner) names. `resolve_type_with_method_generics` checks the overlay
    // first (method-level → fresh `Tag::Var`, OR trait-level → impl arg Idx
    // for inherited defaults), then falls through to `type_params.contains`
    // (impl-level → `Tag::Named`).
    let method_param_names: Vec<Name> = scheme_overlay.keys().copied().collect();
    let combined_type_params: Vec<Name> = type_params
        .iter()
        .copied()
        .chain(method_param_names.iter().copied())
        .collect();

    // Resolve parameter types, substituting Self with the actual type and
    // method-level binders with their fresh-Var Idx via the overlay.
    let params: Vec<_> = arena.get_params(method.params).to_vec();
    let param_types: Vec<Idx> = params
        .iter()
        .map(|p| {
            let is_self = p.name == checker.well_known().self_kw;
            match p.ty.as_ref() {
                Some(ty) => resolve_type_with_method_generics_from(
                    checker,
                    ty,
                    &combined_overlay,
                    &combined_type_params,
                    self_type,
                    arena,
                ),
                None if is_self => self_type,
                None => Idx::ERROR,
            }
        })
        .collect();

    // Resolve return type with the same combined overlay.
    let return_ty = resolve_type_with_method_generics_from(
        checker,
        &method.return_ty,
        &combined_overlay,
        &combined_type_params,
        self_type,
        arena,
    );

    // Detect whether the first parameter is `self` (instance method vs associated function)
    let has_self = params
        .first()
        .is_some_and(|p| p.name == checker.well_known().self_kw);

    // Create the function-type body. When the method has method-level type
    // generics (scheme_var_ids non-empty), wrap in Tag::Scheme so call-site
    // resolution can instantiate per `GN-2`. When empty, store the bare
    // function type (zero behavioral change for non-method-generic methods).
    let fn_type = checker.pool_mut().function(&param_types, return_ty);
    let signature = if scheme_var_ids.is_empty() {
        fn_type
    } else {
        checker.pool_mut().scheme(&scheme_var_ids, fn_type)
    };

    // Count of non-`self` params WITH a default value. The relaxed call-site
    // arity check (resolve_impl_signature) permits omitting that many trailing
    // params; canon fills them.
    let optional_param_count = params
        .iter()
        .skip(usize::from(has_self))
        .filter(|p| p.default.is_some())
        .count();

    ImplMethodDef {
        name: method.name,
        signature,
        has_self,
        body: method.body,
        scheme_var_ids,
        generic_param_metadata,
        where_clause_metadata,
        optional_param_count,
        span: method.span,
    }
}
