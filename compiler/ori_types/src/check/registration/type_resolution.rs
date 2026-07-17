//! Type resolution helpers for the registration phase.
//!
//! These functions resolve `ParsedType` nodes from the IR into `Idx` type
//! handles in the Pool. They are used across all registration submodules
//! (user types, traits, impls, derived).

mod metadata;
mod simple;

use ori_ir::{ExprArena, Name, ParsedType};
use rustc_hash::FxHashMap;

use crate::{Idx, ModuleChecker};

pub(crate) use metadata::{
    build_method_generic_metadata, build_method_generic_metadata_from, build_where_constraint,
};
pub(super) use metadata::{convert_visibility, parsed_type_contains_self};
pub(crate) use simple::resolve_parsed_type_simple;
pub(super) use simple::{
    collect_generic_param_bounds, collect_generic_params, resolve_field_type,
    resolve_type_with_params,
};

/// Resolve a parsed type with Self substitution.
///
/// Replaces `Self` references with the actual implementing type.
/// Takes `arena` as a separate parameter to avoid borrow conflicts.
pub(crate) fn resolve_type_with_self(
    checker: &mut ModuleChecker<'_>,
    parsed: &ParsedType,
    type_params: &[Name],
    self_type: Idx,
) -> Idx {
    let arena = checker.arena();
    let empty: FxHashMap<Name, Idx> = FxHashMap::default();
    resolve_type_with_overlay_inner(checker, parsed, &empty, type_params, self_type, arena)
}

/// Resolve a parsed type with Self substitution AND a method-level binder overlay.
///
/// When a `Named { name }` matches a key in `method_substitutions`, return
/// the substituted `Idx` (a fresh `RigidVar` allocated by the caller via
/// `pool.rigid_var(name)`). Binder identity holds: method-level `T` and
/// impl-level `T` resolve to distinct pool entries even when names collide,
/// because `pool.rigid_var(name)` allocates a fresh `var_id` per call and
/// interning keys on `(Tag::RigidVar, var_id)`.
///
/// `type_params` carries the COMBINED outer scope (impl-level + method-level
/// names) so that names not in the substitution map still resolve to the
/// existing `pool.named(name)` interning shape (impl-level T continues to
/// behave as `Tag::Named`).
pub(crate) fn resolve_type_with_method_generics(
    checker: &mut ModuleChecker<'_>,
    parsed: &ParsedType,
    method_substitutions: &FxHashMap<Name, Idx>,
    type_params: &[Name],
    self_type: Idx,
) -> Idx {
    let arena = checker.arena();
    resolve_type_with_method_generics_from(
        checker,
        parsed,
        method_substitutions,
        type_params,
        self_type,
        arena,
    )
}

pub(crate) fn resolve_type_with_method_generics_from(
    checker: &mut ModuleChecker<'_>,
    parsed: &ParsedType,
    method_substitutions: &FxHashMap<Name, Idx>,
    type_params: &[Name],
    self_type: Idx,
    arena: &ExprArena,
) -> Idx {
    resolve_type_with_overlay_inner(
        checker,
        parsed,
        method_substitutions,
        type_params,
        self_type,
        arena,
    )
}

