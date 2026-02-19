//! Implementation block registration (Pass 0c, part 2).
//!
//! Registers both inherent impls (`impl Type { ... }`) and trait impls
//! (`impl Trait for Type { ... }`). Handles default method inheritance,
//! super-trait transitive defaults, associated types, where clauses,
//! coherence checks, and specificity computation.

use ori_ir::{ExprId, Name, Span};
use rustc_hash::{FxHashMap, FxHashSet};

use super::type_resolution::{
    collect_generic_params, resolve_parsed_type_simple, resolve_type_with_self,
};
use crate::{
    Idx, ImplEntry, ImplMethodDef, ImplSpecificity, ModuleChecker, TypeCheckError, WhereConstraint,
};

/// Register implementation blocks.
///
/// For trait impls, also registers unoverridden default methods so they're
/// visible during method resolution in function body checking (Pass 2).
pub fn register_impls(checker: &mut ModuleChecker<'_>, module: &ori_ir::Module) {
    for impl_def in &module.impls {
        register_impl(checker, impl_def, &module.traits);
    }
}

/// Register a single implementation.
///
/// Converts an `ori_ir::ImplDef` to an `ImplEntry` and registers it in the
/// `TraitRegistry`. Handles both inherent impls (`impl Type { ... }`) and
/// trait impls (`impl Trait for Type { ... }`).
#[expect(
    clippy::too_many_lines,
    reason = "exhaustive impl registration covering inherent and trait impls with method signature collection"
)]
fn register_impl(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
    traits: &[ori_ir::TraitDef],
) {
    // 1. Collect generic parameters
    let arena = checker.arena();
    let type_params = collect_generic_params(arena, impl_def.generics);

    // 2. Resolve self type
    let self_type = resolve_parsed_type_simple(checker, &impl_def.self_ty, arena);

    // 3. Resolve trait (if trait impl)
    let trait_idx = impl_def.trait_path.as_ref().map(|path| {
        // Use the last segment of the trait path as the trait name
        let trait_name = path
            .last()
            .copied()
            .unwrap_or_else(|| checker.interner().intern("<unknown>"));
        checker.pool_mut().named(trait_name)
    });

    // 3b. Resolve trait type arguments (e.g., `<int, str>` in `impl Index<int, str> for T`)
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

    // 4. Process explicitly defined methods
    let mut methods = FxHashMap::default();
    for impl_method in &impl_def.methods {
        let method_def = build_impl_method(checker, impl_method, &type_params, self_type);
        methods.insert(impl_method.name, method_def);
    }

    // 4b. For trait impls, register unoverridden default methods (direct + transitive)
    //
    // explicit_methods tracks methods from steps 3+4b (explicit impl methods + direct
    // trait defaults). Step 6c uses this to detect conflicting defaults — transitive
    // defaults must NOT be in this set, otherwise conflicts are silently masked.
    let explicit_methods: FxHashSet<Name>;
    if let Some(trait_path) = &impl_def.trait_path {
        // Step 1: Direct defaults from the AST trait definition
        if let Some(&trait_name) = trait_path.last() {
            if let Some(trait_def) = traits.iter().find(|t| t.name == trait_name) {
                for item in &trait_def.items {
                    if let ori_ir::TraitItem::DefaultMethod(default) = item {
                        methods.entry(default.name).or_insert_with(|| {
                            let as_impl = ori_ir::ImplMethod::from(default);
                            build_impl_method(checker, &as_impl, &type_params, self_type)
                        });
                    }
                }
            }
        }

        // Snapshot explicit methods BEFORE transitive defaults are added.
        explicit_methods = methods.keys().copied().collect();

        // Step 2: Transitive defaults from super-trait hierarchy via the registry.
        // Borrow dance: scope the immutable trait_registry borrow to extract the
        // data we need, then use checker mutably for build_impl_method.
        if let Some(t_idx) = trait_idx {
            let transitive_defaults: Vec<(Name, Idx, ExprId, Span)> = {
                let reg = checker.trait_registry();
                reg.collected_methods(t_idx)
                    .into_iter()
                    .filter_map(|(name, _owner, def)| {
                        let body = def.default_body?;
                        if !def.has_default {
                            return None;
                        }
                        Some((name, def.signature, body, def.span))
                    })
                    .collect()
            };

            for (name, signature, body, span) in transitive_defaults {
                methods.entry(name).or_insert(ImplMethodDef {
                    name,
                    signature,
                    has_self: true,
                    body,
                    span,
                });
            }
        }
    } else {
        // Non-trait impls: all methods are explicit
        explicit_methods = methods.keys().copied().collect();
    }

    // 5. Process associated type definitions
    let mut assoc_types = FxHashMap::default();
    for impl_assoc in &impl_def.assoc_types {
        let ty = resolve_type_with_self(checker, &impl_assoc.ty, &type_params, self_type);
        assoc_types.insert(impl_assoc.name, ty);
    }

    // 6. Process where clauses (const bounds filtered out — not yet evaluated)
    let where_clause: Vec<WhereConstraint> = impl_def
        .where_clauses
        .iter()
        .filter_map(|wc| build_where_constraint(checker, wc, &type_params, self_type))
        .collect();

    // 6b. Validate all required associated types are defined
    if let Some(t_idx) = trait_idx {
        if let Some(trait_entry) = checker.trait_registry().get_trait_by_idx(t_idx) {
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
    }

    // 6c. Check for conflicting default methods from super-traits
    if let Some(t_idx) = trait_idx {
        // Borrow dance: scope the registry borrow to extract conflict data
        let conflicts: Vec<(Name, Vec<Name>)> = {
            let reg = checker.trait_registry();
            reg.find_conflicting_defaults(t_idx)
                .into_iter()
                .map(|(method_name, provider_idxs)| {
                    let names: Vec<Name> = provider_idxs
                        .iter()
                        .filter_map(|&idx| reg.get_trait_by_idx(idx).map(|e| e.name))
                        .collect();
                    (method_name, names)
                })
                .collect()
        };

        for (method_name, provider_names) in conflicts {
            // Only report if the impl doesn't explicitly override the method.
            // Check against explicit_methods (step 3 + step 4b direct defaults),
            // NOT the full methods map which includes transitive defaults.
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

    // 7. Check for coherence violations
    if let Some(t_idx) = trait_idx {
        // Borrow dance: extract existing impl span and trait name, then push error.
        // Uses type-argument-aware matching so that `impl Index<int, str> for T`
        // and `impl Index<str, str> for T` are correctly treated as distinct.
        let existing: Option<(Span, Name)> = {
            let reg = checker.trait_registry();
            reg.find_impl_with_args(t_idx, self_type, &trait_type_args)
                .and_then(|(_, entry)| {
                    let trait_name = reg.get_trait_by_idx(t_idx).map(|t| t.name)?;
                    Some((entry.span, trait_name))
                })
        };
        if let Some((first_span, trait_name)) = existing {
            checker.push_error(TypeCheckError::duplicate_impl(
                impl_def.span,
                first_span,
                trait_name,
            ));
            return;
        }
    }

    // 8. Compute specificity
    let specificity = if type_params.is_empty() {
        ImplSpecificity::Concrete
    } else if !where_clause.is_empty() {
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
        methods,
        assoc_types,
        where_clause,
        specificity,
        span: impl_def.span,
    };

    checker.trait_registry_mut().register_impl(entry);
}

/// Build an `ImplMethodDef` from an impl method.
fn build_impl_method(
    checker: &mut ModuleChecker<'_>,
    method: &ori_ir::ImplMethod,
    type_params: &[Name],
    self_type: Idx,
) -> ImplMethodDef {
    // Resolve parameter types, substituting Self with the actual type
    let params: Vec<_> = checker.arena().get_params(method.params).to_vec();
    let param_types: Vec<Idx> = params
        .iter()
        .map(|p| {
            let is_self = p.name == checker.well_known().self_kw;
            match p.ty.as_ref() {
                Some(ty) => resolve_type_with_self(checker, ty, type_params, self_type),
                None if is_self => self_type,
                None => Idx::ERROR,
            }
        })
        .collect();

    // Resolve return type (return_ty is a ParsedType, not Option)
    let return_ty = resolve_type_with_self(checker, &method.return_ty, type_params, self_type);

    // Detect whether the first parameter is `self` (instance method vs associated function)
    let has_self = params
        .first()
        .is_some_and(|p| p.name == checker.well_known().self_kw);

    // Create function type for signature
    let signature = checker.pool_mut().function(&param_types, return_ty);

    ImplMethodDef {
        name: method.name,
        signature,
        has_self,
        body: method.body,
        span: method.span,
    }
}

/// Build a `WhereConstraint` from a where clause.
///
/// Returns `None` for const bounds (not yet evaluated).
fn build_where_constraint(
    checker: &mut ModuleChecker<'_>,
    wc: &ori_ir::WhereClause,
    type_params: &[Name],
    self_type: Idx,
) -> Option<WhereConstraint> {
    let (param, _projection, bounds, _span) = wc.as_type_bound()?;

    // Resolve the constrained type parameter
    let ty = if type_params.contains(&param) {
        checker.pool_mut().named(param)
    } else if param == checker.interner().intern("Self") {
        self_type
    } else {
        // Fallback to named type
        checker.pool_mut().named(param)
    };

    // Resolve the trait bounds
    // TraitBound has `first` and `rest` fields for path segments
    // Use the `name()` method to get the last segment (the actual trait name)
    let resolved_bounds: Vec<Idx> = bounds
        .iter()
        .map(|bound| {
            // Use the name() method which returns the last segment (or first if rest is empty)
            checker.pool_mut().named(bound.name())
        })
        .collect();

    Some(WhereConstraint {
        ty,
        bounds: resolved_bounds,
    })
}
