//! Trait definition registration (Pass 0c, part 1).
//!
//! Registers trait definitions from the IR into the `TraitRegistry`. This enables
//! method resolution and trait bound checking. Handles both local and imported
//! (foreign-module) traits.

use ori_ir::{ExprArena, Name, TraitItem};
use rustc_hash::FxHashMap;

use super::type_resolution::{
    collect_generic_params, parsed_type_contains_self, resolve_type_with_params,
};
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
        let (name, params_range, return_ty, span) = match item {
            TraitItem::MethodSig(sig) => (sig.name, sig.params, &sig.return_ty, sig.span),
            TraitItem::DefaultMethod(m) => (m.name, m.params, &m.return_ty, m.span),
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

        // Rule 3: Generic methods — currently trait methods cannot have their
        // own generics (TraitMethodSig has no `generics` field), so this rule
        // cannot be violated. When per-method generics are added to the parser,
        // this check will need to be implemented.
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

    TraitMethodDef {
        name: sig.name,
        signature,
        has_default: false,
        default_body: None,
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

    TraitMethodDef {
        name: method.name,
        signature,
        has_default: true,
        default_body: Some(method.body),
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
