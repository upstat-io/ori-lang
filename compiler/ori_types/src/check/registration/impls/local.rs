//! Source implementation registration and Drop burden wiring.

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use super::super::type_resolution::{
    build_where_constraint, collect_generic_param_bounds, collect_generic_params,
    resolve_parsed_type_simple, resolve_type_with_self,
};
use super::defaults::{
    check_conflicting_defaults, has_coherence_violation, inherit_default_methods,
    validate_assoc_types, ImplBuildContext,
};
use super::methods::build_impl_method;
use crate::registry::burden::UserBurdenSpec;
use crate::registry::burden_compose::scc::mint_drop_operation_sym;
use crate::{
    Idx, ImplEntry, ImplMethodDef, ImplMethodId, ImplMethodRole, ImplSpecificity, ModuleChecker,
    TypeCheckError, WhereConstraint,
};

/// Register a single implementation.
///
/// Converts an `ori_ir::ImplDef` to an `ImplEntry` and registers it in the
/// `TraitRegistry`. Handles both inherent impls (`impl Type { ... }`) and
/// trait impls (`impl Type: Trait { ... }`).
pub(super) fn register_impl(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
    traits: &[ori_ir::TraitDef],
    impl_index: usize,
) {
    // 1. Collect generic parameters
    let arena = checker.arena();
    let type_params = collect_generic_params(arena, impl_def.generics);
    let type_param_bounds = collect_generic_param_bounds(arena, impl_def.generics);
    debug_assert_eq!(
        type_params.len(),
        type_param_bounds.len(),
        "type_param_bounds must be index-aligned with type_params"
    );

    // 2. Resolve self type
    let self_type = resolve_parsed_type_simple(checker, &impl_def.self_ty, arena);

    // 3. Resolve trait reference (if trait impl): trait name + type arguments.
    let (trait_idx, trait_type_args) = resolve_impl_trait_ref(checker, impl_def);

    // 4. Process associated type definitions FIRST.
    //
    // Registration-ordering: a method whose declared param/return type carries
    // `Self.Item` must resolve that projection against this impl's own
    // `type Item = …` bindings — but the `ImplEntry` (and so the trait
    // registry) is not registered until step 9 below. Build the bindings here,
    // before the method loop, and install them on the checker as the
    // `current_impl_assoc` scope so `resolve_type_with_overlay_inner`'s
    // `ParsedType::AssociatedType` arm projects `Self.Item` from the in-scope
    // map The binding RHS (`type Item = int`) is concrete, so its
    // own resolution never recurses into an unresolvable projection.
    let mut assoc_types = FxHashMap::default();
    for impl_assoc in &impl_def.assoc_types {
        let ty = resolve_type_with_self(checker, &impl_assoc.ty, &type_params, self_type);
        assoc_types.insert(impl_assoc.name, ty);
    }

    // 5. Process explicitly defined methods + inherited defaults, under the
    //    associated-type projection scope so `Self.Item` resolves.
    let impl_context = ImplBuildContext {
        type_params: &type_params,
        self_type,
        trait_type_args: &trait_type_args,
    };
    let (methods, explicit_methods) = build_impl_methods(
        checker,
        impl_def,
        traits,
        trait_idx,
        &assoc_types,
        &impl_context,
    );

    // 6. Process where clauses (const bounds filtered out — not yet evaluated)
    // Empty scheme_overlay: impl-level where-clauses don't reference method-level
    // binders. Method-level where-clauses go through `build_method_generic_metadata`
    // with a populated overlay.
    let empty_overlay: FxHashMap<Name, Idx> = FxHashMap::default();
    let where_clause: Vec<WhereConstraint> = impl_def
        .where_clauses
        .iter()
        .filter_map(|wc| {
            build_where_constraint(checker, wc, &type_params, &empty_overlay, self_type)
        })
        .collect();

    // 7. Validate associated types, check conflicting defaults, check coherence.
    // Why: an unregistered `trait_idx` (no `TraitEntry` — missing prelude or a typo'd
    // trait name) would ICE the `validate_assoc_types` / `check_conflicting_defaults`
    // debug_asserts; the guard emits a clean E2003 and skips validation, while the
    // impl still registers structurally below so other diagnostics flow.
    if let Some(t_idx) = trait_idx {
        if checker.trait_registry().get_trait_by_idx(t_idx).is_none() {
            let trait_name = impl_def
                .trait_path
                .as_ref()
                .and_then(|path| path.last().copied())
                .unwrap_or_else(|| checker.interner().intern("<trait>"));
            checker.push_error(TypeCheckError::unresolved_trait(impl_def.span, trait_name));
        } else {
            validate_assoc_types(checker, impl_def, t_idx, &assoc_types);
            check_conflicting_defaults(checker, impl_def, t_idx, &explicit_methods);
            if has_coherence_violation(checker, impl_def, t_idx, self_type, &trait_type_args) {
                return;
            }
        }
    }

    // 8. Compute specificity. A non-empty inline bound (`impl<T: Eq>`) makes the
    //    impl Constrained exactly as a trailing `where` clause does.
    let has_inline_bound = type_param_bounds.iter().any(|b| !b.is_empty());
    let specificity = if type_params.is_empty() {
        ImplSpecificity::Concrete
    } else if !where_clause.is_empty() || has_inline_bound {
        ImplSpecificity::Constrained
    } else {
        ImplSpecificity::Generic
    };

    // 9. Register in TraitRegistry
    let entry = ImplEntry {
        trait_idx,
        trait_type_args,
        self_type,
        type_params,
        type_param_bounds,
        methods,
        assoc_types,
        where_clause,
        specificity,
        span: impl_def.span,
    };

    checker.trait_registry_mut().register_impl_with_origin(
        entry,
        Some(crate::registry::RegisteredImplOrigin::Source { impl_index }),
    );

    // 10. Drop trait wiring: when this impl is `impl T: Drop`, populate
    //     `UserBurdenSpec.user_drop = Some(FnSym)` and mint a stable
    //     `drop_operation = Some(FnSym)`. The logical operation orders user
    //     @drop before reverse-declaration field cleanup; each physical
    //     projection chooses how to realize it. The decision
    //     rule: `drop_operation = Some(_) iff (in non-singleton SCC) OR
    //     (self-loop) OR (user_drop = Some(_))`. The third clause fires
    //     here for every Drop type, including non-recursive ones —
    //     they need an executable cleanup identity.
    //
    //     Drop is explicit-impl-only per `drop-trait-proposal.md
    //     §Auto-derive`; population happens at this `register_impl`
    //     site, NOT `register_derived_impl`.
    if let Some(logical) =
        populate_drop_burden_if_applicable(checker, impl_def, self_type, trait_idx)
    {
        let drop_method = checker.well_known().drop_method;
        for method in &impl_def.methods {
            if method.name == drop_method {
                checker.register_impl_method_role(
                    ImplMethodId::new(impl_index, method.body),
                    ImplMethodRole::UserDrop { logical },
                );
            }
        }
    }
}

