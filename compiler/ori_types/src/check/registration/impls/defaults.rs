//! Default-method inheritance and impl validation.

use ori_ir::{Name, Span};
use rustc_hash::{FxHashMap, FxHashSet};

use super::methods::build_impl_method;
use crate::{Idx, ImplMethodDef, ModuleChecker, TraitMethodDef, TypeCheckError};

/// Type-resolution inputs shared while registering one impl's methods.
pub(super) struct ImplBuildContext<'a> {
    /// Impl-level type-generic param names (e.g. `["X"]` for
    /// `impl<X> Reducer<X> for Container<X>`).
    pub(super) type_params: &'a [Name],
    /// Resolved `Self` type for the impl block (e.g. `Container<X>`).
    pub(super) self_type: Idx,
    /// Resolved trait type arguments (e.g. `[pool.named("X")]` for
    /// `Reducer<X>`). Used by Step 1 to build the trait→impl substitution.
    pub(super) trait_type_args: &'a [Idx],
}

/// Inherit unoverridden default methods from trait definitions.
///
/// For trait impls, collects default methods from both the direct trait (AST)
/// and transitive super-traits (registry). Returns the set of "explicit"
/// method names (user-defined + direct defaults, excluding transitive) for
/// use in conflicting default detection.
pub(super) fn inherit_default_methods(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
    traits: &[ori_ir::TraitDef],
    trait_idx: Option<Idx>,
    impl_ctx: &ImplBuildContext<'_>,
    methods: &mut FxHashMap<Name, ImplMethodDef>,
) -> FxHashSet<Name> {
    let Some(trait_path) = &impl_def.trait_path else {
        // Non-trait impls: all methods are explicit
        return methods.keys().copied().collect();
    };

    // Step 1: Direct defaults from the AST trait definition.
    //
    // Inherited default-method binder remapping: `From<&TraitDefaultMethod>
    // for ImplMethod` at `compiler/ori_ir/src/ast/items/traits.rs` shares
    // the parsed param/return types verbatim, so the trait's view of
    // `(T, F) -> T` survives unchanged into the impl-side `ImplMethodDef`.
    // Without a trait→impl substitution, resolving `F` finds neither a
    // method-level overlay entry NOR an impl-level type_param, so it falls
    // through to a dangling `Tag::Named("F")` that fails to unify at call
    // sites (display-equal `int ≠ int` error).
    //
    // Build `trait_subst: trait_param_name → impl_trait_arg_Idx` from the trait
    // declaration's generics zipped with the impl's resolved trait_type_args
    // (e.g. `F → pool.named("X")` for `impl<X> Reducer<X>` over
    // `trait Reducer<F>`). Pass it through `build_impl_method` so the resolver
    // overlay sees both method-level binders (fresh Vars) AND trait-level
    // binders (impl trait args).
    if let Some(&trait_name) = trait_path.last() {
        if let Some(trait_def) = traits.iter().find(|t| t.name == trait_name) {
            // Collect trait-level type-generic param names (skip const).
            let trait_param_names: Vec<Name> = checker
                .arena()
                .get_generic_params(trait_def.generics)
                .iter()
                .filter(|p| !p.is_const)
                .map(|p| p.name)
                .collect();
            // Build the trait→impl substitution by zipping the trait's
            // declared type-params with the impl's resolved trait_type_args.
            // Length mismatch (e.g. arity-error impl) is silently truncated by
            // zip — the type-checker's earlier coherence/arity checks emit the
            // user-facing diagnostic; the inherited body's resolver simply
            // sees no substitution for the unmapped trait params, which
            // degrades to today's behavior (dangling `Tag::Named`).
            let trait_subst: FxHashMap<Name, Idx> = trait_param_names
                .iter()
                .zip(impl_ctx.trait_type_args.iter())
                .map(|(&name, &idx)| (name, idx))
                .collect();
            for item in &trait_def.items {
                if let ori_ir::TraitItem::DefaultMethod(default) = item {
                    methods.entry(default.name).or_insert_with(|| {
                        let as_impl = ori_ir::ImplMethod::from(default);
                        build_impl_method(
                            checker,
                            &as_impl,
                            impl_ctx.type_params,
                            impl_ctx.self_type,
                            &trait_subst,
                        )
                    });
                }
            }
        }
    }

    // Snapshot explicit methods BEFORE transitive defaults are added.
    let explicit_methods: FxHashSet<Name> = methods.keys().copied().collect();

    // Step 2: Transitive defaults from super-trait hierarchy via the registry.
    // Borrow dance: scope the immutable trait_registry borrow to extract the
    // needed data, then use checker mutably for build_impl_method.
    if let Some(t_idx) = trait_idx {
        let transitive_defaults: Vec<TraitMethodDef> = {
            let reg = checker.trait_registry();
            reg.collected_methods(t_idx)
                .into_iter()
                .filter_map(|(name, _owner, def)| {
                    def.default_body?;
                    def.has_default.then(|| {
                        debug_assert_eq!(name, def.name);
                        def.clone()
                    })
                })
                .collect()
        };

        for def in transitive_defaults {
            let Some(body) = def.default_body else {
                continue;
            };
            methods.entry(def.name).or_insert(ImplMethodDef {
                name: def.name,
                signature: def.signature,
                has_self: def.has_self,
                body,
                scheme_var_ids: def.scheme_var_ids,
                generic_param_metadata: def.generic_param_metadata,
                where_clause_metadata: def.where_clause_metadata,
                fixed_list_capacity_constraints: def.fixed_list_capacity_constraints,
                // Inherited trait-default method copy: strict arity (trait
                // default-param carry-through is a follow-up per R3-F3).
                optional_param_count: 0,
                span: def.span,
            });
        }
    }

    explicit_methods
}

