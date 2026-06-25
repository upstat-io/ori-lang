//! Type resolution helpers for the registration phase.
//!
//! These functions resolve `ParsedType` nodes from the IR into `Idx` type
//! handles in the Pool. They are used across all registration submodules
//! (user types, traits, impls, derived).

use ori_ir::{ExprArena, GenericParamRange, Name, ParsedType, TypeId, WhereClause};
use rustc_hash::FxHashMap;

use crate::{GenericParamMeta, Idx, ModuleChecker, WhereConstraint};

/// Collect type generic parameter names from a generic param range.
///
/// Const generic parameters (`$N: int`) are filtered out — they are values,
/// not types, and should not be bound as type variables.
pub(super) fn collect_generic_params(
    arena: &ExprArena,
    generics: ori_ir::GenericParamRange,
) -> Vec<Name> {
    arena
        .get_generic_params(generics)
        .iter()
        .filter(|param| !param.is_const)
        .map(|param| param.name)
        .collect()
}

/// Collect each type parameter's inline trait bounds (`impl<T: Eq + Clone>`),
/// index-aligned with `collect_generic_params` (same `!is_const` filter and
/// declaration order). One inner `Vec<Name>` of bound trait names per type
/// parameter; an empty inner vec marks an unbounded parameter.
pub(super) fn collect_generic_param_bounds(
    arena: &ExprArena,
    generics: ori_ir::GenericParamRange,
) -> Vec<Vec<Name>> {
    arena
        .get_generic_params(generics)
        .iter()
        .filter(|param| !param.is_const)
        .map(|param| param.bounds.iter().map(ori_ir::TraitBound::name).collect())
        .collect()
}

/// Resolve a struct/enum field type to an Idx during registration.
///
/// Delegates to `resolve_parsed_type_simple` — generic type arguments are not
/// fully instantiated at registration time (deferred to inference). For
/// resolution that is aware of type parameters and Self, use
/// `resolve_type_with_params` or `resolve_type_with_self`.
pub(super) fn resolve_field_type(checker: &mut ModuleChecker<'_>, parsed: &ParsedType) -> Idx {
    let arena = checker.arena();
    resolve_parsed_type_simple(checker, parsed, arena)
}

