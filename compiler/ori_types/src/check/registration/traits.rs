//! Trait definition registration (Pass 0c, part 1).
//!
//! Registers trait definitions from the IR into the `TraitRegistry`. This enables
//! method resolution and trait bound checking. Handles both local and imported
//! (foreign-module) traits.

use ori_ir::{ExprArena, Name, TraitItem};
use rustc_hash::FxHashMap;

use super::type_resolution::{
    build_method_generic_metadata, collect_generic_params, parsed_type_contains_self,
    resolve_type_with_params,
};
use crate::const_eval::collect_method_capacity_constraints;
use crate::{
    Idx, ModuleChecker, ObjectSafetyViolation, TraitAssocTypeDef, TraitEntry, TraitMethodDef,
};

/// Register trait definitions.
pub fn register_traits(checker: &mut ModuleChecker<'_>, module: &ori_ir::Module) {
    let arena = checker.arena();
    for trait_def in &module.traits {
        register_trait(checker, trait_def, arena);
    }
}

/// Propagate object-safety violations from super-traits to their children.
///
/// This is the second phase of two-phase trait registration:
/// `register_traits` populates each trait's *direct* `object_safety_violations`
/// from its own `items` (Rules 1, 2, 3 against the trait's own methods); this
/// pass walks each trait's transitive super-trait DAG and propagates inherited
/// `GenericMethod` violations to the child, with the child's span as the
/// declaration site of the inherited violation.
///
/// The pass MUST run BETWEEN `register_traits` and `register_impls` so that
/// impl-resolution sees the correct violation set on parent traits when
/// constructing trait-object types.
///
/// # Order-Independence
///
/// Naïve "iterate super-traits during `register_trait`" is source-order
/// dependent — if `Sub` is declared before `Super`, `Super`'s `TraitEntry`
/// doesn't exist yet. Two-phase registration solves this: by the time this
/// pass runs, every trait is registered, so super-trait queries always
/// succeed regardless of declaration order.
///
/// # Multi-level Hierarchies
///
/// `TraitRegistry::all_super_traits` performs the transitive DAG walk with
/// cycle protection (BFS). Each visited super-trait contributes its DIRECT
/// `GenericMethod` violations to the child — no fixpoint iteration needed,
/// because direct violations are stable after `register_traits`.
///
/// # Why Only `GenericMethod` Propagates
///
/// `SelfReturn` and `SelfParam` violations reference the trait's own `Self`,
/// which is rebound in each trait that inherits from it (`Self` for `Sub` is
/// distinct from `Self` for `Super`). They do not transitively pollute.
/// `GenericMethod` violations reference the method's OWN type parameters,
/// which exist at every level of the hierarchy regardless of `Self` rebinding.
///
/// BI-6 object safety. Spec: Clause 8.8 (trait objects);
/// `docs/ori_lang/proposals/approved/object-safety-rules-proposal.md`.
pub fn register_object_safety_violations(checker: &mut ModuleChecker<'_>, module: &ori_ir::Module) {
    // Pass 1: read-only — collect inherited GenericMethod violations
    // for each trait by walking the transitive super-trait DAG.
    let mut inherited: FxHashMap<Name, Vec<ObjectSafetyViolation>> = FxHashMap::default();
    for trait_def in &module.traits {
        let registry = checker.trait_registry();
        let trait_idx = match registry.get_trait_by_name(trait_def.name) {
            Some(entry) => entry.idx,
            None => continue,
        };
        let mut here: Vec<ObjectSafetyViolation> = Vec::new();
        for super_idx in registry.all_super_traits(trait_idx) {
            if let Some(super_entry) = registry.get_trait_by_idx(super_idx) {
                for violation in &super_entry.object_safety_violations {
                    if let ObjectSafetyViolation::GenericMethod { method, .. } = violation {
                        here.push(ObjectSafetyViolation::GenericMethod {
                            method: *method,
                            span: trait_def.span,
                        });
                    }
                }
            }
        }
        if !here.is_empty() {
            inherited.insert(trait_def.name, here);
        }
    }

    // Pass 2: apply collected inherited violations.
    for (name, violations) in inherited {
        checker
            .trait_registry_mut()
            .extend_object_safety_violations(name, violations);
    }
}

/// Register public traits from a foreign module (e.g., prelude).
///
/// Uses the foreign module's arena to resolve generic params and method
/// signatures. Only public traits are registered.
pub(crate) fn register_imported_traits(
    checker: &mut ModuleChecker<'_>,
    module: &ori_ir::Module,
    foreign_arena: &ExprArena,
) {
    for trait_def in &module.traits {
        if trait_def.visibility.is_public() {
            register_trait(checker, trait_def, foreign_arena);
        }
    }
}