/// Build explicit and inherited methods for one impl registration.
fn build_impl_methods(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
    traits: &[ori_ir::TraitDef],
    trait_idx: Option<Idx>,
    assoc_types: &FxHashMap<Name, Idx>,
    impl_context: &ImplBuildContext<'_>,
) -> (FxHashMap<Name, ImplMethodDef>, FxHashSet<Name>) {
    checker.with_impl_assoc_scope(assoc_types.clone(), trait_idx, |checker| {
        // Explicit methods use impl-level binders directly. Trait-to-impl
        // substitution is needed only when inherited defaults are added below.
        let empty_trait_subst: FxHashMap<Name, Idx> = FxHashMap::default();
        let mut methods = FxHashMap::default();
        for impl_method in &impl_def.methods {
            let method_def = build_impl_method(
                checker,
                impl_method,
                impl_context.type_params,
                impl_context.self_type,
                &empty_trait_subst,
            );
            methods.insert(impl_method.name, method_def);
        }

        // Inherit unoverridden defaults from direct and transitive traits.
        let explicit_methods = inherit_default_methods(
            checker,
            impl_def,
            traits,
            trait_idx,
            impl_context,
            &mut methods,
        );
        (methods, explicit_methods)
    })
}

/// Resolve a trait impl's trait reference: the trait's pool `Idx` (None for
/// inherent impls) plus its resolved type arguments (e.g., `<int, str>` in
/// `impl T: Index<int, str>`).
///
/// Parser invariant: a `trait_path: Some(_)` impl block carries at least one
/// path segment (`impl T: { }` does not parse). An empty `path` indicates a
/// parser-level invariant breach upstream; debug builds surface the violation
/// while release builds fall back to a synthetic "<unknown>" name so the rest
/// of registration continues producing diagnostics rather than panicking.
fn resolve_impl_trait_ref(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
) -> (Option<Idx>, Vec<Idx>) {
    let arena = checker.arena();
    let trait_idx = impl_def.trait_path.as_ref().map(|path| {
        let trait_name = path.last().copied().unwrap_or_else(|| {
            debug_assert!(
                false,
                "register_impl received Some(trait_path) with empty path segments — \
                 parser invariant violated for impl_def.span={:?}",
                impl_def.span
            );
            checker.interner().intern("<unknown>")
        });
        checker.pool_mut().named(trait_name)
    });

    let trait_type_args: Vec<Idx> = {
        let arg_ids = arena.get_parsed_type_list(impl_def.trait_type_args);
        arg_ids
            .iter()
            .map(|&arg_id| {
                let parsed = arena.get_parsed_type(arg_id);
                resolve_parsed_type_simple(checker, parsed, arena)
            })
            .collect()
    };

    (trait_idx, trait_type_args)
}