/// Simplified type resolution for registration phase.
///
/// Handles primitives, lists, maps, tuples, functions, and named types.
/// Generic type arguments are not fully instantiated (deferred to inference).
///
/// Takes `arena` as a separate parameter to avoid borrow conflicts between
/// immutable arena reads and mutable pool writes during recursive resolution.
pub(crate) fn resolve_parsed_type_simple(
    checker: &mut ModuleChecker<'_>,
    parsed: &ParsedType,
    arena: &ExprArena,
) -> Idx {
    match parsed {
        ParsedType::Primitive(type_id) => {
            let raw = type_id.raw();
            if raw < TypeId::PRIMITIVE_COUNT {
                Idx::from_raw(raw)
            } else {
                Idx::ERROR
            }
        }

        ParsedType::List(elem_id) => {
            let elem = arena.get_parsed_type(*elem_id);
            let elem_ty = resolve_parsed_type_simple(checker, elem, arena);
            checker.pool_mut().list(elem_ty)
        }

        ParsedType::Map { key, value } => {
            let key_parsed = arena.get_parsed_type(*key);
            let value_parsed = arena.get_parsed_type(*value);
            let key_ty = resolve_parsed_type_simple(checker, key_parsed, arena);
            let value_ty = resolve_parsed_type_simple(checker, value_parsed, arena);
            checker.pool_mut().map(key_ty, value_ty)
        }

        ParsedType::Tuple(elems) => {
            let elem_ids = arena.get_parsed_type_list(*elems);
            let elem_types: Vec<Idx> = elem_ids
                .iter()
                .map(|&elem_id| {
                    let elem = arena.get_parsed_type(elem_id);
                    resolve_parsed_type_simple(checker, elem, arena)
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
                    resolve_parsed_type_simple(checker, param, arena)
                })
                .collect();
            let ret_parsed = arena.get_parsed_type(*ret);
            let ret_ty = resolve_parsed_type_simple(checker, ret_parsed, arena);
            checker.pool_mut().function(&param_types, ret_ty)
        }

        ParsedType::Named { name, type_args } => {
            // Resolve type arguments if present
            let type_arg_ids = arena.get_parsed_type_list(*type_args);
            let resolved_args: Vec<Idx> = type_arg_ids
                .iter()
                .map(|&arg_id| {
                    let arg = arena.get_parsed_type(arg_id);
                    resolve_parsed_type_simple(checker, arg, arena)
                })
                .collect();

            // Well-known generic types must use their dedicated Pool constructors
            // to ensure type representations match between annotations and inference.
            if !resolved_args.is_empty() {
                if let Some(idx) = checker.resolve_well_known_generic_cached(*name, &resolved_args)
                {
                    return idx;
                }
                return checker.pool_mut().applied(*name, &resolved_args);
            }

            // No type args — check for pre-interned primitives before falling
            // through to pool.named(). Without this, struct fields like
            // `order: Ordering` would get a fresh Named Idx instead of Idx::ORDERING,
            // causing the same duality bug that affected register_builtin_types.
            if let Some(idx) = checker.resolve_registration_primitive(*name) {
                return idx;
            }
            let named_idx = checker.pool_mut().named(*name);
            // FFI types (CPtr, c_int, ...) get a Pool resolution so downstream
            // phases classify them as trivial scalars; the CAbiKind carrier keeps
            // the C-ABI width identity that resolution discards (pair via attach_ffi_carrier).
            if let (Some(concrete), Some(kind)) = (
                checker.resolve_ffi_concrete(*name),
                checker.resolve_ffi_cabi_kind(*name),
            ) {
                checker
                    .pool_mut()
                    .attach_ffi_carrier(named_idx, concrete, kind);
            }
            named_idx
        }

        ParsedType::FixedList { elem, capacity: _ } => {
            // Treat as regular list for now
            let elem_parsed = arena.get_parsed_type(*elem);
            let elem_ty = resolve_parsed_type_simple(checker, elem_parsed, arena);
            checker.pool_mut().list(elem_ty)
        }

        // These types need special handling during inference.
        // ConstExpr uses ERROR here (not fresh_var) because registration needs
        // deterministic types for Pool interning. Inference (infer/expr.rs) uses
        // fresh_var instead to allow optimistic deferral.
        ParsedType::Infer
        | ParsedType::SelfType
        | ParsedType::AssociatedType { .. }
        | ParsedType::ConstExpr(_) => Idx::ERROR,

        // Bounded trait object: resolve first bound as primary type
        ParsedType::TraitBounds(bounds) => {
            let bound_ids = arena.get_parsed_type_list(*bounds);
            if let Some(&first_id) = bound_ids.first() {
                let first = arena.get_parsed_type(first_id);
                resolve_parsed_type_simple(checker, first, arena)
            } else {
                Idx::ERROR
            }
        }
    }
}

/// Resolve a parsed type with type parameters in scope.
///
/// Type parameters are looked up by name and replaced with fresh type variables
/// during inference. `Self` becomes a Named placeholder. Recurses into compound
/// types (List, Map, Tuple, Function, `TraitBounds`) so that nested `Self` and
/// type parameter references are resolved correctly — without this recursion,
/// types like `-> List<Self>` or `-> (T, int)` would silently map the nested
/// `Self`/param to `Idx::ERROR` via `resolve_parsed_type_simple`.
pub(super) fn resolve_type_with_params(
    checker: &mut ModuleChecker<'_>,
    parsed: &ParsedType,
    type_params: &[Name],
    arena: &ExprArena,
) -> Idx {
    match parsed {
        ParsedType::Named { name, .. } => {
            if type_params.contains(name) {
                checker.pool_mut().named(*name)
            } else {
                resolve_parsed_type_simple(checker, parsed, arena)
            }
        }
        ParsedType::SelfType => {
            let self_name = checker.interner().intern("Self");
            checker.pool_mut().named(self_name)
        }
        ParsedType::List(elem_id) => {
            let elem = arena.get_parsed_type(*elem_id);
            let elem_ty = resolve_type_with_params(checker, elem, type_params, arena);
            checker.pool_mut().list(elem_ty)
        }
        ParsedType::Map { key, value } => {
            let key_parsed = arena.get_parsed_type(*key);
            let value_parsed = arena.get_parsed_type(*value);
            let key_ty = resolve_type_with_params(checker, key_parsed, type_params, arena);
            let value_ty = resolve_type_with_params(checker, value_parsed, type_params, arena);
            checker.pool_mut().map(key_ty, value_ty)
        }
        ParsedType::Tuple(elems) => {
            let elem_ids = arena.get_parsed_type_list(*elems);
            let elem_types: Vec<Idx> = elem_ids
                .iter()
                .map(|&elem_id| {
                    let elem = arena.get_parsed_type(elem_id);
                    resolve_type_with_params(checker, elem, type_params, arena)
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
                    resolve_type_with_params(checker, param, type_params, arena)
                })
                .collect();
            let ret_parsed = arena.get_parsed_type(*ret);
            let ret_ty = resolve_type_with_params(checker, ret_parsed, type_params, arena);
            checker.pool_mut().function(&param_types, ret_ty)
        }
        ParsedType::TraitBounds(bounds) => {
            let bound_ids = arena.get_parsed_type_list(*bounds);
            if let Some(&first_id) = bound_ids.first() {
                let first = arena.get_parsed_type(first_id);
                resolve_type_with_params(checker, first, type_params, arena)
            } else {
                Idx::ERROR
            }
        }
        _ => resolve_parsed_type_simple(checker, parsed, arena),
    }
}

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