/// Register a single trait definition.
///
/// Converts an `ori_ir::TraitDef` to a `TraitEntry` and registers it in the
/// `TraitRegistry`. This enables method resolution and trait bound checking.
///
/// Takes an explicit `arena` so that foreign-module traits can be registered
/// using the foreign module's `ExprArena` (for resolving generic params and
/// method signatures).
fn register_trait(
    checker: &mut ModuleChecker<'_>,
    trait_def: &ori_ir::TraitDef,
    arena: &ExprArena,
) {
    // 1. Collect generic parameters
    let type_params = collect_generic_params(arena, trait_def.generics);

    // 2. Create pool index for this trait
    let idx = checker.pool_mut().named(trait_def.name);

    // 3. Process trait items (methods and associated types)
    let mut methods = FxHashMap::default();
    let mut assoc_types = FxHashMap::default();

    for item in &trait_def.items {
        match item {
            TraitItem::MethodSig(sig) => {
                // Required method (no default implementation)
                let method_def = build_trait_method_sig(checker, sig, &type_params, arena);
                methods.insert(sig.name, method_def);
            }
            TraitItem::DefaultMethod(default_method) => {
                // Method with default implementation
                let method_def =
                    build_trait_default_method(checker, default_method, &type_params, arena);
                methods.insert(default_method.name, method_def);
            }
            TraitItem::AssocType(assoc) => {
                // Associated type (with optional default)
                let assoc_def = build_trait_assoc_type(checker, assoc, &type_params, arena);
                assoc_types.insert(assoc.name, assoc_def);
            }
        }
    }

    // 4. Resolve super-traits to pool indices
    let super_traits: Vec<Idx> = trait_def
        .super_traits
        .iter()
        .map(|bound| checker.pool_mut().named(bound.name()))
        .collect();

    // 5. Compute object safety violations from the original AST
    let object_safety_violations = compute_object_safety_violations(checker, trait_def, arena);

    // 6. Register in TraitRegistry
    let entry = TraitEntry {
        name: trait_def.name,
        idx,
        type_params,
        super_traits,
        methods,
        assoc_types,
        object_safety_violations,
        span: trait_def.span,
    };

    checker.trait_registry_mut().register_trait(entry);
}

/// Analyze a trait definition for object safety violations.
///
/// Checks each trait method against the three object safety rules:
/// 1. No `Self` in return position
/// 2. No `Self` in parameter position (except `self` receiver)
/// 3. No per-method generic type parameters (currently not parseable)
///
/// Returns violations found. An empty list means the trait is object-safe.
pub(super) fn compute_object_safety_violations(
    checker: &ModuleChecker<'_>,
    trait_def: &ori_ir::TraitDef,
    arena: &ExprArena,
) -> Vec<ObjectSafetyViolation> {
    let mut violations = Vec::new();

    for item in &trait_def.items {
        let (name, params_range, return_ty, span, generics) = match item {
            TraitItem::MethodSig(sig) => {
                (sig.name, sig.params, &sig.return_ty, sig.span, sig.generics)
            }
            TraitItem::DefaultMethod(m) => (m.name, m.params, &m.return_ty, m.span, m.generics),
            TraitItem::AssocType(_) => continue,
        };

        // Rule 1: Check return type for Self
        if parsed_type_contains_self(arena, return_ty) {
            violations.push(ObjectSafetyViolation::SelfReturn { method: name, span });
        }

        // Rule 2: Check non-receiver params for Self
        let params = arena.get_params(params_range);
        for (i, param) in params.iter().enumerate() {
            // Skip the first parameter if it's `self` (the receiver)
            if i == 0 && param.name == checker.well_known().self_kw {
                continue;
            }

            if let Some(ty) = &param.ty {
                if parsed_type_contains_self(arena, ty) {
                    violations.push(ObjectSafetyViolation::SelfParam {
                        method: name,
                        param: param.name,
                        span,
                    });
                }
            }
        }

        // Rule 3: Generic methods — methods with their own type parameters
        // require monomorphization, which is incompatible with vtable dispatch.
        // Spec: Clause 8.8 (trait objects); (object safety).
        if !generics.is_empty() {
            violations.push(ObjectSafetyViolation::GenericMethod { method: name, span });
        }
    }

    violations
}

