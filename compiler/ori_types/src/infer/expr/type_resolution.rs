//! Type resolution — converting `ParsedType` AST nodes into pool `Idx` values.

use ori_ir::{ExprArena, Name, ParsedType, ParsedTypeRange, Span, TypeId};

use crate::check::ObjectSafetyChecker;
use crate::{Idx, ObjectSafetyViolation, Tag, TypeCheckError};

use super::super::InferEngine;
use super::fixed_list_capacity::validate_fixed_list_capacities;

/// Resolve a `ParsedType` from the AST into a pool `Idx`, recursing for
/// compound types (functions, containers, etc.). `Named` resolves via
/// well-known generics, the `TypeRegistry`, the environment, then FFI
/// carrier inference, falling back to a fresh named var; `SelfType`
/// resolves via the current impl's `Self` binding, falling back to a
/// fresh var; `AssociatedType` resolves via the base type's trait impls,
/// falling back to a fresh var.
pub fn resolve_parsed_type(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    parsed: &ParsedType,
) -> Idx {
    match parsed {
        // Primitive Types
        ParsedType::Primitive(type_id) => resolve_type_id(engine, *type_id),

        // Container Types
        ParsedType::List(elem_id) => {
            let elem = arena.get_parsed_type(*elem_id);
            let elem_ty = resolve_parsed_type(engine, arena, elem);
            engine.pool_mut().list(elem_ty)
        }

        ParsedType::FixedList { elem, capacity: _ } => {
            // Spec: Clause 8.2.2 - capacity-aware subtyping is target-only;
            // erased to a plain list until capacity tracking ships.
            let elem_parsed = arena.get_parsed_type(*elem);
            let elem_ty = resolve_parsed_type(engine, arena, elem_parsed);
            engine.pool_mut().list(elem_ty)
        }

        ParsedType::Map { key, value } => {
            let key_parsed = arena.get_parsed_type(*key);
            let value_parsed = arena.get_parsed_type(*value);
            let key_ty = resolve_parsed_type(engine, arena, key_parsed);
            let value_ty = resolve_parsed_type(engine, arena, value_parsed);
            engine.pool_mut().map(key_ty, value_ty)
        }

        // Tuple Types
        ParsedType::Tuple(elems) => {
            if elems.is_empty() {
                Idx::UNIT
            } else {
                let elem_types = resolve_parsed_type_list(engine, arena, *elems);
                engine.pool_mut().tuple(&elem_types)
            }
        }

        // Function Types
        ParsedType::Function { params, ret } => {
            let param_types = resolve_parsed_type_list(engine, arena, *params);
            let ret_parsed = arena.get_parsed_type(*ret);
            let ret_ty = resolve_parsed_type(engine, arena, ret_parsed);
            engine.pool_mut().function(&param_types, ret_ty)
        }

        // Named Types
        ParsedType::Named { name, type_args } => {
            resolve_named_type(engine, arena, *name, *type_args)
        }

        // Inference Markers
        // Infer and ConstExpr both produce fresh variables (const eval not yet implemented).
        // Note: registration (check/registration.rs) uses Idx::ERROR for ConstExpr because
        // registration needs deterministic types. Inference can defer via fresh vars.
        ParsedType::Infer | ParsedType::ConstExpr(_) => engine.fresh_var(),

        ParsedType::SelfType => engine
            .impl_self_type()
            .unwrap_or_else(|| engine.fresh_var()),

        ParsedType::AssociatedType { base, assoc_name } => {
            let base_parsed = arena.get_parsed_type(*base);
            let base_ty = resolve_parsed_type(engine, arena, base_parsed);
            let resolved_base = engine.resolve(base_ty);

            // Search trait impls for the associated type
            if let Some(trait_registry) = engine.trait_registry() {
                for impl_entry in trait_registry.impls_for_type(resolved_base) {
                    if let Some(&assoc_ty) = impl_entry.assoc_types.get(assoc_name) {
                        return assoc_ty;
                    }
                }
            }

            // Not found — return fresh variable for deferred resolution
            engine.fresh_var()
        }

        ParsedType::TraitBounds(bounds) => {
            // Bounded trait object: Printable + Hashable
            // Spec: Clause 8.8 - dedicated trait-object encoding is
            // target-only; the first bound stands in as the placeholder Idx.
            let bound_ids = arena.get_parsed_type_list(*bounds);
            if let Some(&first_id) = bound_ids.first() {
                let first = arena.get_parsed_type(first_id);
                resolve_parsed_type(engine, arena, first)
            } else {
                engine.fresh_var()
            }
        }
    }
}