/// Inner implementation of Self-substituting type resolution with optional overlay.
#[expect(
    clippy::too_many_lines,
    reason = "exhaustive ParsedType match — splitting hides the dispatch shape; \
              for the canonical tag enumeration"
)]
fn resolve_type_with_overlay_inner(
    checker: &mut ModuleChecker<'_>,
    parsed: &ParsedType,
    method_substitutions: &FxHashMap<Name, Idx>,
    type_params: &[Name],
    self_type: Idx,
    arena: &ExprArena,
) -> Idx {
    match parsed {
        ParsedType::SelfType => self_type,
        ParsedType::Named { name, type_args } => {
            // Method-level overlay first: the caller pre-allocated fresh
            // RigidVars for method-level type generics; honor those overrides
            // before falling through to the impl-level Named-interning path.
            if let Some(&idx) = method_substitutions.get(name) {
                return idx;
            }
            // Recurse into type_args through the overlay so types like
            // `Box<T>` with a method-level
            // `T` resolve as `Applied("Box", [overlay(T)])` instead of leaking
            // back through `resolve_parsed_type_simple` which is overlay-blind.
            // Without this, the body's expected return type for
            // `@map<U> (...) -> Box<U>` resolves Box's type-arg `U` as a plain
            // `Tag::Named("U")` while the body's actual return value carries
            // the overlay's fresh-Var/RigidVar — UN-6 fails to unify them.
            let arg_ids = arena.get_parsed_type_list(*type_args);
            if !arg_ids.is_empty() {
                let resolved_args: Vec<Idx> = arg_ids
                    .iter()
                    .map(|&arg_id| {
                        let arg = arena.get_parsed_type(arg_id);
                        resolve_type_with_overlay_inner(
                            checker,
                            arg,
                            method_substitutions,
                            type_params,
                            self_type,
                            arena,
                        )
                    })
                    .collect();
                if let Some(idx) = checker.resolve_well_known_generic_cached(*name, &resolved_args)
                {
                    return idx;
                }
                return checker.pool_mut().applied(*name, &resolved_args);
            }
            if type_params.contains(name) {
                checker.pool_mut().named(*name)
            } else {
                resolve_parsed_type_simple(checker, parsed, arena)
            }
        }
        ParsedType::List(elem_id) => {
            let elem = arena.get_parsed_type(*elem_id);
            let elem_ty = resolve_type_with_overlay_inner(
                checker,
                elem,
                method_substitutions,
                type_params,
                self_type,
                arena,
            );
            checker.pool_mut().list(elem_ty)
        }
        ParsedType::Map { key, value } => {
            let key_parsed = arena.get_parsed_type(*key);
            let value_parsed = arena.get_parsed_type(*value);
            let key_ty = resolve_type_with_overlay_inner(
                checker,
                key_parsed,
                method_substitutions,
                type_params,
                self_type,
                arena,
            );
            let value_ty = resolve_type_with_overlay_inner(
                checker,
                value_parsed,
                method_substitutions,
                type_params,
                self_type,
                arena,
            );
            checker.pool_mut().map(key_ty, value_ty)
        }
        ParsedType::Tuple(elems) => {
            let elem_ids = arena.get_parsed_type_list(*elems);
            let elem_types: Vec<Idx> = elem_ids
                .iter()
                .map(|&elem_id| {
                    let elem = arena.get_parsed_type(elem_id);
                    resolve_type_with_overlay_inner(
                        checker,
                        elem,
                        method_substitutions,
                        type_params,
                        self_type,
                        arena,
                    )
                })
                .collect();
            checker.pool_mut().tuple(&elem_types)
        }
        ParsedType::Function { params, ret } => {
            let param_ids = arena.get_parsed_type_list(*params);
            let param_types: Vec<Idx> = param_ids
                .iter()
                .map(|&param_id| {
                    let param = arena.get_parsed_type(param_id);
                    resolve_type_with_overlay_inner(
                        checker,
                        param,
                        method_substitutions,
                        type_params,
                        self_type,
                        arena,
                    )
                })
                .collect();
            let ret_parsed = arena.get_parsed_type(*ret);
            let ret_ty = resolve_type_with_overlay_inner(
                checker,
                ret_parsed,
                method_substitutions,
                type_params,
                self_type,
                arena,
            );
            checker.pool_mut().function(&param_types, ret_ty)
        }
        // Bounded trait object: resolve first bound with self-substitution
        ParsedType::TraitBounds(bounds) => {
            let bound_ids = arena.get_parsed_type_list(*bounds);
            if let Some(&first_id) = bound_ids.first() {
                let first = arena.get_parsed_type(first_id);
                resolve_type_with_overlay_inner(
                    checker,
                    first,
                    method_substitutions,
                    type_params,
                    self_type,
                    arena,
                )
            } else {
                Idx::ERROR
            }
        }
        // Fixed-capacity list `[T, max N]`: resolve the element through the
        // overlay so a projection inside it (`[Self.Item, max N]`) resolves;
        // capacity is erased to a plain list per `TYPES:PT-2`.
        ParsedType::FixedList { elem, capacity: _ } => {
            let elem_parsed = arena.get_parsed_type(*elem);
            let elem_ty = resolve_type_with_overlay_inner(
                checker,
                elem_parsed,
                method_substitutions,
                type_params,
                self_type,
                arena,
            );
            checker.pool_mut().list(elem_ty)
        }
        // Associated-type projection `Self.Item` / `T.Assoc`. Resolve the base
        // first (base-first recursion — base may be `Self` already substituted
        // to a concrete `self_type`, or a nested projection), then project the
        // assoc binding. Symbolic/generic base → clean `Idx::ERROR` poison (the
        // projection legitimately cannot resolve until an impl is selected).
        ParsedType::AssociatedType { base, assoc_name } => {
            let base_parsed = arena.get_parsed_type(*base);
            let base_ty = resolve_type_with_overlay_inner(
                checker,
                base_parsed,
                method_substitutions,
                type_params,
                self_type,
                arena,
            );
            resolve_associated_projection(checker, base_ty, *assoc_name, self_type)
        }
        _ => resolve_parsed_type_simple(checker, parsed, arena),
    }
}