/// Validate that all required associated types are defined in the impl.
pub(super) fn validate_assoc_types(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
    trait_idx: Idx,
    assoc_types: &FxHashMap<Name, Idx>,
) {
    // Registration discipline: trait_idx is queried after the Registration group has
    // populated TraitRegistry (CK-1 pass 0c). A None here indicates the caller passed
    // an Idx that bypassed registration — a missing-registration bug, not "trait has
    // no associated types". Surface in debug builds; release continues without an
    // associated-type check (downstream impl-checking already emits diagnostics for
    // the structural problems an unregistered trait would otherwise produce).
    let Some(trait_entry) = checker.trait_registry().get_trait_by_idx(trait_idx) else {
        debug_assert!(
            false,
            "validate_assoc_types called with unregistered trait_idx={trait_idx:?} — \
             Registration group pass 0c must precede this query (CK-1)"
        );
        return;
    };
    let trait_name = trait_entry.name;
    let required: Vec<Name> = trait_entry
        .assoc_types
        .iter()
        .filter(|(_, def)| def.default.is_none())
        .map(|(&name, _)| name)
        .collect();

    for name in required {
        if !assoc_types.contains_key(&name) {
            checker.push_error(TypeCheckError::missing_assoc_type(
                impl_def.span,
                name,
                trait_name,
            ));
        }
    }
}

/// Check for conflicting default methods inherited from multiple super-traits.
///
/// Only reports conflicts for methods not explicitly overridden in the impl.
pub(super) fn check_conflicting_defaults(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
    trait_idx: Idx,
    explicit_methods: &FxHashSet<Name>,
) {
    // Borrow dance: scope the registry borrow to extract conflict data.
    //
    // Registration discipline: provider_idxs comes from find_conflicting_defaults,
    // which already guarded its own super-trait lookup against unregistered ids
    // (CK-1 pass 0c precedence). Every provider Idx returned here MUST resolve via
    // get_trait_by_idx; a None branch indicates registry inconsistency between
    // find_conflicting_defaults's view and the registry's get_trait_by_idx surface.
    // Surface in debug, fall back to filter_map in release so diagnostic emission
    // continues with the nameable providers.
    let conflicts: Vec<(Name, Vec<Name>)> = {
        let reg = checker.trait_registry();
        reg.find_conflicting_defaults(trait_idx)
            .into_iter()
            .map(|(method_name, provider_idxs)| {
                let names: Vec<Name> = provider_idxs
                    .iter()
                    .filter_map(|&idx| {
                        let entry = reg.get_trait_by_idx(idx);
                        debug_assert!(
                            entry.is_some(),
                            "check_conflicting_defaults: super-trait Idx {idx:?} \
                             returned by find_conflicting_defaults is missing from \
                             registry (CK-1 pass 0c invariant breach)"
                        );
                        entry.map(|e| e.name)
                    })
                    .collect();
                (method_name, names)
            })
            .collect()
    };

    for (method_name, provider_names) in conflicts {
        if !explicit_methods.contains(&method_name) && provider_names.len() >= 2 {
            checker.push_error(TypeCheckError::conflicting_defaults(
                impl_def.span,
                method_name,
                provider_names[0],
                provider_names[1],
            ));
        }
    }
}

/// Check for coherence violations (duplicate impls of the same trait for the same type).
///
/// Returns `true` if a violation was found (caller should skip registration).
pub(super) fn has_coherence_violation(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
    trait_idx: Idx,
    self_type: Idx,
    trait_type_args: &[Idx],
) -> bool {
    // Borrow dance: extract existing impl span and trait name, then push error.
    // Uses type-argument-aware matching so that `impl T: Index<int, str>`
    // and `impl T: Index<str, str>` are correctly treated as distinct.
    let existing: Option<(Span, Name)> = {
        let reg = checker.trait_registry();
        reg.find_impl_with_args(trait_idx, self_type, trait_type_args)
            .and_then(|(_, entry)| {
                let trait_name = reg.get_trait_by_idx(trait_idx).map(|t| t.name)?;
                Some((entry.span, trait_name))
            })
    };
    if let Some((first_span, trait_name)) = existing {
        checker.push_error(TypeCheckError::duplicate_impl(
            impl_def.span,
            first_span,
            trait_name,
        ));
        return true;
    }
    false
}