fn resolve_named_type(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    name: Name,
    type_args: ParsedTypeRange,
) -> Idx {
    let resolved_args = resolve_named_type_args(engine, arena, type_args);
    if !resolved_args.is_empty() {
        return resolve_applied_named_type(engine, name, &resolved_args);
    }

    if let Some(primitive) = resolve_named_primitive(engine, name) {
        return primitive;
    }
    if engine
        .type_registry()
        .is_some_and(|registry| registry.get_by_name(name).is_some())
    {
        return engine.pool_mut().named(name);
    }
    if let Some(ty) = engine.env().lookup(name) {
        return engine.instantiate(ty);
    }
    if let Some(ffi_type) = resolve_named_ffi_type(engine, name) {
        return ffi_type;
    }
    engine.fresh_named_var(name)
}

fn resolve_named_type_args(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    type_args: ParsedTypeRange,
) -> Vec<Idx> {
    if type_args.is_empty() {
        Vec::new()
    } else {
        resolve_parsed_type_list(engine, arena, type_args)
    }
}

fn resolve_applied_named_type(engine: &mut InferEngine<'_>, name: Name, args: &[Idx]) -> Idx {
    let resolved = if let Some(wk) = engine.well_known() {
        wk.resolve_generic(engine.pool_mut(), name, args)
    } else if let Some(name_str) = engine.lookup_name(name) {
        crate::check::resolve_well_known_generic(engine.pool_mut(), name_str, args)
    } else {
        None
    };
    resolved.unwrap_or_else(|| engine.pool_mut().applied(name, args))
}

fn resolve_named_primitive(engine: &InferEngine<'_>, name: Name) -> Option<Idx> {
    if let Some(wk) = engine.well_known() {
        return wk.resolve_primitive(name);
    }
    match engine.lookup_name(name)? {
        "int" => Some(Idx::INT),
        "float" => Some(Idx::FLOAT),
        "bool" => Some(Idx::BOOL),
        "str" => Some(Idx::STR),
        "char" => Some(Idx::CHAR),
        "byte" => Some(Idx::BYTE),
        "void" | "()" => Some(Idx::UNIT),
        "never" | "Never" => Some(Idx::NEVER),
        "Duration" | "duration" => Some(Idx::DURATION),
        "Size" | "size" => Some(Idx::SIZE),
        "ordering" | "Ordering" => Some(Idx::ORDERING),
        _ => None,
    }
}

fn resolve_named_ffi_type(engine: &mut InferEngine<'_>, name: Name) -> Option<Idx> {
    let carrier = if let Some(wk) = engine.well_known() {
        wk.resolve_ffi_concrete(name)
            .zip(wk.resolve_ffi_cabi_kind(name))
    } else {
        let kind = ori_ir::CAbiKind::from_name(engine.lookup_name(name)?)?;
        let concrete = if kind.is_float() {
            Idx::FLOAT
        } else {
            Idx::INT
        };
        Some((concrete, kind))
    }?;
    let named_idx = engine.pool_mut().named(name);
    engine
        .pool_mut()
        .attach_ffi_carrier(named_idx, carrier.0, carrier.1);
    Some(named_idx)
}

/// Resolve a list of parsed types into a vector of pool indices.
pub(super) fn resolve_parsed_type_list(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    range: ParsedTypeRange,
) -> Vec<Idx> {
    let ids = arena.get_parsed_type_list(range);
    ids.iter()
        .map(|id| {
            let parsed = arena.get_parsed_type(*id);
            resolve_parsed_type(engine, arena, parsed)
        })
        .collect()
}