/// Build a `TraitMethodDef` from a required method signature.
fn build_trait_method_sig(
    checker: &mut ModuleChecker<'_>,
    sig: &ori_ir::TraitMethodSig,
    type_params: &[Name],
    arena: &ExprArena,
) -> TraitMethodDef {
    // Resolve parameter types
    let params: Vec<_> = arena.get_params(sig.params).to_vec();
    let param_types: Vec<Idx> = params
        .iter()
        .map(|p| {
            p.ty.as_ref().map_or(Idx::ERROR, |ty| {
                resolve_type_with_params(checker, ty, type_params, arena)
            })
        })
        .collect();

    // Resolve return type
    let return_ty = resolve_type_with_params(checker, &sig.return_ty, type_params, arena);

    // Create function type for signature
    let signature = checker.pool_mut().function(&param_types, return_ty);

    // Phase B Step 3: deep-copy method-level generics + where-clauses into
    // arena-independent owned form for downstream bound enforcement. In trait
    // context `Self` stays symbolic, so we pass `Idx::ERROR` as the self_type.
    // The Phase B Step 5b `_overlay` is consumed by `build_impl_method` for
    // call-site instantiable scheme wrapping; trait-side scheme wrapping is
    // not yet wired.
    let (scheme_var_ids, _overlay, generic_param_metadata, where_clause_metadata) =
        build_method_generic_metadata(
            checker,
            sig.generics,
            &sig.where_clauses,
            type_params,
            Idx::ERROR,
        );

    let has_self = params
        .first()
        .is_some_and(|p| p.name == checker.well_known().self_kw);
    let const_params: Vec<Name> = arena
        .get_generic_params(sig.generics)
        .iter()
        .filter(|param| param.is_const)
        .map(|param| param.name)
        .collect();
    let fixed_list_capacity_constraints =
        collect_method_capacity_constraints(arena, &const_params, &params, &sig.return_ty, None);

    TraitMethodDef {
        name: sig.name,
        signature,
        has_self,
        has_default: false,
        default_body: None,
        scheme_var_ids,
        generic_param_metadata,
        where_clause_metadata,
        fixed_list_capacity_constraints,
        span: sig.span,
    }
}

/// Build a `TraitMethodDef` from a method with default implementation.
fn build_trait_default_method(
    checker: &mut ModuleChecker<'_>,
    method: &ori_ir::TraitDefaultMethod,
    type_params: &[Name],
    arena: &ExprArena,
) -> TraitMethodDef {
    // Resolve parameter types
    let params: Vec<_> = arena.get_params(method.params).to_vec();
    let param_types: Vec<Idx> = params
        .iter()
        .map(|p| {
            p.ty.as_ref().map_or(Idx::ERROR, |ty| {
                resolve_type_with_params(checker, ty, type_params, arena)
            })
        })
        .collect();

    // Resolve return type
    let return_ty = resolve_type_with_params(checker, &method.return_ty, type_params, arena);

    // Create function type for signature
    let signature = checker.pool_mut().function(&param_types, return_ty);

    // Phase B Step 3: deep-copy method-level generics + where-clauses (see
    // `build_trait_method_sig` for rationale on the `Idx::ERROR` self_type and
    // for the Phase B Step 5b `_overlay` discard).
    let (scheme_var_ids, _overlay, generic_param_metadata, where_clause_metadata) =
        build_method_generic_metadata(
            checker,
            method.generics,
            &method.where_clauses,
            type_params,
            Idx::ERROR,
        );

    let has_self = params
        .first()
        .is_some_and(|p| p.name == checker.well_known().self_kw);
    let const_params: Vec<Name> = arena
        .get_generic_params(method.generics)
        .iter()
        .filter(|param| param.is_const)
        .map(|param| param.name)
        .collect();
    let fixed_list_capacity_constraints = collect_method_capacity_constraints(
        arena,
        &const_params,
        &params,
        &method.return_ty,
        Some(method.body),
    );

    TraitMethodDef {
        name: method.name,
        signature,
        has_self,
        has_default: true,
        default_body: Some(method.body),
        scheme_var_ids,
        generic_param_metadata,
        where_clause_metadata,
        fixed_list_capacity_constraints,
        span: method.span,
    }
}

/// Build a `TraitAssocTypeDef` from an associated type declaration.
fn build_trait_assoc_type(
    checker: &mut ModuleChecker<'_>,
    assoc: &ori_ir::TraitAssocType,
    type_params: &[Name],
    arena: &ExprArena,
) -> TraitAssocTypeDef {
    // Resolve default type if present
    let default = assoc
        .default_type
        .as_ref()
        .map(|ty| resolve_type_with_params(checker, ty, type_params, arena));

    // TODO: Resolve bounds on associated type
    let bounds = Vec::new();

    TraitAssocTypeDef {
        name: assoc.name,
        bounds,
        default,
        span: assoc.span,
    }
}