/// Check if a `ParsedType` tree contains `SelfType` anywhere.
///
/// Recursively walks the type tree looking for `ParsedType::SelfType`.
/// Used for object safety analysis: methods returning `Self` or taking
/// `Self` as a non-receiver parameter make a trait non-object-safe.
pub(super) fn parsed_type_contains_self(arena: &ori_ir::ExprArena, ty: &ParsedType) -> bool {
    match ty {
        ParsedType::SelfType => true,

        // Leaf types — never contain Self
        ParsedType::Primitive(_) | ParsedType::Infer | ParsedType::ConstExpr(_) => false,

        // Named types — check type arguments
        ParsedType::Named { type_args, .. } => {
            let args = arena.get_parsed_type_list(*type_args);
            args.iter()
                .any(|&id| parsed_type_contains_self(arena, arena.get_parsed_type(id)))
        }

        // Container types — check children
        ParsedType::List(elem) | ParsedType::FixedList { elem, .. } => {
            parsed_type_contains_self(arena, arena.get_parsed_type(*elem))
        }
        ParsedType::Map { key, value } => {
            parsed_type_contains_self(arena, arena.get_parsed_type(*key))
                || parsed_type_contains_self(arena, arena.get_parsed_type(*value))
        }
        ParsedType::Tuple(elems) | ParsedType::TraitBounds(elems) => {
            let ids = arena.get_parsed_type_list(*elems);
            ids.iter()
                .any(|&id| parsed_type_contains_self(arena, arena.get_parsed_type(id)))
        }
        ParsedType::Function { params, ret } => {
            let param_ids = arena.get_parsed_type_list(*params);
            param_ids
                .iter()
                .any(|&id| parsed_type_contains_self(arena, arena.get_parsed_type(id)))
                || parsed_type_contains_self(arena, arena.get_parsed_type(*ret))
        }
        ParsedType::AssociatedType { base, .. } => {
            parsed_type_contains_self(arena, arena.get_parsed_type(*base))
        }
    }
}

/// Convert IR visibility to Types visibility.
pub(super) fn convert_visibility(ir_vis: ori_ir::Visibility) -> crate::Visibility {
    match ir_vis {
        ori_ir::Visibility::Public => crate::Visibility::Public,
        ori_ir::Visibility::Private => crate::Visibility::Private,
    }
}

/// Build a `WhereConstraint` from a single AST `WhereClause`.
///
/// Returns `None` for const bounds (deferred per `WhereClause::as_type_bound`).
///
/// `type_params` is the combined scope of outer + method-level type params;
/// `self_type` substitutes `Self` references in the constrained-param position
/// (pass `Idx::ERROR` in trait-method context where `Self` remains symbolic).
///
/// `scheme_overlay` maps method-level binder names to the fresh `Tag::Var`
/// Idx allocated for them at registration.
/// When `param` is a method-level binder, `wc.ty` is the same Idx that flows
/// through the registered method signature's `Tag::Scheme` body. Call-site
/// `instantiate_with_subst` substitutes that Idx with a fresh Var Idx; the
/// downstream `check_method_where_clauses` helper applies the same
/// substitution to `wc.ty` before resolving so the bound check sees the
/// post-instantiation concrete type. Without this overlay, `wc.ty` was a
/// fresh `Tag::Named(param)` lookup that union-find never updated (INVERTED-TDD:goal-drift).
pub(crate) fn build_where_constraint(
    checker: &mut ModuleChecker<'_>,
    wc: &WhereClause,
    type_params: &[Name],
    scheme_overlay: &FxHashMap<Name, Idx>,
    self_type: Idx,
) -> Option<WhereConstraint> {
    let (param, _projection, bounds, _span) = wc.as_type_bound()?;

    let ty = if let Some(&overlay_idx) = scheme_overlay.get(&param) {
        overlay_idx
    } else if type_params.contains(&param) {
        checker.pool_mut().named(param)
    } else if param == checker.interner().intern("Self") {
        self_type
    } else {
        checker.pool_mut().named(param)
    };

    let resolved_bounds: Vec<Idx> = bounds
        .iter()
        .map(|b| checker.pool_mut().named(b.name()))
        .collect();

    Some(WhereConstraint {
        ty,
        bounds: resolved_bounds,
    })
}