/// Resolve a `TypeId` primitive to an `Idx`.
///
/// Handles the mapping between `TypeId` constants (from `ori_ir`) and `Idx` constants.
///
/// # `TypeId` Overlap
///
/// `TypeId` and `Idx` now share the same index layout for primitives (0-11),
/// so this is an identity mapping. INFER (12) and `SELF_TYPE` (13) are markers
/// that become fresh inference variables.
fn resolve_type_id(engine: &mut InferEngine<'_>, type_id: TypeId) -> Idx {
    let raw = type_id.raw();
    if raw < TypeId::PRIMITIVE_COUNT {
        // Primitives 0-11 map by identity (TypeId and Idx share the same layout)
        Idx::from_raw(raw)
    } else {
        // INFER (12), SELF_TYPE (13), or unknown — create a fresh variable
        engine.fresh_var()
    }
}

// Type Well-Formedness Checks (Inference Phase)

/// Check that map key types implement `Hashable` (E2031).
///
/// If `ty` is a `Map<K, V>`, verifies that `K` implements `Hashable`.
/// Uses `WellKnownNames::type_satisfies_trait` for primitives and compound types,
/// and the trait registry for user-defined types.
fn check_map_key_hashable(engine: &mut InferEngine<'_>, ty: Idx, span: Span) {
    if engine.pool().tag(ty) != Tag::Map {
        return;
    }

    let key_ty = engine.pool().map_key(ty);
    let key_tag = engine.pool().tag(key_ty);

    // Skip checks for type variables (not yet resolved) and error types
    if key_tag == Tag::Var || key_tag == Tag::Infer || key_ty == Idx::ERROR {
        return;
    }

    // Check via WellKnownNames (primitives + compound types) — borrow dance
    let satisfies_via_wellknown = {
        engine
            .well_known()
            .is_some_and(|wk| wk.type_satisfies_trait(key_ty, wk.hashable, engine.pool()))
    };
    if satisfies_via_wellknown {
        return;
    }

    // User-defined types: check trait registry for Hashable impl
    let has_impl = {
        let hashable_name = engine.well_known().map(|wk| wk.hashable);
        if let Some(h_name) = hashable_name {
            let hashable_idx = engine.pool_mut().named(h_name);
            engine
                .trait_registry()
                .is_some_and(|reg| reg.has_impl(hashable_idx, key_ty))
        } else {
            // No well-known cache — skip check (isolated test context)
            return;
        }
    };
    if !has_impl {
        engine.push_error(TypeCheckError::non_hashable_map_key(span, key_ty));
    }
}

/// Resolve a parsed type and check it for non-object-safe trait usage (E2024).
///
/// Combines `resolve_parsed_type` with an object safety check. Use this
/// instead of `resolve_parsed_type` at sites where user-written type
/// annotations may contain trait objects: let bindings, lambda parameters,
/// lambda return types, and type casts.
pub(crate) fn resolve_and_check_parsed_type(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    parsed: &ParsedType,
    span: Span,
) -> Idx {
    validate_fixed_list_capacities(engine, arena, parsed);
    crate::check::check_parsed_type_object_safety(engine, parsed, span, arena);
    let resolved = resolve_parsed_type(engine, arena, parsed);
    check_map_key_hashable(engine, resolved, span);
    resolved
}

/// `InferEngine` implementation of object safety checking.
///
/// Unlike `ModuleChecker`, `InferEngine` has an *optional* trait registry
/// (it may not be set during isolated inference). When absent, all names
/// pass the object safety check.
impl ObjectSafetyChecker for InferEngine<'_> {
    fn is_well_known_concrete(&self, name: Name, num_args: usize) -> bool {
        if let Some(wk) = self.well_known() {
            wk.is_concrete(name, num_args)
        } else {
            self.lookup_name(name)
                .is_some_and(|s| crate::check::is_concrete_named_type(s, num_args))
        }
    }

    fn check_and_emit(&mut self, name: Name, span: Span) {
        // Borrow dance: scope the trait_registry borrow to extract violations,
        // then use self mutably to push the error.
        let violations: Option<Vec<ObjectSafetyViolation>> = {
            let Some(trait_reg) = self.trait_registry() else {
                return;
            };
            trait_reg
                .get_trait_by_name(name)
                .filter(|entry| !entry.is_object_safe())
                .map(|entry| entry.object_safety_violations.clone())
        };
        if let Some(violations) = violations {
            self.push_error(TypeCheckError::not_object_safe(span, name, violations));
        }
    }
}