/// When `impl_def` is a `Drop` impl on `self_type`, populate
/// `UserBurdenSpec.user_drop` and `drop_operation` on the type's burden
/// entry.
///
/// Resolves `Drop`'s trait `Idx` via the interner; gracefully no-ops if
/// the trait is not yet registered (pre-deployment shape).
///
/// `drop_operation` is minted through the shared SCC identity helper so
/// recursive, closure, and user-Drop populators use one stable logical ID
/// space. Physical helper naming is not part of this contract.
fn populate_drop_burden_if_applicable(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
    self_type: Idx,
    trait_idx: Option<Idx>,
) -> Option<ori_registry::burden::FnSym> {
    let t_idx = trait_idx?;

    // Resolve the language-defined Drop identity through the frontend's
    // well-known-name SSOT. Downstream phases never classify by spelling.
    let drop_name = checker.well_known().drop_trait;
    let drop_trait_entry = checker.trait_registry().get_trait_by_name(drop_name)?;
    if drop_trait_entry.idx != t_idx {
        return None;
    }

    // Value/Drop conflict detection (E2049).
    //
    // When `impl T: Drop` is being registered AND T's trait set already
    // carries `Value` (recorded at the type-decl registration site via
    // `TypeRegistry::record_value_marker`), the two markers mutually
    // exclude per Annex E §AIMS: `Value` declares no independent ownership
    // identity, so no logical cleanup transition exists for `@drop` to extend.
    //
    // The span points at the `impl T: Drop` block (the second
    // registration to land); the diagnostic still permits Phase 5 to
    // proceed with whichever burden spec was already populated for T
    // (the executable-projection gate at the driver level — `PC-4` —
    // suppresses emission when any typeck error remains).
    if checker.type_registry().carries_value_marker(self_type) {
        // Resolve the type's name from the registry for the diagnostic.
        // Fall back to the placeholder `<unknown>` when the type is not
        // registered (shouldn't happen for well-formed input, but the
        // graceful fallback matches the existing pattern in
        // `register_impl`).
        let type_name = checker.type_registry().get_by_idx(self_type).map_or_else(
            || checker.interner().intern("<unknown>"),
            |entry| entry.name,
        );
        checker.push_error(TypeCheckError::value_drop_conflict(
            impl_def.span,
            type_name,
        ));
        // Do NOT short-circuit Drop wiring: emitting the diagnostic
        // suppresses executable projection at the driver level, but Phase 5 may still
        // touch the type's burden. Wiring `user_drop` / `drop_operation`
        // below keeps the spec internally consistent (matches the
        // shape of well-formed Drop types) until the user resolves the
        // conflict by removing one of the two markers.
    }

    let user_drop_fn_sym = mint_drop_operation_sym(self_type);

    // Look up existing burden + merge: preserve any spec already
    // computed by `burden_compute` at type-registration time;
    // overlay the user_drop / drop_operation fields.
    let existing = checker
        .type_registry()
        .burden(self_type)
        .cloned()
        .unwrap_or_default();
    let merged = UserBurdenSpec {
        user_drop: Some(user_drop_fn_sym),
        drop_operation: Some(mint_drop_operation_sym(self_type)),
        ..existing
    };
    checker
        .type_registry_mut()
        .register_user_burden(self_type, merged);
    Some(user_drop_fn_sym)
}