/// Deep-copy method-level generics + where-clauses into arena-independent owned form.
///
/// Returns `(scheme_var_ids, scheme_overlay, generic_param_metadata, where_clause_metadata)`:
/// - `scheme_var_ids` — pool `var_ids` parallel to declaration order of TYPE-generic params (skip const)
/// - `scheme_overlay` — `Name → Idx` map from each method-level type-generic name to the fresh
///   `Tag::Var` Idx allocated for it. Used by callers (e.g. `build_impl_method`) as the
///   substitution overlay for `resolve_type_with_method_generics`, so method-level type-name
///   references in the registered signature resolve to the SAME `Tag::Var` Idx whose `var_id` is
///   recorded in `scheme_var_ids`. The signature is then wrappable in `Tag::Scheme(scheme_var_ids,
///   fn_type)` and `engine.instantiate(...)` at call-site replaces those vars with fresh ones —
///   the `GN-2` pattern extended to method-level binders.
/// - `generic_param_metadata` — full owned `GenericParamMeta` per param (type AND const), declaration order
/// - `where_clause_metadata` — type-bound `WhereConstraint` entries (const bounds skipped)
///
/// `outer_type_params` carries impl-level / trait-level type params already in scope.
/// `self_type` is `Idx::ERROR` in trait-method context (Self stays symbolic) or the
/// implementing type in impl-method context.
pub(crate) fn build_method_generic_metadata(
    checker: &mut ModuleChecker<'_>,
    generics: GenericParamRange,
    where_clauses: &[WhereClause],
    outer_type_params: &[Name],
    self_type: Idx,
) -> (
    Vec<u32>,
    FxHashMap<Name, Idx>,
    Vec<GenericParamMeta>,
    Vec<WhereConstraint>,
) {
    let generic_params = checker.arena().get_generic_params(generics).to_vec();

    let method_type_param_names: Vec<Name> = generic_params
        .iter()
        .filter(|p| !p.is_const)
        .map(|p| p.name)
        .collect();
    let combined_scope: Vec<Name> = outer_type_params
        .iter()
        .copied()
        .chain(method_type_param_names.iter().copied())
        .collect();

    let mut scheme_var_ids: Vec<u32> = Vec::new();
    let mut scheme_overlay: FxHashMap<Name, Idx> = FxHashMap::default();
    let mut param_meta: Vec<GenericParamMeta> = Vec::with_capacity(generic_params.len());

    for p in &generic_params {
        let bounds: Vec<Idx> = p
            .bounds
            .iter()
            .map(|tb| checker.pool_mut().named(tb.name()))
            .collect();

        let default_type = p
            .default_type
            .as_ref()
            .map(|dt| resolve_type_with_self(checker, dt, &combined_scope, self_type));

        let const_type = p
            .const_type
            .as_ref()
            .map(|ct| resolve_type_with_self(checker, ct, &combined_scope, self_type));

        if !p.is_const {
            let var_idx = checker.pool_mut().fresh_named_var(p.name);
            let var_id = checker.pool().data(var_idx);
            scheme_var_ids.push(var_id);
            scheme_overlay.insert(p.name, var_idx);
        }

        param_meta.push(GenericParamMeta {
            name: p.name,
            is_const: p.is_const,
            bounds,
            default_type,
            const_type,
            const_default_value: None,
            projection_bounds: Vec::new(),
        });
    }

    let where_meta: Vec<WhereConstraint> = where_clauses
        .iter()
        .filter_map(|wc| {
            build_where_constraint(checker, wc, &combined_scope, &scheme_overlay, self_type)
        })
        .collect();

    (scheme_var_ids, scheme_overlay, param_meta, where_meta)
}