/// Project an associated-type binding for `base.assoc_name` once the base type
/// is concretely known.
///
/// Resolution order:
/// 1. CURRENT impl, in-scope bindings: when `base_ty` is the impl's own
///    `self_type` and the checker carries a `current_impl_assoc` context (set
///    by registration / body-check while resolving this impl's method
///    signatures), read `assoc_name` from the in-scope `type Item = …`
///    bindings — the `ImplEntry` is not yet registered at Pass 0c.
/// 2. CROSS-impl, registered registry: a `find_impl`-shaped lookup over already-
///    registered impls whose `self_type` matches the concrete `base_ty`.
///
/// A non-concrete `base_ty` (type variable / poison) or a concrete miss returns
/// `Idx::ERROR` — clean poison, no spurious diagnostic (`PC-3` / `UN-4`). The
/// concrete-miss case is the genuinely-unresolvable shape; a successful
/// resolution lets a downstream mismatch surface its own `E2001`.
fn resolve_associated_projection(
    checker: &mut ModuleChecker<'_>,
    base_ty: Idx,
    assoc_name: Name,
    self_type: Idx,
) -> Idx {
    // A non-concrete base (unbound/rigid/bound Var, or poison) cannot project to
    // a concrete binding yet — keep the symbolic projection as poison.
    if base_ty == Idx::ERROR || checker.pool().tag(base_ty).is_type_variable() {
        return Idx::ERROR;
    }

    // 1. Current impl's in-scope bindings (registration-ordering cure): the impl
    //    being resolved has its `assoc_types` map installed on the checker before
    //    its `ImplEntry` is registered. Project `Self.Item` from it directly.
    //    Snapshot the impl's `trait_idx` so the cross-impl path below can
    //    disambiguate by `(trait_idx, self_type, assoc_name)` when the concrete
    //    base implements two traits each declaring a same-named associated type.
    let ctx_trait_idx = checker.current_impl_assoc().and_then(|(_, t)| *t);
    if base_ty == self_type {
        if let Some((bindings, _trait_idx)) = checker.current_impl_assoc() {
            if let Some(&projected) = bindings.get(&assoc_name) {
                return projected;
            }
        }
    }

    // 2. Cross-impl projection: find a registered impl whose self type matches the
    //    concrete base and carries this associated-type binding. Pass the current
    //    impl's `trait_idx` so a type implementing two traits with a same-named
    //    associated type resolves the trait-matched binding.
    if let Some(projected) =
        checker
            .trait_registry()
            .find_impl_assoc_binding(ctx_trait_idx, base_ty, assoc_name)
    {
        return projected;
    }

    // Concrete receiver, no matching binding: clean poison.
    Idx::ERROR
}
