//! Type resolution helpers for the registration phase.
//!
//! These functions resolve `ParsedType` nodes from the IR into `Idx` type
//! handles in the Pool. They are used across all registration submodules
//! (user types, traits, impls, derived).

use ori_ir::{ExprArena, Name, ParsedType, TypeId};

use crate::{Idx, ModuleChecker};

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

/// Resolve a parsed type to an Idx, with generic parameters in scope.
///
/// This is a simplified version that handles common cases during type registration.
/// For full type resolution during inference, use the `resolve_parsed_type` function
/// from the `infer` module.
pub(super) fn resolve_field_type(
    checker: &mut ModuleChecker<'_>,
    parsed: &ParsedType,
    _type_params: &[Name],
) -> Idx {
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
            checker.pool_mut().named(*name)
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
/// during inference. For registration, we just create a named type placeholder.
pub(super) fn resolve_type_with_params(
    checker: &mut ModuleChecker<'_>,
    parsed: &ParsedType,
    type_params: &[Name],
    arena: &ExprArena,
) -> Idx {
    match parsed {
        ParsedType::Named { name, .. } => {
            // Check if this is a type parameter
            if type_params.contains(name) {
                // Create a named type for the parameter
                // During inference, this will be replaced with a fresh type variable
                checker.pool_mut().named(*name)
            } else {
                // Regular named type
                resolve_parsed_type_simple(checker, parsed, arena)
            }
        }
        ParsedType::SelfType => {
            // Self type - create a placeholder named type
            // Will be substituted with the actual implementing type during impl registration
            let self_name = checker.interner().intern("Self");
            checker.pool_mut().named(self_name)
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
    resolve_type_with_self_inner(checker, parsed, type_params, self_type, arena)
}

/// Inner implementation of Self-substituting type resolution.
fn resolve_type_with_self_inner(
    checker: &mut ModuleChecker<'_>,
    parsed: &ParsedType,
    type_params: &[Name],
    self_type: Idx,
    arena: &ExprArena,
) -> Idx {
    match parsed {
        ParsedType::SelfType => self_type,
        ParsedType::Named { name, .. } => {
            // Check if this is a type parameter
            if type_params.contains(name) {
                checker.pool_mut().named(*name)
            } else {
                resolve_parsed_type_simple(checker, parsed, arena)
            }
        }
        ParsedType::List(elem_id) => {
            let elem = arena.get_parsed_type(*elem_id);
            let elem_ty =
                resolve_type_with_self_inner(checker, elem, type_params, self_type, arena);
            checker.pool_mut().list(elem_ty)
        }
        ParsedType::Map { key, value } => {
            let key_parsed = arena.get_parsed_type(*key);
            let value_parsed = arena.get_parsed_type(*value);
            let key_ty =
                resolve_type_with_self_inner(checker, key_parsed, type_params, self_type, arena);
            let value_ty =
                resolve_type_with_self_inner(checker, value_parsed, type_params, self_type, arena);
            checker.pool_mut().map(key_ty, value_ty)
        }
        ParsedType::Tuple(elems) => {
            let elem_ids = arena.get_parsed_type_list(*elems);
            let elem_types: Vec<Idx> = elem_ids
                .iter()
                .map(|&elem_id| {
                    let elem = arena.get_parsed_type(elem_id);
                    resolve_type_with_self_inner(checker, elem, type_params, self_type, arena)
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
                    resolve_type_with_self_inner(checker, param, type_params, self_type, arena)
                })
                .collect();
            let ret_parsed = arena.get_parsed_type(*ret);
            let ret_ty =
                resolve_type_with_self_inner(checker, ret_parsed, type_params, self_type, arena);
            checker.pool_mut().function(&param_types, ret_ty)
        }
        // Bounded trait object: resolve first bound with self-substitution
        ParsedType::TraitBounds(bounds) => {
            let bound_ids = arena.get_parsed_type_list(*bounds);
            if let Some(&first_id) = bound_ids.first() {
                let first = arena.get_parsed_type(first_id);
                resolve_type_with_self_inner(checker, first, type_params, self_type, arena)
            } else {
                Idx::ERROR
            }
        }
        _ => resolve_parsed_type_simple(checker, parsed, arena),
    }
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
